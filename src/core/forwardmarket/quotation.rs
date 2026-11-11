//! Generates native horizon quotes and reconstructible market definitions.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ad::dual::DualFwd,
    calibration::impliedquote::ImpliedQuotePricer,
    core::{
        elements::curveelement::{ADCurveElement, DiscountCurveElement},
        makeinstrument::MakeInstrument,
        pricer::Pricer,
        pricingcontext::PricingContext,
        request::Request,
        trade::Side,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::credit::{
        creditdefaultswap::CdsTrade, makecreditdefaultswap::MakeCreditDefaultSwap,
    },
    math::interpolation::interpolator::Interpolator,
    pricers::credit::cdspricer::CdsPricer,
    quotes::{
        quote::{
            CalibrationInstrumentType, Level, Quote, QuoteDetails, QuoteInstrument, QuoteLevels,
        },
        quotestore::QuoteStore,
    },
    rates::{
        bootstrapping::{
            creditcurveconfiguration::CreditCurveConfiguration,
            curveconfiguration::CurveConfiguration,
        },
        compounding::Compounding,
        interestrate::InterestRate,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency, period::Period},
    utils::errors::{QSError, Result},
    volatility::{
        volatilitycubeconfiguration::VolatilityCubeConfiguration,
        volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
    },
};

use super::{
    advanceintime::AdvanceInTime,
    configuration::{ForwardMarketConfiguration, MarketAdvancementDetails, VolatilityRoll},
    source::ForwardMarketSource,
};

/// Bound source and horizon dependencies for forward quotation and input export.
pub struct ForwardQuotation<'a> {
    original: &'a PricingContext,
    configuration: &'a ForwardMarketConfiguration,
    source: &'a ForwardMarketSource<'a>,
    market: &'a PricingContext,
}

impl<'a> ForwardQuotation<'a> {
    /// Binds quotation to a source definition and a constructed horizon market.
    #[must_use]
    pub const fn new(
        original: &'a PricingContext,
        configuration: &'a ForwardMarketConfiguration,
        source: &'a ForwardMarketSource<'a>,
        market: &'a PricingContext,
    ) -> Self {
        Self {
            original,
            configuration,
            source,
            market,
        }
    }
    fn selected_quote(&self, details: QuoteDetails, value: f64) -> Result<Quote> {
        let level = self.original.quote_level();
        if !value.is_finite() {
            return Err(QSError::InvalidValueErr(format!(
                "Non-finite forward quote {}",
                details.identifier()
            )));
        }
        let levels = match level {
            Level::Mid => QuoteLevels::new(Some(value), None, None),
            Level::Bid => QuoteLevels::new(None, Some(value), None),
            Level::Ask => QuoteLevels::new(None, None, Some(value)),
        };
        Ok(Quote::new(details, levels))
    }

    fn direct_quote(&self, details: &QuoteDetails, dividend: bool) -> Result<f64> {
        let market = self.market;
        let index = details
            .market_index()
            .ok_or_else(|| QSError::ValueNotSetErr("Direct quote index".into()))?;
        let curve = if dividend {
            market
                .constructed_elements()
                .dividend_curve(index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Dividend curve {index}")))?
                .curve()
        } else {
            market
                .constructed_elements()
                .discount_curve(index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Curve {index}")))?
                .curve()
        };
        let date = details
            .maturity()
            .or_else(|| {
                details
                    .tenor()
                    .map(|tenor| market.evaluation_date() + tenor)
            })
            .ok_or_else(|| QSError::ValueNotSetErr("Direct quote maturity".into()))?;
        let factor = curve.discount_factor(date)?.value();
        if *details.instrument() == QuoteInstrument::DiscountFactor {
            return Ok(factor);
        }
        let counter = details
            .day_counter()
            .or_else(|| curve.day_counter())
            .ok_or_else(|| QSError::ValueNotSetErr("Direct zero quote day counter".into()))?;
        let end = if date == market.evaluation_date() {
            date + 1
        } else {
            date
        };
        let factor = if end == date {
            factor
        } else {
            curve.discount_factor(end)?.value()
        };
        Ok(InterestRate::<f64>::implied_rate(
            1.0 / factor,
            counter,
            details.compounding().unwrap_or(Compounding::Continuous),
            details.frequency().unwrap_or(Frequency::Annual),
            counter.year_fraction(market.evaluation_date(), end),
        )?
        .rate())
    }

    fn credit_quote(&self, details: &QuoteDetails, original_id: &str) -> Result<f64> {
        let source = self.source.context();
        let config = self.configuration;
        let spec = source
            .credit_curve_configurations()
            .iter()
            .find(|spec| {
                spec.quotes()
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(original_id))
            })
            .ok_or_else(|| {
                QSError::NotFoundErr(format!("Credit configuration for {original_id}"))
            })?;
        let spec = spec
            .clone()
            .with_recovery(config.recovery(spec.market_index(), spec.recovery()));
        self.credit_quote_for_spec(details, &spec)
    }

    fn credit_quote_for_spec(
        &self,
        details: &QuoteDetails,
        spec: &CreditCurveConfiguration,
    ) -> Result<f64> {
        let market = self.market;
        let horizon = market.evaluation_date();
        let maturity = details
            .maturity()
            .or_else(|| details.tenor().map(|tenor| horizon + tenor))
            .ok_or_else(|| QSError::ValueNotSetErr("CDS tenor".into()))?;
        let cds = MakeCreditDefaultSwap::default()
            .with_identifier(details.identifier())
            .with_credit_index(spec.market_index().clone())
            .with_discount_index(spec.discount_index().clone())
            .with_currency(spec.currency())
            .with_recovery(spec.recovery())
            .with_premium_frequency(spec.premium_frequency())
            .with_day_counter(details.day_counter().unwrap_or_else(|| spec.day_counter()))
            .with_spread(0.0)
            .with_start_date(horizon)
            .with_maturity_date(maturity)
            .build()?;
        let trade = CdsTrade::new(cds, horizon, 1.0, Side::LongReceive);
        CdsPricer::new()
            .evaluate(&trade, &[Request::FairRate(0)], market)?
            .fair_rate()
            .ok_or_else(|| QSError::EvaluationErr("No implied CDS spread".into()))
    }

    fn volatility_quote(&self, quote: &Quote) -> Result<f64> {
        let config = self.configuration;
        let source = self.source;
        let details = quote.details();
        let index = details
            .market_index()
            .ok_or_else(|| QSError::ValueNotSetErr("Volatility quote index".into()))?;
        let expiry = details
            .option_expiry()
            .or_else(|| details.tenor())
            .ok_or_else(|| QSError::ValueNotSetErr("Volatility quote expiry".into()))?;
        let key = details
            .strike()
            .ok_or_else(|| QSError::ValueNotSetErr("Volatility quote strike".into()))?
            .resolve(0.0);
        if config.volatility_roll(index) == VolatilityRoll::ConstantTenor {
            if let Some(surface) = source
                .context()
                .constructed_elements()
                .volatility_surface(index)
            {
                return surface
                    .surface()
                    .volatility_from_period(expiry, key)
                    .map(|value| value.value());
            }
            if let Some(cube) = source
                .context()
                .constructed_elements()
                .volatility_cube(index)
            {
                let tenor = details
                    .tenor()
                    .ok_or_else(|| QSError::ValueNotSetErr("Swaption tenor".into()))?;
                return cube
                    .cube()
                    .volatility_from_period(expiry, tenor, key)
                    .map(|value| value.value());
            }
            return quote.levels().value(source.context().quote_level());
        }
        let horizon = config.target_date();
        let end = horizon + expiry;
        let volatility = |date| {
            if let Some(surface) = source
                .context()
                .constructed_elements()
                .volatility_surface(index)
            {
                surface
                    .surface()
                    .volatility_from_date(date, key)
                    .map(|value| value.value())
            } else if let Some(cube) = source
                .context()
                .constructed_elements()
                .volatility_cube(index)
            {
                let tenor = details
                    .tenor()
                    .ok_or_else(|| QSError::ValueNotSetErr("Swaption tenor".into()))?;
                cube.cube()
                    .volatility_from_date(date, tenor, key)
                    .map(|value| value.value())
            } else {
                Err(QSError::NotFoundErr(format!("Volatility market {index}")))
            }
        };
        let counter = DayCounter::Actual365;
        let elapsed = counter.year_fraction(source.context().evaluation_date(), horizon);
        let total = counter.year_fraction(source.context().evaluation_date(), end);
        let start_variance = if elapsed == 0.0 {
            0.0
        } else {
            volatility(horizon)?.powi(2) * elapsed
        };
        let remaining_variance = volatility(end)?.powi(2).mul_add(total, -start_variance);
        if remaining_variance < -1e-12 {
            return Err(QSError::InvalidValueErr(format!(
                "Negative forward variance for {}",
                details.identifier()
            )));
        }
        Ok((remaining_variance.max(0.0) / counter.year_fraction(horizon, end)).sqrt())
    }

    fn generate_quote(
        &self,
        quote: &Quote,
        original_id: &str,
        implied: Option<f64>,
    ) -> Result<Quote> {
        let config = self.configuration;
        let source = self.source;
        let details = quote.details();
        if details
            .maturity()
            .is_some_and(|date| date < config.target_date())
        {
            return Err(QSError::InvalidValueErr(format!(
                "Dated quote {} expires before the horizon; replace its definition",
                details.identifier()
            )));
        }
        let value = match details.instrument() {
            QuoteInstrument::DiscountFactor | QuoteInstrument::ZeroRate => self.direct_quote(
                details,
                source
                    .context()
                    .dividend_curve_configurations()
                    .iter()
                    .any(|spec| {
                        spec.quotes()
                            .iter()
                            .any(|id| id.eq_ignore_ascii_case(original_id))
                    }),
            )?,
            QuoteInstrument::Cds => self.credit_quote(details, original_id)?,
            QuoteInstrument::EquityCall
            | QuoteInstrument::EquityPut
            | QuoteInstrument::FxCall
            | QuoteInstrument::FxPut
            | QuoteInstrument::CapFloor
            | QuoteInstrument::CapletFloorlet
            | QuoteInstrument::EuropeanSwaption => self.volatility_quote(quote)?,
            // Convexity adjustments are explicit model assumptions, retained separately from futures forwards.
            QuoteInstrument::ConvexityAdjustment => {
                quote.levels().value(source.context().quote_level())?
            }
            _ => implied.ok_or_else(|| {
                QSError::ValueNotSetErr(format!("Implied quote {}", details.identifier()))
            })?,
        };
        self.selected_quote(details.clone(), value)
    }

    const fn is_rate_quote(instrument: QuoteInstrument) -> bool {
        matches!(
            instrument,
            QuoteInstrument::FixedRateDeposit
                | QuoteInstrument::FixedRateBond
                | QuoteInstrument::BasisSwap
                | QuoteInstrument::OIS
                | QuoteInstrument::Swap
                | QuoteInstrument::FixFloatCrossCurrencySwap
                | QuoteInstrument::FloatFloatCrossCurrencySwap
                | QuoteInstrument::FxForward
                | QuoteInstrument::Future
        )
    }

    fn validate_target(&self, quote: &Quote) -> Result<()> {
        let config = self.configuration;
        let source = self.source;
        let details = quote.details();
        if details
            .maturity()
            .is_some_and(|date| date < config.target_date())
        {
            return Err(QSError::InvalidValueErr(format!(
                "Dated quote {} expires before the horizon; replace its definition",
                details.identifier()
            )));
        }
        if *details.instrument() == QuoteInstrument::Future {
            // Resolve the contract against the source date so a one-digit IMM year
            // cannot silently become the same code in a later decade.
            let built = quote.build_instrument(
                source.context().evaluation_date(),
                source.context().quote_level(),
                None,
            )?;
            if let CalibrationInstrumentType::RateFutures(future) = built {
                if future.start_date() < config.target_date() {
                    return Err(QSError::InvalidValueErr(format!(
                    "Future {} has already started at the horizon; supply a replacement contract",
                    details.identifier()
                )));
                }
            }
        }
        Ok(())
    }

    /// Builds a normal context from generated horizon observations.
    ///
    /// # Errors
    /// Returns an error when quotation or market construction fails.
    pub fn build(&self) -> Result<PricingContext> {
        let original = self.original;
        let config = self.configuration;
        let source = self.source;
        let market = self.market;
        let quote_source = ForwardMarketSource::new(source.context()).with_horizon_market(market);
        let mut quotes = original
            .quote_store()
            .advance_in_time(config, &quote_source)?;
        let mapped = |identifiers: &[String]| {
            identifiers
                .iter()
                .map(|id| config.target_identifier(id))
                .collect()
        };
        let mut curves = original
            .curve_configurations()
            .iter()
            .filter(|spec| {
                !(spec.composite_components().is_some()
                    && config.has_curve_override(spec.market_index()))
            })
            .map(|spec| spec.clone().with_quotes(mapped(spec.quotes())))
            .collect::<Vec<_>>();
        let mut dividend_curves = original
            .dividend_curve_configurations()
            .iter()
            .filter(|spec| {
                !(spec.composite_components().is_some()
                    && config.has_curve_override(spec.market_index()))
            })
            .map(|spec| spec.clone().with_quotes(mapped(spec.quotes())))
            .collect::<Vec<_>>();
        let mut credit_curves = original
            .credit_curve_configurations()
            .iter()
            .map(|spec| {
                spec.clone()
                    .with_quotes(mapped(spec.quotes()))
                    .with_recovery(config.recovery(spec.market_index(), spec.recovery()))
            })
            .collect::<Vec<_>>();
        let mut surfaces = original
            .volatility_surface_configurations()
            .iter()
            .map(|spec| spec.clone().with_quotes(mapped(spec.quotes())))
            .collect::<Vec<_>>();
        let mut cubes = original
            .volatility_cube_configurations()
            .iter()
            .map(|spec| spec.clone().with_quotes(mapped(spec.quotes())))
            .collect::<Vec<_>>();
        // Supplied curves are exported as direct horizon quotes, so later scenarios rebuild them.
        self.export_discount_curves(&mut quotes, &mut curves)?;
        self.export_dividend_curves(&mut quotes, &mut dividend_curves)?;
        self.export_credit_curves(&mut quotes, &mut credit_curves)?;
        self.export_volatility_surfaces(&mut quotes, &mut surfaces)?;
        self.export_volatility_cubes(&mut quotes, &mut cubes)?;

        let context = PricingContext::new()
            .with_quote_store(quotes)
            .with_quote_level(original.quote_level())
            .with_base_currency(original.base_currency())
            .with_base_index(original.base_index().clone())
            .with_anchor_fixings(original.anchor_fixings())
            .with_fixing_store(market.fixing_store().clone())
            .with_fx_store(market.fx_store().clone())
            .with_curve_configurations(curves)
            .with_dividend_curve_configurations(dividend_curves)
            .with_credit_curve_configurations(credit_curves)
            .with_volatility_surface_configurations(surfaces)
            .with_volatility_cube_configurations(cubes)
            .with_simulation_configurations(original.simulation_configurations().clone())
            .with_pricer_configurations(original.pricer_configurations().clone())
            .with_market_advancement_details(MarketAdvancementDetails {
                source_date: original.evaluation_date(),
                source_timestamp: original.snapshot_timestamp(),
                source_scenario: None,
                configuration: config.clone(),
            });
        context.validate()?;
        Ok(context)
    }

    fn export_discount_curves(
        &self,
        quotes: &mut QuoteStore,
        curves: &mut Vec<CurveConfiguration>,
    ) -> Result<()> {
        let original = self.original;
        let market = self.market;
        for (index, curve) in market.constructed_elements().discount_curves() {
            if curves.iter().any(|spec| spec.market_index() == index) {
                continue;
            }
            let currency = match index {
                MarketIndex::Collateral(currency, _) => *currency,
                _ => index
                    .rate_index_details()
                    .map_or_else(|_| original.base_currency(), |details| details.currency()),
            };
            let spec = self.export_curve(index, curve, currency, quotes)?;
            curves.push(spec);
        }
        Ok(())
    }

    fn export_dividend_curves(
        &self,
        quotes: &mut QuoteStore,
        dividend_curves: &mut Vec<CurveConfiguration>,
    ) -> Result<()> {
        let original = self.original;
        let market = self.market;
        for (index, curve) in market.constructed_elements().dividend_curves() {
            if dividend_curves
                .iter()
                .any(|spec| spec.market_index() == index)
            {
                continue;
            }
            let curve = DiscountCurveElement::new(
                index.clone(),
                Rc::new(RefCell::new(curve.curve().clone_curve()?)),
            );
            dividend_curves.push(self.export_curve_named(
                index,
                &curve,
                original.base_currency(),
                quotes,
                "DividendDiscountFactor",
            )?);
        }
        Ok(())
    }

    fn export_credit_curves(
        &self,
        quotes: &mut QuoteStore,
        credit_curves: &mut Vec<CreditCurveConfiguration>,
    ) -> Result<()> {
        let original = self.original;
        let config = self.configuration;
        let market = self.market;
        for (index, curve) in market.constructed_elements().credit_curves() {
            if credit_curves
                .iter()
                .any(|spec| spec.market_index() == index)
            {
                continue;
            }
            let MarketIndex::Credit(name) = index else {
                return Err(QSError::InvalidValueErr(format!(
                    "Supplied credit index {index} must identify a credit entity"
                )));
            };
            let dates = self.export_nodes(index, &*curve.curve())?;
            let definitions = dates
                .into_iter()
                .filter(|(date, _)| *date > config.target_date())
                .map(|(date, _)| {
                    format!(
                        "Cds_{name}_{}_{}D",
                        original.base_currency(),
                        date - config.target_date()
                    )
                    .parse::<QuoteDetails>()
                })
                .collect::<Result<Vec<_>>>()?;
            if definitions.is_empty() {
                return Err(QSError::InvalidValueErr(format!(
                    "No horizon credit pillars for {index}"
                )));
            }
            let spec = CreditCurveConfiguration::new(
                index.clone(),
                original.base_currency(),
                original.base_index().clone(),
                curve.recovery(),
                definitions.iter().map(QuoteDetails::identifier).collect(),
            );
            for details in definitions {
                let value = self.credit_quote_for_spec(&details, &spec)?;
                quotes.add_quote(self.selected_quote(details, value)?);
            }
            credit_curves.push(spec);
        }
        Ok(())
    }

    fn export_volatility_surfaces(
        &self,
        quotes: &mut QuoteStore,
        surfaces: &mut Vec<VolatilitySurfaceConfiguration>,
    ) -> Result<()> {
        let config = self.configuration;
        let source = self.source;
        for (index, element) in source
            .context()
            .constructed_elements()
            .volatility_surfaces()
        {
            if surfaces.iter().any(|spec| spec.market_index() == index) {
                continue;
            }
            let surface = element.surface();
            let definitions = if let Some(grid) = config.surface_grid(index) {
                grid.to_vec()
            } else {
                surface
                    .calibration_instrument_ids()
                    .ok_or_else(|| {
                        QSError::ValueNotSetErr(format!("Supply a surface export grid for {index}"))
                    })?
                    .iter()
                    .map(|id| id.parse::<QuoteDetails>())
                    .collect::<Result<Vec<_>>>()?
            };
            for details in &definitions {
                let expiry = details
                    .option_expiry()
                    .ok_or_else(|| QSError::ValueNotSetErr("Surface grid expiry".into()))?;
                let key = details
                    .strike()
                    .ok_or_else(|| QSError::ValueNotSetErr("Surface grid strike".into()))?
                    .resolve(0.0);
                let value = surface.volatility_from_period(expiry, key)?.value();
                let quote = self.selected_quote(details.clone(), value)?;
                let value = self.volatility_quote(&quote)?;
                quotes.add_quote(self.selected_quote(details.clone(), value)?);
            }
            surfaces.push(VolatilitySurfaceConfiguration::new(
                index.clone(),
                surface.volatility_type(),
                surface.smile_type(),
                definitions.iter().map(QuoteDetails::identifier).collect(),
            ));
        }
        Ok(())
    }

    fn export_volatility_cubes(
        &self,
        quotes: &mut QuoteStore,
        cubes: &mut Vec<VolatilityCubeConfiguration>,
    ) -> Result<()> {
        let config = self.configuration;
        let source = self.source;
        for (index, element) in source.context().constructed_elements().volatility_cubes() {
            if cubes.iter().any(|spec| spec.market_index() == index) {
                continue;
            }
            let cube = element.cube();
            let definitions = if let Some(grid) = config.cube_grid(index) {
                grid.to_vec()
            } else {
                cube.calibration_instrument_ids()
                    .ok_or_else(|| {
                        QSError::ValueNotSetErr(format!("Supply a cube export grid for {index}"))
                    })?
                    .iter()
                    .map(|id| id.parse::<QuoteDetails>())
                    .collect::<Result<Vec<_>>>()?
            };
            for details in &definitions {
                let expiry = details
                    .option_expiry()
                    .ok_or_else(|| QSError::ValueNotSetErr("Cube grid expiry".into()))?;
                let tenor = details
                    .tenor()
                    .ok_or_else(|| QSError::ValueNotSetErr("Cube grid tenor".into()))?;
                let key = details
                    .strike()
                    .ok_or_else(|| QSError::ValueNotSetErr("Cube grid strike".into()))?
                    .resolve(0.0);
                let value = cube.volatility_from_period(expiry, tenor, key)?.value();
                let quote = self.selected_quote(details.clone(), value)?;
                let value = self.volatility_quote(&quote)?;
                quotes.add_quote(self.selected_quote(details.clone(), value)?);
            }
            cubes.push(VolatilityCubeConfiguration::new(
                index.clone(),
                cube.volatility_type(),
                cube.smile_type(),
                definitions.iter().map(QuoteDetails::identifier).collect(),
            ));
        }
        Ok(())
    }

    fn export_curve(
        &self,
        index: &MarketIndex,
        element: &DiscountCurveElement,
        currency: Currency,
        quotes: &mut QuoteStore,
    ) -> Result<CurveConfiguration> {
        self.export_curve_named(index, element, currency, quotes, "DiscountFactor")
    }

    fn export_curve_named(
        &self,
        index: &MarketIndex,
        element: &DiscountCurveElement,
        currency: Currency,
        quotes: &mut QuoteStore,
        prefix: &str,
    ) -> Result<CurveConfiguration> {
        let curve = element.curve();
        let counter = curve.day_counter().ok_or_else(|| {
            QSError::ValueNotSetErr(format!("Day counter of supplied curve {index}"))
        })?;
        let nodes = self.export_nodes(index, &*curve)?;
        if nodes.len() < 2 {
            return Err(QSError::InvalidValueErr(format!(
                "Supplied curve {index} has no future export pillars"
            )));
        }
        let mut identifiers = Vec::new();
        for (date, factor) in nodes {
            let identifier = format!("{prefix}_{currency}_{index}_{date}");
            let details = QuoteDetails::new(identifier.clone(), QuoteInstrument::DiscountFactor)
                .with_market_index(index.clone())
                .with_currency(currency)
                .with_maturity(date);
            quotes.add_quote(self.selected_quote(details, factor.value())?);
            identifiers.push(identifier);
        }
        Ok(CurveConfiguration::new(
            index.clone(),
            counter,
            Interpolator::LogLinear,
            true,
            identifiers,
        ))
    }

    fn export_nodes(
        &self,
        index: &MarketIndex,
        curve: &dyn ADCurveElement,
    ) -> Result<Vec<(Date, DualFwd)>> {
        let config = self.configuration;
        let dates = if let Some(tenors) = config.curve_grid(index) {
            tenors
                .iter()
                .map(|tenor| config.target_date() + *tenor)
                .collect::<Vec<_>>()
        } else if let Some(nodes) = curve.nodes().filter(|nodes| nodes.len() > 1) {
            nodes.into_iter().map(|(date, _)| date).collect()
        } else {
            ["0D", "1Y", "2Y", "5Y", "10Y", "30Y"]
                .iter()
                .map(|tenor| {
                    tenor
                        .parse::<Period>()
                        .map(|tenor| config.target_date() + tenor)
                })
                .collect::<Result<_>>()?
        };
        if dates.is_empty() || dates.windows(2).any(|dates| dates[0] >= dates[1]) {
            return Err(QSError::InvalidValueErr(format!(
                "Export grid for {index} must contain ordered distinct horizon dates"
            )));
        }
        dates
            .into_iter()
            .map(|date| curve.discount_factor(date).map(|factor| (date, factor)))
            .collect()
    }

    fn quotes(&self, store: &QuoteStore) -> Result<QuoteStore> {
        let config = self.configuration;
        let source = self.source;
        let market = self.market;

        source.validate_date(config)?;
        let mut result = QuoteStore::new(config.target_date());
        let mut entries = store
            .quotes()
            .values()
            .map(|quote| {
                let details = config
                    .replacement(&quote.details().identifier())
                    .unwrap_or_else(|| quote.details());
                (quote, Quote::new(details.clone(), *quote.levels()))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|(_, target)| target.details().identifier());
        let mut rate_definitions = Vec::new();
        for (original, target) in &entries {
            if config.target_date() == store.reference_date()
                && config
                    .replacement(&original.details().identifier())
                    .is_none()
            {
                continue;
            }
            self.validate_target(target)?;
            if Self::is_rate_quote(*target.details().instrument()) {
                rate_definitions.push(target.details().clone());
            }
        }
        let values = if rate_definitions.is_empty() {
            Vec::new()
        } else {
            ImpliedQuotePricer::new(market, market.constructed_elements().discount_curves())
                .quotes(&rate_definitions)?
        };
        let implied: HashMap<_, _> = rate_definitions
            .iter()
            .map(QuoteDetails::identifier)
            .zip(values)
            .collect();
        for (original, target) in entries {
            let unchanged_date = config.target_date() == store.reference_date();
            let no_replacement = config
                .replacement(&original.details().identifier())
                .is_none();
            let generated = if unchanged_date && no_replacement {
                original.clone()
            } else {
                self.generate_quote(
                    &target,
                    &original.details().identifier(),
                    implied.get(&target.details().identifier()).copied(),
                )
                .map_err(|error| {
                    error.context(format!(
                        "advancing quote {}",
                        original.details().identifier()
                    ))
                })?
            };
            if result.quote(&generated.details().identifier()).is_some() {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate horizon quote {}",
                    generated.details().identifier()
                )));
            }
            result.add_quote(generated);
        }
        Ok(result)
    }
}

impl AdvanceInTime for QuoteStore {
    type Output = Self;
    fn advance_in_time(
        &self,
        config: &ForwardMarketConfiguration,
        source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        let market = source.horizon_market().ok_or_else(|| {
            QSError::ValueNotSetErr(
                "Quote advancement requires a horizon market; use PricingContext::advance_to"
                    .into(),
            )
        })?;
        ForwardQuotation::new(source.context(), config, source, market).quotes(self)
    }
}
