//! Price-preserving changes of coordinates for rate-curve sensitivities.

use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use nalgebra::DMatrix;

use crate::{
    ad::{
        dual::DualFwd,
        forward::ADForward,
        tape::{Tape, TapeHolder},
    },
    calibration::impliedquote::ImpliedQuotePricer,
    core::{
        elements::curveelement::{ADCurveElement, DiscountCurveElement},
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
        let unique: std::collections::HashSet<_> = targets.iter().collect();
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
            values: values
                .into_iter()
                .map(|value| DualFwd::constant(ADForward::constant(value)))
                .collect(),
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
            *element = crate::core::elements::curveelement::CreditCurveElement::new(
                element.market_index().clone(),
                element.recovery(),
                Rc::new(RefCell::new(curve)),
            );
        }
        for element in store.dividend_curves_mut().values_mut() {
            let curve = self.projection.wrap(&*element.curve())?;
            *element = crate::core::elements::curveelement::DividendCurveElement::new(
                element.market_index().clone(),
                Rc::new(RefCell::new(curve)),
            );
        }
        Ok(market)
    }
}

impl MarketDataProvider for RiskContext<'_> {
    fn evaluation_date(&self) -> Date {
        self.base.evaluation_date()
    }
    fn anchor_fixings(&self) -> bool {
        self.base.anchor_fixings()
    }
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
        let _tape = ScratchTape::new();
        let indices: Vec<_> = grids.iter().map(|grid| grid.market_index.clone()).collect();
        let curves = requested_curves(self, &indices)?;
        let sources = source_labels(&curves);
        Tape::start_recording_fwd();
        let mut targets = Vec::new();
        let mut values = Vec::new();
        let mut rows = Vec::new();
        for grid in grids {
            let mut curve = curves[&grid.market_index].curve().clone_curve()?;
            curve.put_pillars_on_tape();
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
                Tape::reset_adjoints_fwd();
                let factor = curve.discount_factor(date)?;
                let rate = InterestRate::<DualFwd>::implied_rate(
                    (DualFwd::one() / factor).into(),
                    definition.day_counter(),
                    definition.compounding(),
                    definition.frequency(),
                    time,
                )?
                .rate();
                rate.backward()?;
                let mut row = vec![0.0; sources.len()];
                for (label, pillar) in curve.pillars().unwrap_or_default() {
                    if let Some(column) = sources.iter().position(|source| source == &label) {
                        row[column] += pillar.adjoint()?.value();
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
        let _tape = ScratchTape::new();
        // Numerical calibration queries should not accumulate AD operations.
        Tape::stop_recording_fwd();
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

fn source_labels(curves: &HashMap<MarketIndex, DiscountCurveElement>) -> Vec<String> {
    let mut labels: Vec<_> = curves
        .values()
        .flat_map(|curve| curve.curve().pillar_labels().unwrap_or_default())
        .collect();
    labels.sort();
    labels.dedup();
    labels
}

// Preserve an enclosing valuation's recording, including on an error or unwind.
struct ScratchTape(Option<Tape<ADForward>>);

impl ScratchTape {
    fn new() -> Self {
        let old = ADForward::with_tape(std::mem::take);
        Self(Some(old))
    }
}

impl Drop for ScratchTape {
    fn drop(&mut self) {
        if let Some(old) = self.0.take() {
            ADForward::with_tape(|tape| *tape = old);
        }
    }
}

struct MappedCurve {
    inner: Box<dyn ADCurveElement>,
    labels: Vec<String>,
    values: Vec<DualFwd>,
    mapping: Vec<Vec<f64>>,
}

impl Pillars<DualFwd> for MappedCurve {
    fn pillars(&self) -> Option<Vec<(String, &DualFwd)>> {
        Some(self.labels.iter().cloned().zip(&self.values).collect())
    }
    fn pillar_labels(&self) -> Option<Vec<String>> {
        Some(self.labels.clone())
    }

    fn put_pillars_on_tape(&mut self) {
        self.values.iter_mut().for_each(DualFwd::put_on_tape);
        self.inner.put_pillars_on_tape();
        // Target leaves precede native leaves, which precede all curve operations.
        // Linking the native leaves therefore preserves reverse tape ordering.
        ADForward::with_tape(|tape| {
            for ((_, native), row) in self
                .inner
                .pillars()
                .unwrap_or_default()
                .iter()
                .zip(&self.mapping)
            {
                if let Some(node) = native.node_ptr().and_then(|pointer| tape.mut_node(pointer)) {
                    for (value, derivative) in self.values.iter().zip(row) {
                        if let Some(pointer) = value.node_ptr() {
                            if *derivative != 0.0 {
                                node.push_child(pointer, ADForward::constant(*derivative));
                            }
                        }
                    }
                }
            }
        });
    }
}

impl ADCurveElement for MappedCurve {
    fn clone_curve(&self) -> Result<Box<dyn ADCurveElement>> {
        Ok(Box::new(Self {
            inner: self.inner.clone_curve()?,
            labels: self.labels.clone(),
            values: self
                .values
                .iter()
                .map(|value| DualFwd::constant(ADForward::constant(value.value())))
                .collect(),
            mapping: self.mapping.clone(),
        }))
    }
}

impl InterestRatesTermStructure<DualFwd> for MappedCurve {
    fn reference_date(&self) -> Date {
        self.inner.reference_date()
    }
    fn discount_factor(&self, date: Date) -> Result<DualFwd> {
        self.inner.discount_factor(date)
    }
    fn forward_rate(
        &self,
        start: Date,
        end: Date,
        comp: Compounding,
        freq: Frequency,
    ) -> Result<DualFwd> {
        self.inner.forward_rate(start, end, comp, freq)
    }
    fn nodes(&self) -> Option<Vec<(Date, DualFwd)>> {
        self.inner.nodes()
    }
    fn day_counter(&self) -> Option<DayCounter> {
        self.inner.day_counter()
    }
    fn discount_factor_from_time(&self, time: f64) -> Result<DualFwd> {
        self.inner.discount_factor_from_time(time)
    }
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<DualFwd> {
        self.inner.forward_rate_from_time(start, end)
    }
}

#[cfg(test)]
mod tests;
