//! Price-preserving changes of coordinates for rate-curve sensitivities.

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use nalgebra::DMatrix;

use crate::{
    ad::{Recorded, Recording, RecordingScope},
    calibration::impliedquote::ImpliedQuotePricer,
    core::{
        elements::curveelement::{
            ADCurveElement, CreditCurveElement, DiscountCurveElement, DividendCurveElement,
        },
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricingcontext::PricingContext,
    },
    indices::marketindex::MarketIndex,
    quotes::quote::{Level, Quote, QuoteDetails, QuoteLevels},
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
        yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// A zero-rate risk grid for one discount or forwarding curve.
///
/// Rates are annual decimal rates under the supplied convention. By default the
/// grid uses the curve's future nodes; use [`Self::with_dates`] for another grid.
#[derive(Clone)]
pub struct ZeroRateRisk {
    market_index: MarketIndex,
    rate_definition: RateDefinition,
    dates: Option<Vec<Date>>,
}

impl ZeroRateRisk {
    /// Selects a curve and the convention used to quote its zero rates.
    #[must_use]
    pub const fn new(market_index: MarketIndex, rate_definition: RateDefinition) -> Self {
        Self {
            market_index,
            rate_definition,
            dates: None,
        }
    }

    /// Uses explicit future dates, in the supplied order, as the risk grid.
    #[must_use]
    pub fn with_dates(mut self, dates: Vec<Date>) -> Self {
        self.dates = Some(dates);
        self
    }
}

/// A pricing-context view reporting rate-curve risk in another basis.
///
/// The base curves, interpolation, observations and prices are retained. Each
/// response owns detached curve wrappers, so recording risk in this view does
/// not change the base context's pillars. FX, volatility, credit-specific and
/// other independent risk factors retain their native coordinates.
///
/// The change of coordinates is local and first order. The target basis must
/// span the selected source pillars. Redundant target factors receive the
/// minimum Euclidean-norm exposure; no risk is silently discarded.
#[derive(Clone)]
pub struct RiskContext<'a> {
    base: Cow<'a, PricingContext>,
    projection: Arc<Projection>,
}

struct Projection {
    sources: Vec<String>,
    targets: Vec<String>,
    values: Vec<f64>,
    // dq_source / dx_target = pseudoinverse(dx_target / dq_source).
    inverse: DMatrix<f64>,
}

impl Projection {
    /// Validates that the target basis spans the source pillars and computes its inverse map.
    fn new(
        sources: Vec<String>,
        targets: Vec<String>,
        values: Vec<f64>,
        jacobian: DMatrix<f64>,
    ) -> Result<Self> {
        if sources.is_empty()
            || targets.is_empty()
            || values.iter().any(|value| !value.is_finite())
            || jacobian.iter().any(|value| !value.is_finite())
        {
            return Err(QSError::InvalidValueErr(
                "Risk basis must contain finite, nonempty pillars and derivatives".into(),
            ));
        }
        let unique: HashSet<_> = targets.iter().collect();
        if unique.len() != targets.len() {
            return Err(QSError::InvalidValueErr(
                "Risk basis contains duplicate target labels".into(),
            ));
        }
        let svd = jacobian.svd(true, true);
        let tolerance = svd.singular_values.max() * 1e-10;
        if targets.len() < sources.len()
            || svd
                .singular_values
                .iter()
                .filter(|value| **value > tolerance)
                .count()
                < sources.len()
        {
            return Err(QSError::InvalidValueErr(format!(
                "Target risk basis does not span all {} source pillars; use independent instruments or zero dates covering the selected curve risk", sources.len()
            )));
        }
        let inverse = svd
            .pseudo_inverse(tolerance)
            .map_err(|error| QSError::InvalidValueErr(error.into()))?;
        Ok(Self {
            sources,
            targets,
            values,
            inverse,
        })
    }

    /// Clones a curve and reconnects its native pillars to the selected reporting basis.
    fn wrap(&self, curve: &dyn ADCurveElement) -> Result<Box<dyn ADCurveElement>> {
        let native = curve.pillars().unwrap_or_default();
        if !native.iter().any(|(label, _)| self.sources.contains(label)) {
            return curve.clone_curve();
        }
        let mut labels = self.targets.clone();
        let mut values = self.values.clone();
        let mut mapping = Vec::with_capacity(native.len());
        for (label, value) in native {
            if let Some(row) = self.sources.iter().position(|source| source == &label) {
                mapping.push(self.inverse.row(row).iter().copied().collect::<Vec<_>>());
            } else {
                if self.targets.contains(&label) {
                    return Err(QSError::InvalidValueErr(format!(
                        "Target risk label collides with an unchanged pillar: {label}"
                    )));
                }
                let column = labels
                    .iter()
                    .position(|existing| existing == &label)
                    .unwrap_or_else(|| {
                        labels.push(label);
                        values.push(value.value());
                        labels.len() - 1
                    });
                let mut row = vec![0.0; column + 1];
                row[column] = 1.0;
                mapping.push(row);
            }
        }
        Ok(Box::new(MappedCurve {
            inner: curve.clone_curve()?,
            labels,
            values: values.into_iter().map(Recorded::constant).collect(),
            mapping,
        }))
    }
}

impl RiskContext<'_> {
    /// Retains the market inputs and risk basis without borrowing the caller.
    ///
    /// As with a cloned pricing context, underlying cached market objects can be
    /// shared. Responses still receive independent, detached curve wrappers.
    #[must_use]
    pub fn into_owned(self) -> RiskContext<'static> {
        RiskContext {
            base: Cow::Owned(self.base.into_owned()),
            projection: self.projection,
        }
    }

    /// Original rate-curve pillar labels, in Jacobian column order.
    #[must_use]
    pub fn source_labels(&self) -> &[String] {
        &self.projection.sources
    }

    /// Reporting labels for the new rate-curve risk factors.
    #[must_use]
    pub fn target_labels(&self) -> &[String] {
        &self.projection.targets
    }

    /// Base-market implied quotes or zero rates aligned with target labels.
    #[must_use]
    pub fn target_values(&self) -> &[f64] {
        &self.projection.values
    }

    /// Applies this basis to independently supplied market-data responses.
    ///
    /// The response must use the market inputs and native pillar coordinates
    /// from which this basis was calculated. Recompute the basis after shocks.
    ///
    /// # Errors
    /// Returns an error if curves cannot be cloned or pillar labels conflict.
    pub fn transform_market_data(&self, mut market: MarketData) -> Result<MarketData> {
        let store = market.constructed_elements_mut();
        for element in store.discount_curves_mut().values_mut() {
            let curve = self.projection.wrap(&*element.curve())?;
            *element = DiscountCurveElement::new(
                element.market_index().clone(),
                Rc::new(RefCell::new(curve)),
            );
        }
        // Other curve families can carry dependencies on the selected rate pillars.
        for element in store.credit_curves_mut().values_mut() {
            let curve = self.projection.wrap(&*element.curve())?;
            *element = CreditCurveElement::new(
                element.market_index().clone(),
                element.recovery(),
                Rc::new(RefCell::new(curve)),
            );
        }
        for element in store.dividend_curves_mut().values_mut() {
            let curve = self.projection.wrap(&*element.curve())?;
            *element = DividendCurveElement::new(
                element.market_index().clone(),
                Rc::new(RefCell::new(curve)),
            );
        }
        Ok(market)
    }
}

impl MarketDataProvider for RiskContext<'_> {
    /// Returns the valuation date of the underlying market.
    fn evaluation_date(&self) -> Date {
        self.base.evaluation_date()
    }
    /// Returns the underlying market's fixing-anchor policy.
    fn anchor_fixings(&self) -> bool {
        self.base.anchor_fixings()
    }
    /// Resolves the requested market inputs and applies this risk basis to their curves.
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
        self.transform_market_data(self.base.handle_request(request)?)
    }
}

impl PricingContext {
    /// Reports curve sensitivities to zero rates after constructing the curves.
    ///
    /// This changes risk coordinates without resampling or replacing the pricing
    /// curves. A grid's zero rate is implied from the original discount factor
    /// using its explicit day count, compounding and frequency. Selected curves'
    /// native pillars (including parent-curve dependencies) are replaced together;
    /// unrelated curve pillars retain their original coordinates.
    ///
    /// # Errors
    /// Rejects missing curves/nodes, invalid or repeated dates, and grids that do
    /// not span the selected source risk. Composite curves need explicit dates.
    pub fn with_zero_rate_risk(&self, grids: &[ZeroRateRisk]) -> Result<RiskContext<'_>> {
        let _capture = RecordingScope::new();
        let indices: Vec<_> = grids.iter().map(|grid| grid.market_index.clone()).collect();
        let curves = requested_curves(self, &indices)?;
        let sources = source_labels(&curves);
        Recording::start();
        let mut targets = Vec::new();
        let mut values = Vec::new();
        let mut rows = Vec::new();
        for grid in grids {
            let mut curve = curves[&grid.market_index].curve().clone_curve()?;
            curve.register_pillars();
            let dates = match &grid.dates {
                Some(dates) => dates.clone(),
                None => curve
                    .nodes()
                    .ok_or_else(|| {
                        QSError::InvalidValueErr(format!(
                            "Curve {} has no nodes; supply explicit zero-risk dates",
                            grid.market_index
                        ))
                    })?
                    .into_iter()
                    .map(|(date, _)| date)
                    .filter(|date| *date > self.evaluation_date())
                    .collect(),
            };
            if dates.is_empty() {
                return Err(QSError::InvalidValueErr(
                    "Zero-rate risk grid has no future dates".into(),
                ));
            }
            for date in dates {
                let definition = grid.rate_definition;
                let time = definition
                    .day_counter()
                    .year_fraction(self.evaluation_date(), date);
                if date <= self.evaluation_date() || time <= 0.0 {
                    return Err(QSError::InvalidValueErr(
                        "Zero-rate risk dates must be after evaluation".into(),
                    ));
                }
                Recording::reset_derivatives();
                let factor = curve.discount_factor(date)?;
                let rate = InterestRate::<Recorded>::implied_rate(
                    Recorded::one() / factor,
                    definition.day_counter(),
                    definition.compounding(),
                    definition.frequency(),
                    time,
                )?
                .rate();
                rate.differentiate()?;
                let mut row = vec![0.0; sources.len()];
                for (label, pillar) in curve.pillars().unwrap_or_default() {
                    if let Some(column) = sources.iter().position(|source| source == &label) {
                        row[column] += pillar.derivative()?.value();
                    }
                }
                targets.push(format!(
                    "ZeroRateRisk_{}_{date}_{:?}_{:?}_{:?}",
                    grid.market_index,
                    definition.day_counter(),
                    definition.compounding(),
                    definition.frequency()
                ));
                values.push(rate.value());
                rows.extend(row);
            }
        }
        let jacobian = DMatrix::from_row_slice(targets.len(), sources.len(), &rows);
        let projection = Projection::new(sources, targets, values, jacobian)?;
        Ok(RiskContext {
            base: Cow::Borrowed(self),
            projection: Arc::new(projection),
        })
    }

    /// Reports rate-curve risk using a different set of calibration instruments.
    ///
    /// Instrument details define the reporting contracts; their quotes are solved
    /// against the original curves and need not exist in the quote store. The
    /// target quote Jacobian is estimated by central differences of full curve
    /// reconstruction, then inverted jointly across curves. Discounting, parent
    /// dependencies, fixings and the selected quote side enter this calculation.
    /// Prices continue to use the original curves and interpolation.
    ///
    /// The basis covers all configured/supplied discount and forwarding curves.
    /// Supply at least as many independent target quotes as native curve pillars.
    /// This is a local first-order transformation; target markets are not stored
    /// as replacement construction inputs. Rebuild the view after input changes.
    ///
    /// # Errors
    /// Rejects unsupported calibration instruments, non-convergent implied
    /// quotes, unspanned bases, and source pillars without reconstruction quotes.
    /// Supplied runtime curves support zero-rate risk; instrument reprojection
    /// requires configuration-driven curve reconstruction.
    pub fn reproject_risk(&self, instruments: &[QuoteDetails]) -> Result<RiskContext<'_>> {
        let _capture = RecordingScope::new();
        // Numerical calibration queries should not accumulate AD operations.
        Recording::stop();
        let mut indices: Vec<_> = self
            .curve_configurations()
            .iter()
            .map(|config| config.market_index().clone())
            .collect();
        indices.extend(
            self.constructed_elements()
                .discount_curves()
                .keys()
                .cloned(),
        );
        indices.sort_by_key(ToString::to_string);
        indices.dedup();
        let curves = requested_curves(self, &indices)?;
        let sources = source_labels(&curves);
        if self.has_supplied_elements()
            && self
                .constructed_elements()
                .discount_curves()
                .keys()
                .any(|index| {
                    !self
                        .curve_configurations()
                        .iter()
                        .any(|config| config.market_index() == index)
                })
        {
            return Err(QSError::InvalidValueErr("Instrument risk reprojection requires reconstructible curve configurations; use zero-rate risk for supplied curves".into()));
        }
        let values = ImpliedQuotePricer::new(self, &curves).quotes(instruments)?;
        let mut jacobian = DMatrix::zeros(instruments.len(), sources.len());
        for (column, label) in sources.iter().enumerate() {
            let quote = self.quote_store().quote(label).ok_or_else(|| {
                QSError::NotFoundErr(format!("Reconstruction quote for risk pillar {label}"))
            })?;
            let value = quote.levels().value(self.quote_level())?;
            let bump = 1e-5 * value.abs().max(0.01);
            let mut bumped_values = Vec::new();
            for shift in [bump, -bump] {
                let mut quotes = self.quote_store().clone();
                // Preserve the selected level; the other levels are irrelevant here.
                let levels = match self.quote_level() {
                    Level::Mid => QuoteLevels::new(
                        Some(value + shift),
                        quote.levels().bid(),
                        quote.levels().ask(),
                    ),
                    Level::Bid => QuoteLevels::new(
                        quote.levels().mid(),
                        Some(value + shift),
                        quote.levels().ask(),
                    ),
                    Level::Ask => QuoteLevels::new(
                        quote.levels().mid(),
                        quote.levels().bid(),
                        Some(value + shift),
                    ),
                };
                quotes.add_quote(Quote::new(quote.details().clone(), levels));
                let context = self.clone().with_quote_store(quotes);
                let shifted_curves = requested_curves(&context, &indices)?;
                bumped_values
                    .push(ImpliedQuotePricer::new(&context, &shifted_curves).quotes(instruments)?);
            }
            for row in 0..instruments.len() {
                jacobian[(row, column)] =
                    (bumped_values[0][row] - bumped_values[1][row]) / (2.0 * bump);
            }
        }
        let targets = instruments.iter().map(QuoteDetails::identifier).collect();
        let projection = Projection::new(sources, targets, values, jacobian)?;
        Ok(RiskContext {
            base: Cow::Borrowed(self),
            projection: Arc::new(projection),
        })
    }
}

/// Loads the discount or forwarding curves selected for a change of risk basis.
fn requested_curves(
    context: &PricingContext,
    indices: &[MarketIndex],
) -> Result<HashMap<MarketIndex, DiscountCurveElement>> {
    let request = MarketDataRequest::default().with_constructed_elements_request(
        indices
            .iter()
            .map(|index| ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            })
            .collect(),
    );
    Ok(context
        .handle_request(&request)?
        .constructed_elements()
        .discount_curves()
        .iter()
        .map(|(index, curve)| (index.clone(), curve.clone()))
        .collect())
}

/// Returns the distinct native curve-pillar labels in a stable order.
fn source_labels(curves: &HashMap<MarketIndex, DiscountCurveElement>) -> Vec<String> {
    let mut labels: Vec<_> = curves
        .values()
        .flat_map(|curve| curve.curve().pillar_labels().unwrap_or_default())
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

struct MappedCurve {
    inner: Box<dyn ADCurveElement>,
    labels: Vec<String>,
    values: Vec<Recorded>,
    mapping: Vec<Vec<f64>>,
}

impl Pillars<Recorded> for MappedCurve {
    /// Returns the labelled inputs used to report market sensitivities.
    fn pillars(&self) -> Option<Vec<(String, &Recorded)>> {
        Some(self.labels.iter().cloned().zip(&self.values).collect())
    }
    /// Returns the reporting labels in the same order as this curve's input values.
    fn pillar_labels(&self) -> Option<Vec<String>> {
        Some(self.labels.clone())
    }

    /// Registers current labelled market pillars as independent compiler inputs.
    fn register_pillars(&mut self) {
        self.values.iter_mut().for_each(Recorded::register_input);
        self.inner.register_pillars();
        // Target leaves precede native leaves, which precede all curve operations.
        // Linking the native leaves therefore preserves reverse capture ordering.
        for ((_, native), row) in self
            .inner
            .pillars()
            .unwrap_or_default()
            .iter()
            .zip(&self.mapping)
        {
            native.set_linearization(&self.values, row);
        }
    }
}

impl ADCurveElement for MappedCurve {
    /// Clones the curve with detached reporting inputs and the same coordinate map.
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        Ok(Box::new(Self {
            inner: self.inner.clone_curve()?,
            labels: self.labels.clone(),
            values: self
                .values
                .iter()
                .map(|value| Recorded::constant(value.value()))
                .collect(),
            mapping: self.mapping.clone(),
        }))
    }
}

impl InterestRatesTermStructure<Recorded> for MappedCurve {
    /// Returns the original curve's reference date.
    fn reference_date(&self) -> Date {
        self.inner.reference_date()
    }
    /// Returns the discount factor at a date under the curve conventions.
    fn discount_factor(&self, date: Date) -> Result<Recorded> {
        self.inner.discount_factor(date)
    }
    /// Queries the original curve using the requested compounding and frequency.
    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<Recorded> {
        self.inner.forward_rate(start, end, comp, freq)
    }
    /// Returns the curve nodes in their interpolation order.
    fn nodes(&self) -> Option<Vec<(Date, Recorded)>> {
        self.inner.nodes()
    }
    /// Returns the original curve's day-count convention.
    fn day_counter(&self) -> Option<DayCounter> {
        self.inner.day_counter()
    }
    /// Returns the discount factor for a day-count year fraction.
    fn discount_factor_from_time(&self, time: f64) -> Result<Recorded> {
        self.inner.discount_factor_from_time(time)
    }
    /// Returns the forward rate between two day-count year fractions.
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<Recorded> {
        self.inner.forward_rate_from_time(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    /// Builds a two-pillar EUR market for the risk-basis regression tests.
    fn context() -> Result<PricingContext> {
        let date = Date::new(2026, 1, 15);
        let mut quotes = QuoteStore::new(date);
        let labels = [
            "FixedRateDeposit_EUR_ESTR_1Y",
            "FixedRateDeposit_EUR_ESTR_2Y",
        ];
        for (label, value) in labels.iter().zip([0.03, 0.04]) {
            quotes.add_quote(Quote::new(
                label.parse()?,
                QuoteLevels::new(Some(value), Some(value - 0.001), Some(value + 0.001)),
            ));
        }
        Ok(PricingContext::new()
            .with_quote_store(quotes)
            .with_base_currency(Currency::EUR)
            .with_base_index(MarketIndex::ESTR)
            .with_curve_configurations(vec![CurveConfiguration::new(
                MarketIndex::ESTR,
                DayCounter::Actual365,
                Interpolator::LogLinear,
                true,
                labels.iter().map(|label| (*label).into()).collect(),
            )]))
    }

    /// Selects continuous annual zero rates under Actual/365 for the test basis.
    fn definition() -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        )
    }

    /// Evaluates a discount factor and its reported market sensitivities.
    fn factor_risk(
        provider: &impl MarketDataProvider,
        index: MarketIndex,
        date: Date,
    ) -> Result<(f64, HashMap<String, f64>)> {
        let _capture = RecordingScope::new();
        let request = MarketDataRequest::default().with_constructed_elements_request(vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: index.clone(),
            },
        ]);
        let mut market = provider.handle_request(&request)?;
        let element = market
            .constructed_elements_mut()
            .discount_curves_mut()
            .get_mut(&index)
            .unwrap();
        Recording::start();
        element.curve_mut().register_pillars();
        let curve = element.curve();
        let factor = curve.discount_factor(date)?;
        factor.differentiate()?;
        let mut risk = HashMap::new();
        for (label, value) in curve.pillars().unwrap_or_default() {
            *risk.entry(label).or_insert(0.0) += value.derivative()?.value();
        }
        Ok((factor.value(), risk))
    }

    /// Checks projected zero-rate risk against independent node bumps while preserving off-grid prices.
    #[test]
    fn zero_risk_matches_node_bumps_and_preserves_off_grid_prices() -> Result<()> {
        let context = context()?;
        let index = MarketIndex::ESTR;
        let view =
            context.with_zero_rate_risk(&[ZeroRateRisk::new(index.clone(), definition())])?;
        let query = Date::new(2027, 7, 15);
        let (base_value, _) = factor_risk(&context, index.clone(), query)?;
        let (value, risk) = factor_risk(&view, index.clone(), query)?;
        assert_eq!(value, base_value);
        let curves = requested_curves(&context, &[index])?;
        let nodes = curves[&MarketIndex::ESTR].curve().nodes().unwrap();
        let dates: Vec<_> = nodes.iter().map(|(date, _)| *date).collect();
        for (column, label) in view.target_labels().iter().enumerate() {
            let bump = 1e-6;
            let mut values = Vec::new();
            for shift in [bump, -bump] {
                let factors = nodes
                    .iter()
                    .enumerate()
                    .map(|(node, (date, factor))| {
                        let time =
                            DayCounter::Actual365.year_fraction(context.evaluation_date(), *date);
                        if node == column + 1 {
                            factor.value() * (-shift * time).exp()
                        } else {
                            factor.value()
                        }
                    })
                    .collect();
                let bumped = DiscountTermStructure::<f64>::new(
                    dates.clone(),
                    factors,
                    DayCounter::Actual365,
                    Interpolator::LogLinear,
                    true,
                )?;
                values.push(bumped.discount_factor(query)?);
            }
            let finite_difference = (values[0] - values[1]) / (2.0 * bump);
            assert!((risk[label] - finite_difference).abs() < 1e-8);
        }
        Ok(())
    }

    /// Checks custom risk dates and rate conventions through the ordinary pricer interface.
    #[test]
    fn zero_risk_conventions_custom_dates_and_pricer_integration() -> Result<()> {
        let context = context()?;
        let maturity = Date::new(2027, 7, 15);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Semiannual,
        );
        let view = context.with_zero_rate_risk(&[ZeroRateRisk::new(
            MarketIndex::ESTR,
            rate_definition,
        )
        .with_dates(vec![Date::new(2027, 1, 15), Date::new(2028, 1, 15)])])?;
        let deposit = MakeFixedRateDeposit::<Recorded>::default()
            .with_identifier("trade".into())
            .with_start_date(context.evaluation_date())
            .with_maturity_date(maturity)
            .with_notional(100.0)
            .with_rate(0.035)
            .with_rate_definition(RateDefinition::default())
            .with_currency(Currency::EUR)
            .with_discount_index(Some(MarketIndex::ESTR))
            .build()?;
        let trade = FixedRateDepositTrade::new(
            deposit,
            context.evaluation_date(),
            100.0,
            Side::LongReceive,
        );
        let pricer = DiscountedCashflowPricer::<
            FixedRateDeposit<Recorded>,
            FixedRateDepositTrade<Recorded>,
        >::new();
        let base = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &context)?;
        for _ in 0..2 {
            let result =
                pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &view)?;
            assert_eq!(result.price(), base.price());
            let sensitivities = result.sensitivities().unwrap();
            assert_eq!(sensitivities.instrument_keys(), view.target_labels());
            assert!(sensitivities
                .exposure()
                .iter()
                .any(|value| value.abs() > 0.0));
        }
        let (factor, risk) = factor_risk(&view, MarketIndex::ESTR, Date::new(2027, 1, 15))?;
        let zero = view.target_values()[0];
        let time =
            DayCounter::Actual360.year_fraction(context.evaluation_date(), Date::new(2027, 1, 15));
        let expected = -time * factor / (1.0 + zero / 2.0);
        assert!((risk[&view.target_labels()[0]] - expected).abs() < 1e-9);
        Ok(())
    }

    /// Compares instrument-basis risk with independent reconstruction of the target curves.
    #[test]
    fn instrument_reprojection_matches_independent_target_rebootstrap() -> Result<()> {
        let context = context()?;
        let targets: Vec<QuoteDetails> = ["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"]
            .iter()
            .map(|label| label.parse())
            .collect::<Result<_>>()?;
        let view = context.reproject_risk(&targets)?;
        let query = Date::new(2027, 7, 15);
        let (value, risk) = factor_risk(&view, MarketIndex::ESTR, query)?;
        assert_eq!(value, factor_risk(&context, MarketIndex::ESTR, query)?.0);
        for (column, label) in view.target_labels().iter().enumerate() {
            let bump = 1e-6;
            let mut prices = Vec::new();
            for shift in [bump, -bump] {
                let mut quotes = QuoteStore::new(context.evaluation_date());
                for (row, target) in targets.iter().enumerate() {
                    quotes.add_quote(Quote::new(
                        target.clone(),
                        QuoteLevels::with_mid(
                            view.target_values()[row] + if column == row { shift } else { 0.0 },
                        ),
                    ));
                }
                let config = CurveConfiguration::new(
                    MarketIndex::ESTR,
                    DayCounter::Actual365,
                    Interpolator::LogLinear,
                    true,
                    view.target_labels().to_vec(),
                );
                let bumped = context
                    .clone()
                    .with_quote_store(quotes)
                    .with_curve_configurations(vec![config]);
                prices.push(factor_risk(&bumped, MarketIndex::ESTR, query)?.0);
            }
            assert!(
                (risk[label] - (prices[0] - prices[1]) / (2.0 * bump)).abs() < 5e-4,
                "{label}: {} vs {}",
                risk[label],
                (prices[0] - prices[1]) / (2.0 * bump)
            );
        }
        Ok(())
    }

    /// Checks that an identity projection uses the selected market quote side.
    #[test]
    fn identity_reprojection_respects_selected_quote_side() -> Result<()> {
        for level in [Level::Mid, Level::Bid, Level::Ask] {
            let context = context()?.with_quote_level(level);
            let targets: Vec<_> = context.curve_configurations()[0]
                .quotes()
                .iter()
                .map(|label| label.parse())
                .collect::<Result<_>>()?;
            let view = context.reproject_risk(&targets)?;
            let query = Date::new(2027, 7, 15);
            let native = factor_risk(&context, MarketIndex::ESTR, query)?;
            let projected = factor_risk(&view, MarketIndex::ESTR, query)?;
            assert_eq!(native.0, projected.0);
            for (label, exposure) in native.1 {
                assert!((exposure - projected.1[&label]).abs() < 1e-5);
            }
            for (label, value) in view.target_labels().iter().zip(view.target_values()) {
                assert!(
                    (context
                        .quote_store()
                        .quote(label)
                        .unwrap()
                        .levels()
                        .value(level)?
                        - value)
                        .abs()
                        < 1e-10
                );
            }
        }
        Ok(())
    }

    /// Checks that constructing a risk view preserves its enclosing capture and the native pillars.
    #[test]
    fn transformation_preserves_existing_capture_and_native_pillars() -> Result<()> {
        let context = context()?;
        let curves = requested_curves(&context, &[MarketIndex::ESTR])?;
        let _capture = RecordingScope::new();
        Recording::start();
        let x = Recorded::new(2.0);
        let y: Recorded = (x * x).into();
        let mut curve = curves[&MarketIndex::ESTR].clone();
        curve.curve_mut().register_pillars();
        let pointers: Vec<_> = curve
            .curve()
            .pillars()
            .unwrap()
            .iter()
            .map(|(_, value)| value.identity())
            .collect();
        let view =
            context.with_zero_rate_risk(&[ZeroRateRisk::new(MarketIndex::ESTR, definition())])?;
        assert_eq!(
            pointers,
            curve
                .curve()
                .pillars()
                .unwrap()
                .iter()
                .map(|(_, value)| value.identity())
                .collect::<Vec<_>>()
        );
        let _ = factor_risk(&view, MarketIndex::ESTR, Date::new(2027, 7, 15))?;
        assert!(context.with_zero_rate_risk(&[]).is_err());
        y.differentiate()?;
        assert_eq!(x.derivative()?.value(), 4.0);
        Ok(())
    }

    /// Checks that invalid or insufficient reporting bases are rejected.
    #[test]
    fn rejects_empty_duplicate_and_unspanned_bases() -> Result<()> {
        let context = context()?;
        let grid = ZeroRateRisk::new(MarketIndex::ESTR, definition());
        assert!(context.with_zero_rate_risk(&[]).is_err());
        assert!(context.reproject_risk(&[]).is_err());
        for dates in [
            vec![],
            vec![context.evaluation_date()],
            vec![Date::new(2027, 1, 15)],
            vec![Date::new(2027, 1, 15); 2],
        ] {
            assert!(context
                .with_zero_rate_risk(&[grid.clone().with_dates(dates)])
                .is_err());
        }
        assert!(context
            .reproject_risk(&["OIS_EUR_ESTR_1Y".parse()?])
            .is_err());
        assert!(context
            .reproject_risk(&["OIS_EUR_ESTR_1Y".parse()?, "OIS_EUR_ESTR_1Y".parse()?])
            .is_err());
        assert!(context
            .reproject_risk(&["OIS_EUR_ESTR_3M".parse()?, "OIS_EUR_ESTR_6M".parse()?])
            .is_err());
        Ok(())
    }

    /// Checks supplied curves and redundant target grids under the minimum-norm projection.
    #[test]
    fn zero_risk_supports_supplied_curves_and_redundant_target_grids() -> Result<()> {
        let date = Date::new(2026, 1, 15);
        let curve = FlatForwardTermStructure::new(date, Recorded::from(0.03), definition())
            .with_pillar_label("flat".into());
        let mut store = ConstructedElementStore::default();
        store.discount_curves_mut().insert(
            MarketIndex::ESTR,
            DiscountCurveElement::new(MarketIndex::ESTR, Rc::new(RefCell::new(curve))),
        );
        let context = PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_constructed_elements(store);
        let grid = ZeroRateRisk::new(MarketIndex::ESTR, definition())
            .with_dates(vec![Date::new(2027, 1, 15), Date::new(2028, 1, 15)]);
        let view = context.with_zero_rate_risk(&[grid])?;
        let query = Date::new(2027, 7, 15);
        let (value, risk) = factor_risk(&view, MarketIndex::ESTR, query)?;
        let time = DayCounter::Actual365.year_fraction(date, query);
        for label in view.target_labels() {
            assert!((risk[label] + time * value / 2.0).abs() < 1e-10);
        }
        assert!(context
            .reproject_risk(&["FixedRateDeposit_EUR_ESTR_1Y".parse()?])
            .is_err());
        Ok(())
    }

    /// Checks that joint and partial curve bases retain the intended parent-curve dependencies.
    #[test]
    fn joint_zero_basis_removes_bootstrap_parent_risk_and_partial_basis_retains_other_quotes(
    ) -> Result<()> {
        let date = Date::new(2026, 2, 24);
        let mut quotes = QuoteStore::new(date);
        let parent = [
            "FixedRateDeposit_USD_SOFR_1Y",
            "FixedRateDeposit_USD_SOFR_2Y",
        ];
        let child = ["Swap_USD_TermSOFR3m_1Y", "Swap_USD_TermSOFR3m_2Y"];
        for (label, value) in parent.iter().chain(&child).zip([0.04, 0.039, 0.045, 0.046]) {
            quotes.add_quote(Quote::new(label.parse()?, QuoteLevels::with_mid(value)));
        }
        let configurations = [
            (MarketIndex::SOFR, parent),
            (MarketIndex::TermSOFR3m, child),
        ]
        .into_iter()
        .map(|(index, labels)| {
            CurveConfiguration::new(
                index,
                DayCounter::Actual360,
                Interpolator::LogLinear,
                true,
                labels.iter().map(|label| (*label).into()).collect(),
            )
        })
        .collect();
        let context = PricingContext::new()
            .with_quote_store(quotes)
            .with_curve_configurations(configurations);
        let query = Date::new(2028, 2, 24);
        let (base, native) = factor_risk(&context, MarketIndex::TermSOFR3m, query)?;
        assert!(parent.iter().any(|label| native[*label].abs() > 1e-5));

        let partial =
            context.with_zero_rate_risk(&[ZeroRateRisk::new(MarketIndex::SOFR, definition())])?;
        let (partial_value, partial_risk) = factor_risk(&partial, MarketIndex::TermSOFR3m, query)?;
        assert_eq!(partial_value, base);
        for label in child {
            assert!((partial_risk[label] - native[label]).abs() < 1e-10);
        }
        assert!(parent
            .iter()
            .all(|label| !partial_risk.contains_key(*label)));

        let joint = context.with_zero_rate_risk(&[
            ZeroRateRisk::new(MarketIndex::SOFR, definition()),
            ZeroRateRisk::new(MarketIndex::TermSOFR3m, definition()),
        ])?;
        let (joint_value, joint_risk) = factor_risk(&joint, MarketIndex::TermSOFR3m, query)?;
        assert_eq!(joint_value, base);
        for label in &joint.target_labels()[..2] {
            assert!(joint_risk[label].abs() < 1e-8);
        }
        let targets = parent
            .iter()
            .chain(&child)
            .map(|label| label.parse())
            .collect::<Result<Vec<_>>>()?;
        let identity = context.reproject_risk(&targets)?;
        let (projected_value, projected_risk) =
            factor_risk(&identity, MarketIndex::TermSOFR3m, query)?;
        assert_eq!(projected_value, base);
        for (label, exposure) in native {
            assert!((exposure - projected_risk[&label]).abs() < 1e-4, "{label}");
        }
        Ok(())
    }
}
