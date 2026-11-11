//! Pricing-context orchestration for generated horizon markets.

use crate::{
    ad::{
        forward::ADForward,
        tape::{Tape, TapeHolder},
    },
    core::{
        marketdatahandling::{
            constructedelementstore::ConstructedElementStore, marketview::MarketView,
        },
        pricingcontext::PricingContext,
    },
    quotes::{quote::Quote, quoteselector::QuoteSelector, quotestore::QuoteStore},
    utils::errors::{QSError, Result},
};

use super::{
    advanceintime::AdvanceInTime,
    configuration::{ForwardMarketConfiguration, MarketAdvancementDetails},
    curve::CompositeAdvancer,
    quotation::ForwardQuotation,
    source::ForwardMarketSource,
};

// Preserve any enclosing valuation's differentiation recording, including errors.
struct RecordingScope(Option<Tape<ADForward>>);

impl RecordingScope {
    fn new() -> Self {
        Self(Some(ADForward::with_tape(std::mem::take)))
    }
}

impl Drop for RecordingScope {
    fn drop(&mut self) {
        if let Some(tape) = self.0.take() {
            ADForward::with_tape(|current| *current = tape);
        }
    }
}

impl PricingContext {
    /// Generates horizon quotes, FX and fixings and returns a fresh pricing context.
    ///
    /// Construction remains lazy; call [`Self::initialize`] to construct every
    /// configured market including simulations. Generated numeric observations
    /// have independent horizon risk coordinates. The source is unchanged.
    ///
    /// # Errors
    /// Rejects backward dates, expired dated calibration contracts, missing
    /// dependencies, and unsupported source objects or evolution conventions.
    pub fn advance_to(&self, configuration: &ForwardMarketConfiguration) -> Result<Self> {
        configuration.validate()?;
        if configuration.target_date() < self.evaluation_date() {
            return Err(QSError::InvalidValueErr(
                "Cannot advance a pricing context backwards".into(),
            ));
        }
        let _recording = RecordingScope::new();
        let mut source = self.clone().with_simulation_configurations(Vec::new());
        source.initialize()?;
        self.advance_in_time(configuration, &ForwardMarketSource::new(&source))
    }
}

impl AdvanceInTime for PricingContext {
    type Output = Self;
    fn advance_in_time(
        &self,
        configuration: &ForwardMarketConfiguration,
        source: &ForwardMarketSource<'_>,
    ) -> Result<Self> {
        source.validate_date(configuration)?;
        for index in source.context().constructed_elements().simulations().keys() {
            if !self
                .simulation_configurations()
                .iter()
                .any(|spec| spec.market_index() == index)
            {
                return Err(QSError::ValueNotSetErr(format!("Supplied simulation {index} requires a SimulationConfiguration for advancement")));
            }
        }
        let mut elements = ConstructedElementStore::default();
        for (index, curve) in source.context().constructed_elements().discount_curves() {
            elements
                .discount_curves_mut()
                .insert(index.clone(), curve.advance_in_time(configuration, source)?);
        }
        for (index, curve) in source.context().constructed_elements().credit_curves() {
            elements
                .credit_curves_mut()
                .insert(index.clone(), curve.advance_in_time(configuration, source)?);
        }
        for (index, curve) in source.context().constructed_elements().dividend_curves() {
            elements
                .dividend_curves_mut()
                .insert(index.clone(), curve.advance_in_time(configuration, source)?);
        }
        CompositeAdvancer::new(source.context(), configuration).rebuild(&mut elements)?;
        let fx = self.fx_store().advance_in_time(configuration, source)?;
        let fixings = self.fixing_store().advance_in_time(configuration, source)?;
        let market = Self::new()
            .with_quote_store(QuoteStore::new(configuration.target_date()))
            .with_quote_level(self.quote_level())
            .with_base_currency(self.base_currency())
            .with_base_index(self.base_index().clone())
            .with_fixing_store(fixings)
            .with_fx_store(fx)
            .with_constructed_elements(elements)
            .with_anchor_fixings(self.anchor_fixings());
        ForwardQuotation::new(self, configuration, source, &market).build()
    }
}

impl MarketView<'_> {
    /// Generates a forward context from this view's effective shocked quotes.
    ///
    /// # Errors
    /// Returns the errors documented by [`PricingContext::advance_to`].
    pub fn advance_to(&self, configuration: &ForwardMarketConfiguration) -> Result<PricingContext> {
        let base = self.source_context();
        let mut quotes = QuoteStore::new(base.evaluation_date());
        for identifier in base.quote_store().quotes().keys() {
            let quote: Quote = self
                .select(identifier)
                .ok_or_else(|| QSError::NotFoundErr(format!("Source quote {identifier}")))?;
            quotes.add_quote(quote);
        }
        let future = base
            .clone()
            .with_quote_store(quotes)
            .advance_to(configuration)?;
        let details = MarketAdvancementDetails {
            source_date: base.evaluation_date(),
            source_timestamp: base.snapshot_timestamp(),
            source_scenario: Some(self.applied_scenario().clone()),
            configuration: configuration.clone(),
        };
        Ok(future.with_market_advancement_details(details))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ad::{dual::DualFwd, tape::Tape},
        calibration::impliedquote::ImpliedQuotePricer,
        core::{
            elements::curveelement::{DiscountCurveElement, DividendCurveElement},
            forwardmarket::{
                advanceintime::AdvanceInTime,
                configuration::{
                    CurveRoll, FixingRoll, ForwardMarketConfiguration, VolatilityRoll,
                },
                source::ForwardMarketSource,
            },
            makeinstrument::MakeInstrument,
            marketdatahandling::{
                constructedelementrequest::ConstructedElementRequest,
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketDataProvider, MarketDataRequest},
            },
            marketsnapshot::MarketSnapshot,
            pricer::Pricer,
            pricingcontext::PricingContext,
            request::Request,
            trade::Side,
        },
        currencies::currency::Currency,
        indices::{fxpair::FxPair, marketindex::MarketIndex},
        instruments::{
            cashflows::{cashflow::Cashflow, cashflowtype::CashflowType},
            credit::{creditdefaultswap::CdsTrade, makecreditdefaultswap::MakeCreditDefaultSwap},
            equity::{
                equityeuropeanoption::{EquityEuropeanOptionTrade, EuroOptionType},
                makeequityeuropeanoption::MakeEquityEuropeanOption,
            },
            fx::{fxforward::FxForwardTrade, makefxforward::MakeFxForward},
            rates::{
                capletfloorlet::{CapletFloorletTrade, CapletFloorletType},
                makecapletfloorlet::MakeCapletFloorlet,
            },
        },
        math::interpolation::{extrapolation::ExtrapolationPolicy, interpolator::Interpolator},
        models::{
            brownianmotion::brownianmotionmodel::BrownianMotion,
            modelconfiguration::SimulationConfiguration,
        },
        pricers::{
            credit::cdspricer::CdsPricer,
            equity::blackeuropeanoptionpricer::BlackEuropeanOptionPricer,
            fx::fxforwardpricer::FxForwardPricer,
            rates::{
                closedformblackcapletpricer::ClosedFormBlackCapletPricer,
                closedformhullwhitecapletpricer::ClosedFormHullWhiteCapletPricer,
            },
        },
        quotes::{
            fixingstore::FixingStore,
            fxstore::FxStore,
            quote::{CalibrationInstrumentType, Level, Quote, QuoteDetails, QuoteLevels},
            quotestore::QuoteStore,
            scenario::{Scenario, ScenarioType, Shock},
        },
        rates::{
            bootstrapping::{
                creditcurveconfiguration::CreditCurveConfiguration,
                curveconfiguration::CurveConfiguration,
            },
            compounding::Compounding,
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{
            date::Date,
            datetime::DateTime,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        utils::errors::{QSError, Result},
        volatility::{
            volatilitycubeconfiguration::VolatilityCubeConfiguration,
            volatilityindexing::{SmileType, Strike, VolatilityType},
            volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
        },
    };
    use std::{cell::RefCell, rc::Rc};

    struct Fixture;

    impl Fixture {
        fn context() -> Result<PricingContext> {
            let today = Date::new(2026, 1, 5);
            let mut quotes = QuoteStore::new(today);
            let mut identifiers = Vec::new();
            for (tenor, value) in [("0D", 0.025), ("1Y", 0.03), ("3Y", 0.04), ("10Y", 0.05)] {
                let id = format!("ZeroRate_USD_SOFR_{tenor}");
                quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(value)));
                identifiers.push(id);
            }
            Ok(PricingContext::new()
                .with_quote_store(quotes)
                .with_curve_configurations(vec![CurveConfiguration::new(
                    MarketIndex::SOFR,
                    DayCounter::Actual365,
                    Interpolator::Linear,
                    true,
                    identifiers,
                )]))
        }

        fn curve(context: &PricingContext) -> Result<DiscountCurveElement> {
            context
                .handle_request(
                    &MarketDataRequest::default().with_constructed_elements_request(vec![
                        ConstructedElementRequest::DiscountCurve {
                            market_index: MarketIndex::SOFR,
                        },
                    ]),
                )?
                .constructed_elements()
                .discount_curve(&MarketIndex::SOFR)
                .cloned()
                .ok_or_else(|| QSError::NotFoundErr("SOFR".into()))
        }
    }

    #[test]
    fn direct_quotes_rebuild_forward_factors_and_leave_source_independent() -> Result<()> {
        let source = Fixture::context()?;
        let horizon = source.evaluation_date() + Period::new(6, TimeUnit::Months);
        let config = ForwardMarketConfiguration::new(horizon);
        let future = source.advance_to(&config)?;
        assert_eq!(future.evaluation_date(), horizon);
        assert_eq!(source.request_cache().discount_curve_count(), 0);
        assert!(!future.has_supplied_elements());
        let base = Fixture::curve(&source)?;
        let advanced = Fixture::curve(&future)?;
        for tenor in ["1Y", "3Y", "10Y"] {
            let date = horizon + tenor.parse::<Period>()?;
            let expected = base.curve().discount_factor(date)?.value()
                / base.curve().discount_factor(horizon)?.value();
            assert!((advanced.curve().discount_factor(date)?.value() - expected).abs() < 1e-12);
        }
        let stressed = Scenario::new(vec![Shock::new(
            "ZeroRate_USD_SOFR",
            0.01,
            ScenarioType::Absolute,
        )])
        .apply(&future)?;
        let data = stressed.handle_request(
            &MarketDataRequest::default().with_constructed_elements_request(vec![
                ConstructedElementRequest::DiscountCurve {
                    market_index: MarketIndex::SOFR,
                },
            ]),
        )?;
        let date = horizon + Period::new(3, TimeUnit::Years);
        assert!(
            data.constructed_elements()
                .discount_curve(&MarketIndex::SOFR)
                .unwrap()
                .curve()
                .discount_factor(date)?
                .value()
                < advanced.curve().discount_factor(date)?.value()
        );
        assert_eq!(source.evaluation_date(), Date::new(2026, 1, 5));
        Ok(())
    }

    #[test]
    fn projects_intervening_fixings_without_future_information() -> Result<()> {
        let source = Fixture::context()?;
        let today = source.evaluation_date();
        let mut history = FixingStore::default();
        history.add_fixing(&MarketIndex::SOFR, today - 1, 0.02);
        history.add_fixing(&MarketIndex::SOFR, today + 1, 0.99);
        let source = source.with_fixing_store(history);
        let horizon = today + 8;
        let future = source.advance_to(&ForwardMarketConfiguration::new(horizon))?;
        assert_eq!(
            future
                .fixing_store()
                .fixing(&MarketIndex::SOFR, today - 1)?,
            0.02
        );
        let rate = future
            .fixing_store()
            .fixing(&MarketIndex::SOFR, today + 1)?;
        assert!(rate > 0.02 && rate < 0.05);
        assert_eq!(
            source
                .fixing_store()
                .fixing(&MarketIndex::SOFR, today + 1)?,
            0.99
        );
        Ok(())
    }

    #[test]
    fn par_swap_generation_recalibrates_at_horizon() -> Result<()> {
        let mut source = Fixture::context()?;
        source.initialize()?;
        let definitions = ["OIS_USD_SOFR_1Y", "OIS_USD_SOFR_3Y", "OIS_USD_SOFR_5Y"]
            .iter()
            .map(|id| id.parse())
            .collect::<Result<Vec<QuoteDetails>>>()?;
        let values =
            ImpliedQuotePricer::new(&source, source.constructed_elements().discount_curves())
                .quotes(&definitions)?;
        let mut quotes = QuoteStore::new(source.evaluation_date());
        for (details, value) in definitions.into_iter().zip(values) {
            quotes.add_quote(Quote::new(details, QuoteLevels::with_mid(value)));
        }
        let ids = quotes.quotes().keys().cloned().collect();
        let source = PricingContext::new()
            .with_quote_store(quotes)
            .with_curve_configurations(vec![CurveConfiguration::new(
                MarketIndex::SOFR,
                DayCounter::Actual365,
                Interpolator::LogLinear,
                true,
                ids,
            )]);
        let mut future = source.advance_to(&ForwardMarketConfiguration::new(
            source.evaluation_date() + 180,
        ))?;
        future.initialize()?;
        let details = future
            .quote_store()
            .quotes()
            .values()
            .map(|quote| quote.details().clone())
            .collect::<Vec<_>>();
        let values =
            ImpliedQuotePricer::new(&future, future.constructed_elements().discount_curves())
                .quotes(&details)?;
        for (details, value) in details.iter().zip(values) {
            assert!(
                (value
                    - future
                        .quote_store()
                        .quote(&details.identifier())
                        .unwrap()
                        .levels()
                        .value(Level::Mid)?)
                .abs()
                    < 1e-9
            );
        }
        Ok(())
    }

    #[test]
    fn constant_tenor_volatility_and_credit_rebuild() -> Result<()> {
        let mut source = Fixture::context()?;
        let mut quotes = source.quote_store().clone();
        let vol = "CapletFloorlet_USD_TermSOFR3m_3M_1Y_Absolute_0.040_Straddle_Black";
        quotes.add_quote(Quote::new(vol.parse()?, QuoteLevels::with_mid(0.25)));
        let cds = ["Cds_ACME_USD_1Y", "Cds_ACME_USD_5Y"];
        for (id, spread) in cds.iter().zip([0.01, 0.02]) {
            quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(spread)));
        }
        source = source
            .with_quote_store(quotes)
            .with_volatility_surface_configurations(vec![VolatilitySurfaceConfiguration::new(
                MarketIndex::TermSOFR3m,
                VolatilityType::Black,
                SmileType::Strike,
                vec![vol.into()],
            )])
            .with_credit_curve_configurations(vec![CreditCurveConfiguration::new(
                MarketIndex::Credit("ACME".into()),
                Currency::USD,
                MarketIndex::SOFR,
                0.4,
                cds.iter().map(|id| (*id).to_owned()).collect(),
            )]);
        let mut future = source.advance_to(&ForwardMarketConfiguration::new(
            source.evaluation_date() + 90,
        ))?;
        let overridden = source.advance_to(
            &ForwardMarketConfiguration::new(source.evaluation_date() + 90)
                .with_recovery_override(MarketIndex::Credit("ACME".into()), 0.2),
        )?;
        assert!((overridden.credit_curve_configurations()[0].recovery() - 0.2).abs() < 1e-12);
        let original_spread = future
            .quote_store()
            .quote(cds[1])
            .unwrap()
            .levels()
            .value(Level::Mid)?;
        let overridden_spread = overridden
            .quote_store()
            .quote(cds[1])
            .unwrap()
            .levels()
            .value(Level::Mid)?;
        assert!((overridden_spread / original_spread - 4.0 / 3.0).abs() < 1e-12);

        future.initialize()?;
        assert_eq!(
            future
                .quote_store()
                .quote(vol)
                .unwrap()
                .levels()
                .value(Level::Mid)?,
            0.25
        );
        let credit = future
            .constructed_elements()
            .credit_curve(&MarketIndex::Credit("ACME".into()))
            .unwrap();
        assert_eq!(credit.curve().reference_date(), future.evaluation_date());
        assert!(
            (credit
                .curve()
                .discount_factor(future.evaluation_date())?
                .value()
                - 1.0)
                .abs()
                < 1e-12
        );
        for (id, years) in cds.iter().zip([1, 5]) {
            let contract = MakeCreditDefaultSwap::default()
                .with_identifier((*id).into())
                .with_credit_index(MarketIndex::Credit("ACME".into()))
                .with_discount_index(MarketIndex::SOFR)
                .with_currency(Currency::USD)
                .with_spread(0.0)
                .with_recovery(0.4)
                .with_start_date(future.evaluation_date())
                .with_maturity_date(future.evaluation_date() + Period::new(years, TimeUnit::Years))
                .build()?;
            let trade = CdsTrade::new(contract, future.evaluation_date(), 1.0, Side::LongReceive);
            let implied = CdsPricer::new()
                .evaluate(&trade, &[Request::FairRate(0)], &future)?
                .fair_rate()
                .unwrap();
            let target = future
                .quote_store()
                .quote(id)
                .unwrap()
                .levels()
                .value(Level::Mid)?;
            assert!(
                (implied - target).abs() < 1e-8,
                "{id}: {implied} vs {target}"
            );
        }
        Ok(())
    }

    #[test]
    fn configuration_and_generated_snapshot_round_trip() -> Result<()> {
        let source = Fixture::context()?;
        let config = ForwardMarketConfiguration::new(source.evaluation_date() + 30)
            .with_curve_override(MarketIndex::Equity("AAPL".into()), CurveRoll::ConstantTenor);
        let encoded = serde_json::to_string(&config).unwrap();
        let decoded = serde_json::from_str(&encoded).unwrap();
        let future = source.advance_to(&decoded)?;
        let timestamp = "2026-01-05T12:00:00Z".parse::<DateTime>()?;
        let snapshot = MarketSnapshot::from_context(timestamp, &future)?;
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: MarketSnapshot = serde_json::from_str(&json).unwrap();
        let rebuilt = restored.pricing_context(future.fixing_store().clone(), Level::Mid);
        assert_eq!(rebuilt.evaluation_date(), future.evaluation_date());
        assert_eq!(
            rebuilt.market_advancement_details().unwrap().source_date,
            source.evaluation_date()
        );
        let date = future.evaluation_date() + Period::new(1, TimeUnit::Years);
        assert!(
            (Fixture::curve(&rebuilt)?
                .curve()
                .discount_factor(date)?
                .value()
                - Fixture::curve(&future)?
                    .curve()
                    .discount_factor(date)?
                    .value())
            .abs()
                < 1e-12
        );

        Ok(())
    }

    #[test]
    fn same_date_is_identity_and_backward_dates_fail() -> Result<()> {
        let source = Fixture::context()?;
        let same = source.advance_to(&ForwardMarketConfiguration::new(source.evaluation_date()))?;
        for (id, quote) in source.quote_store().quotes() {
            assert_eq!(
                quote.levels().value(Level::Mid)?,
                same.quote_store()
                    .quote(id)
                    .unwrap()
                    .levels()
                    .value(Level::Mid)?
            );
        }
        assert!(source
            .advance_to(&ForwardMarketConfiguration::new(
                source.evaluation_date() - 1
            ))
            .is_err());
        Ok(())
    }

    #[test]
    fn supplied_financing_dividends_and_fx_export_as_rebuildable_inputs() -> Result<()> {
        let mut source = Fixture::context()?;
        let today = source.evaluation_date();
        let index = MarketIndex::Equity("AAPL".into());
        let rate_definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(
                index.clone(),
                Rc::new(RefCell::new(FlatForwardTermStructure::new(
                    today,
                    DualFwd::from(0.04),
                    rate_definition,
                ))),
            ),
        );
        elements.dividend_curves_mut().insert(
            index.clone(),
            DividendCurveElement::new(
                index.clone(),
                Rc::new(RefCell::new(FlatForwardTermStructure::new(
                    today,
                    DualFwd::from(0.01),
                    rate_definition,
                ))),
            ),
        );
        let eur = MarketIndex::Collateral(Currency::EUR, Currency::USD);
        elements.discount_curves_mut().insert(
            eur.clone(),
            DiscountCurveElement::new(
                eur,
                Rc::new(RefCell::new(FlatForwardTermStructure::new(
                    today,
                    DualFwd::from(0.02),
                    rate_definition,
                ))),
            ),
        );
        let mut fx = FxStore::new();
        fx.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::from(1.10));
        let mut fixings = FixingStore::default();
        fixings.add_fixing(&index, today, 100.0);
        let mut quotes = source.quote_store().clone();
        let ids = [
            "EquityCall_USD_AAPL_1Y_Absolute_100",
            "EquityCall_USD_AAPL_3Y_Absolute_100",
        ];
        for id in ids {
            quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(0.20)));
        }
        source = source
            .with_constructed_elements(elements)
            .with_fx_store(fx)
            .with_fixing_store(fixings)
            .with_quote_store(quotes)
            .with_volatility_surface_configurations(vec![VolatilitySurfaceConfiguration::new(
                index.clone(),
                VolatilityType::Black,
                SmileType::Strike,
                ids.into_iter().map(str::to_owned).collect(),
            )]);
        let horizon = today + 180;
        let future = source.advance_to(&ForwardMarketConfiguration::new(horizon))?;
        assert!(!future.has_supplied_elements());
        assert_eq!(future.dividend_curve_configurations().len(), 1);
        let preserved = source.advance_to(
            &ForwardMarketConfiguration::new(horizon).with_fixings(FixingRoll::Preserve),
        )?;
        assert!(
            (preserved.fixing_store().fixing(&index, horizon)?
                - future.fixing_store().fixing(&index, horizon)?)
            .abs()
                < 1e-12
        );
        assert!(preserved
            .fixing_store()
            .fixing(&MarketIndex::SOFR, today + 1)
            .is_err());
        let pair = MarketIndex::FxPair(FxPair::new(Currency::EUR, Currency::USD)?);
        assert!(
            (future.fixing_store().fixing(&pair, horizon)?
                - future
                    .fx_store()
                    .get_fx_rate(Currency::EUR, Currency::USD)?
                    .value())
            .abs()
                < 1e-12
        );

        for quote in future.quote_store().quotes().values() {
            let parsed = quote.details().identifier().parse::<QuoteDetails>()?;
            assert_eq!(parsed.market_index(), quote.details().market_index());
        }

        assert!(
            (future.fixing_store().fixing(&index, horizon)?
                - 100.0 * (0.03_f64 * 180.0 / 365.0).exp())
            .abs()
                < 1e-10
        );
        let usd_factor = Fixture::curve(&source)?
            .curve()
            .discount_factor(horizon)?
            .value();
        let expected_fx = 1.10 * (-0.02_f64 * 180.0 / 365.0).exp() / usd_factor;
        assert!(
            (future
                .fx_store()
                .get_fx_rate(Currency::EUR, Currency::USD)?
                .value()
                - expected_fx)
                .abs()
                < 1e-12
        );
        let serialized_fixings = serde_json::to_string(future.fixing_store()).unwrap();
        let restored_fixings: FixingStore = serde_json::from_str(&serialized_fixings).unwrap();
        assert!(
            (restored_fixings.fixing(&index, horizon)?
                - future.fixing_store().fixing(&index, horizon)?)
            .abs()
                < 1e-12
        );
        let inverse = MarketIndex::FxPair(FxPair::new(Currency::USD, Currency::EUR)?);
        assert!(
            (restored_fixings.fixing(&inverse, horizon)?
                * future
                    .fx_store()
                    .get_fx_rate(Currency::EUR, Currency::USD)?
                    .value()
                - 1.0)
                .abs()
                < 1e-12
        );
        let fixing_date = horizon - 10;
        let delivery = horizon + 30;
        let realized = future.fixing_store().fixing(&pair, fixing_date)?;
        let data = future.handle_request(
            &MarketDataRequest::default().with_constructed_elements_request(vec![
                ConstructedElementRequest::DiscountCurve {
                    market_index: MarketIndex::SOFR,
                },
                ConstructedElementRequest::DiscountCurve {
                    market_index: MarketIndex::Collateral(Currency::EUR, Currency::USD),
                },
            ]),
        )?;
        let usd = data
            .constructed_elements()
            .discount_curve(&MarketIndex::SOFR)
            .unwrap()
            .curve()
            .discount_factor(delivery)?
            .value();
        let eur = data
            .constructed_elements()
            .discount_curve(&MarketIndex::Collateral(Currency::EUR, Currency::USD))
            .unwrap()
            .curve()
            .discount_factor(delivery)?
            .value();
        for settlement in [Currency::USD, Currency::EUR] {
            let instrument = MakeFxForward::default()
                .with_identifier("fixed-ndf".into())
                .with_base_currency(Currency::EUR)
                .with_quote_currency(Currency::USD)
                .with_delivery_date(delivery)
                .with_forward_price(1.05)
                .as_ndf(fixing_date, settlement)
                .build()?;
            let trade = FxForwardTrade::new(instrument, today, 1.0, Side::LongReceive);
            let mut pricer = FxForwardPricer::new();
            pricer.set_discount_policy(Box::new(future.csa_policy()));
            let value = pricer
                .evaluate(&trade, &[Request::Value, Request::Sensitivities], &future)?
                .price()
                .unwrap();
            let expected = if settlement == Currency::USD {
                (realized - 1.05) * usd
            } else {
                (realized - 1.05) / realized
                    * eur
                    * future
                        .fx_store()
                        .get_fx_rate(Currency::EUR, Currency::USD)?
                        .value()
            };
            assert!(
                (value - expected).abs() < 1e-12,
                "NDF {settlement}: {value} vs {expected}"
            );
        }
        let expiry = today + Period::new(2, TimeUnit::Years);
        let option = MakeEquityEuropeanOption::default()
            .with_identifier("existing-equity-option".into())
            .with_market_index(index)
            .with_expiry_date(expiry)
            .with_currency(Currency::USD)
            .with_day_counter(DayCounter::Actual365)
            .with_strike(100.0)
            .with_option_type(EuroOptionType::Call)
            .build()?;
        let trade = EquityEuropeanOptionTrade::new(option, 1.0, today, Side::LongReceive);
        let actual = BlackEuropeanOptionPricer::new()
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &future)?
            .price()
            .unwrap();
        let time = DayCounter::Actual365.year_fraction(horizon, expiry);
        let spot = 100.0 * (0.03_f64 * 180.0 / 365.0).exp();
        let forward = spot * (0.03 * time).exp();
        let expected = (-0.04 * time).exp()
            * BrownianMotion::<f64>::closed_form_price(forward, 100.0, 0.20, time, true)?;
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual {actual} expected {expected}"
        );
        let snapshot = MarketSnapshot::from_context("2026-01-05T12:00:00Z".parse()?, &future)?;
        let restored = snapshot.pricing_context(future.fixing_store().clone(), Level::Mid);
        let replay = BlackEuropeanOptionPricer::new()
            .evaluate(&trade, &[Request::Value], &restored)?
            .price()
            .unwrap();
        assert!((replay - actual).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn source_and_horizon_shocks_are_independent_and_tape_is_preserved() -> Result<()> {
        let source = Fixture::context()?;
        let config = ForwardMarketConfiguration::new(source.evaluation_date() + 180);
        let shock = Scenario::new(vec![Shock::new("SOFR", 0.01, ScenarioType::Absolute)]);
        let before = shock.apply(&source)?.advance_to(&config)?;
        let after_base = source.advance_to(&config)?;
        let after = shock.apply(&after_base)?;
        assert_eq!(before.evaluation_date(), after.evaluation_date());
        assert_eq!(
            source
                .quote_store()
                .quote("ZeroRate_USD_SOFR_1Y")
                .unwrap()
                .levels()
                .value(Level::Mid)?,
            0.03
        );
        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        let mut input = DualFwd::new(2.0);
        input.put_on_tape();
        let output: DualFwd = (input * input).into();
        let _future = source.advance_to(&config)?;
        output.backward()?;
        assert!((input.adjoint()?.value() - 4.0).abs() < 1e-12);
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    #[test]
    fn fixed_but_unpaid_caplets_use_generated_observations_in_both_models() -> Result<()> {
        let base = Fixture::context()?;
        let today = base.evaluation_date();
        let mut quotes = QuoteStore::new(today);
        let mut identifiers = Vec::new();
        for quote in base.quote_store().quotes().values() {
            let id = quote.details().identifier().replace("SOFR", "TermSOFR3m");
            quotes.add_quote(Quote::new(id.parse()?, *quote.levels()));
            identifiers.push(id);
        }
        let vol = "CapletFloorlet_USD_TermSOFR3m_3M_1Y_Absolute_0.020_Straddle_Black";
        quotes.add_quote(Quote::new(vol.parse()?, QuoteLevels::with_mid(0.20)));
        let source = PricingContext::new()
            .with_quote_store(quotes)
            .with_base_index(MarketIndex::TermSOFR3m)
            .with_curve_configurations(vec![CurveConfiguration::new(
                MarketIndex::TermSOFR3m,
                DayCounter::Actual365,
                Interpolator::Linear,
                true,
                identifiers,
            )])
            .with_volatility_surface_configurations(vec![VolatilitySurfaceConfiguration::new(
                MarketIndex::TermSOFR3m,
                VolatilityType::Black,
                SmileType::Strike,
                vec![vol.into()],
            )]);
        let fixing = today + 3;
        let start = today + 5;
        let end = start + 90;
        let horizon = today + 30;
        let future = source.advance_to(&ForwardMarketConfiguration::new(horizon))?;
        let option = MakeCapletFloorlet::default()
            .with_identifier("fixed-caplet".into())
            .with_market_index(MarketIndex::TermSOFR3m)
            .with_currency(Currency::USD)
            .with_payoff_type(CapletFloorletType::Caplet)
            .with_strike(Strike::Absolute(0.02))
            .with_start_accrual_date(start)
            .with_end_accrual_date(end)
            .with_fixing_date(fixing)
            .with_payment_date(end)
            .build()?;
        let trade = CapletFloorletTrade::new(option, today, 1_000_000.0, Side::LongReceive);
        let observation = future
            .fixing_store()
            .fixing(&MarketIndex::TermSOFR3m, fixing)?;
        let market = future.handle_request(
            &MarketDataRequest::default().with_constructed_elements_request(vec![
                ConstructedElementRequest::DiscountCurve {
                    market_index: MarketIndex::TermSOFR3m,
                },
            ]),
        )?;
        let factor = market
            .constructed_elements()
            .discount_curve(&MarketIndex::TermSOFR3m)
            .unwrap()
            .curve()
            .discount_factor(end)?
            .value();
        let expected = factor
            * (observation - 0.02).max(0.0)
            * DayCounter::Actual360.year_fraction(start, end)
            * 1_000_000.0;
        for value in [
            ClosedFormBlackCapletPricer::new()
                .evaluate(&trade, &[Request::Value, Request::Sensitivities], &future)?
                .price()
                .unwrap(),
            ClosedFormHullWhiteCapletPricer::new(0.1, 0.01)
                .evaluate(&trade, &[Request::Value, Request::Sensitivities], &future)?
                .price()
                .unwrap(),
        ] {
            assert!(
                (value - expected).abs() < 1e-9,
                "fixed option {value} expected {expected}"
            );
        }
        Ok(())
    }

    #[test]
    fn forward_variance_is_an_explicit_validated_convention() -> Result<()> {
        let base = Fixture::context()?;
        let mut quotes = base.quote_store().clone();
        let ids = [
            "CapletFloorlet_USD_TermSOFR3m_3M_1Y_Absolute_0.040_Straddle_Black",
            "CapletFloorlet_USD_TermSOFR3m_3M_2Y_Absolute_0.040_Straddle_Black",
        ];
        for id in ids {
            quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(0.20)));
        }
        let surface = VolatilitySurfaceConfiguration::new(
            MarketIndex::TermSOFR3m,
            VolatilityType::Black,
            SmileType::Strike,
            ids.into_iter().map(str::to_owned).collect(),
        )
        .with_extrapolation_policy(ExtrapolationPolicy::Flat);
        let source = base
            .with_quote_store(quotes)
            .with_volatility_surface_configurations(vec![surface]);
        let config = ForwardMarketConfiguration::new(
            source.evaluation_date() + Period::new(1, TimeUnit::Years),
        )
        .with_volatility(VolatilityRoll::ForwardVariance);
        let future = source.advance_to(&config)?;
        assert!(
            (future
                .quote_store()
                .quote(ids[0])
                .unwrap()
                .levels()
                .value(Level::Mid)?
                - 0.20)
                .abs()
                < 1e-12
        );
        let mut inverted = source.quote_store().clone();
        inverted.add_quote(Quote::new(ids[0].parse()?, QuoteLevels::with_mid(0.50)));
        inverted.add_quote(Quote::new(ids[1].parse()?, QuoteLevels::with_mid(0.10)));
        let error = source
            .with_quote_store(inverted)
            .advance_to(&config)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Negative forward variance"), "{error}");
        Ok(())
    }

    #[test]
    fn dated_futures_require_explicit_replacement_and_retarget_configurations() -> Result<()> {
        let today = Date::new(2026, 1, 5);
        let mut quotes = QuoteStore::new(today);
        for (id, value) in [("OIS_USD_SOFR_1M", 0.025), ("Future_USD_SOFR_M6", 97.0)] {
            quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(value)));
        }
        let ids = quotes.quotes().keys().cloned().collect();
        let source = PricingContext::new()
            .with_quote_store(quotes)
            .with_curve_configurations(vec![CurveConfiguration::new(
                MarketIndex::SOFR,
                DayCounter::Actual365,
                Interpolator::LogLinear,
                true,
                ids,
            )]);
        let config = ForwardMarketConfiguration::new(Date::new(2026, 11, 2));
        let error = source.advance_to(&config).err().unwrap().to_string();
        assert!(error.contains("replacement contract"), "{error}");
        let mut future = source.advance_to(
            &config.with_quote_replacement("Future_USD_SOFR_M6", "Future_USD_SOFR_H7".parse()?),
        )?;
        assert!(future.quote_store().quote("Future_USD_SOFR_M6").is_none());
        assert!(future.quote_store().quote("Future_USD_SOFR_H7").is_some());
        assert!(future.curve_configurations()[0]
            .quotes()
            .iter()
            .any(|id| id == "Future_USD_SOFR_H7"));
        future.initialize()?;
        Ok(())
    }

    #[test]
    fn selected_quote_side_is_preserved_without_manufacturing_other_sides() -> Result<()> {
        let base = Fixture::context()?;
        let mut quotes = QuoteStore::new(base.evaluation_date());
        for quote in base.quote_store().quotes().values() {
            let mid = quote.levels().value(Level::Mid)?;
            quotes.add_quote(Quote::new(
                quote.details().clone(),
                QuoteLevels::new(Some(mid), None, Some(mid + 0.01)),
            ));
        }
        let source = base.with_quote_store(quotes).with_quote_level(Level::Ask);
        let mut future = source.advance_to(&ForwardMarketConfiguration::new(
            source.evaluation_date() + 30,
        ))?;
        assert_eq!(future.quote_level(), Level::Ask);
        for quote in future.quote_store().quotes().values() {
            assert!(quote.levels().ask().is_some());
            assert!(quote.levels().mid().is_none());
            assert!(quote.levels().bid().is_none());
        }
        future.initialize()?;
        Ok(())
    }

    #[test]
    fn cubes_and_simulations_rebuild_on_the_horizon_date_grid() -> Result<()> {
        let mut source = Fixture::context()?;
        let mut quotes = source.quote_store().clone();
        let id = "Swaption_USD_SOFR_1Y_5Y_ATM_Black";
        quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(0.20)));
        let simulation: SimulationConfiguration = serde_json::from_value(serde_json::json!({
        "market_index": "SOFR", "model": {"HullWhite": {"alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}}}},
        "n_paths": 3, "seed": 42, "horizon": "1Y", "frequency": "Monthly"
    })).unwrap();
        source = source
            .with_quote_store(quotes)
            .with_volatility_cube_configurations(vec![VolatilityCubeConfiguration::new(
                MarketIndex::SOFR,
                VolatilityType::Black,
                SmileType::Strike,
                vec![id.into()],
            )])
            .with_simulation_configurations(vec![simulation]);
        source.initialize()?;
        let original_date = source.constructed_elements().simulations()[&MarketIndex::SOFR]
            .simulation()
            .borrow()
            .dates()[0];
        let horizon = source.evaluation_date() + 180;
        let mut future = source.advance_to(&ForwardMarketConfiguration::new(horizon))?;
        assert!(future.constructed_elements().simulations().is_empty());
        future.initialize()?;
        let simulation = future.constructed_elements().simulations()[&MarketIndex::SOFR]
            .simulation()
            .borrow();
        assert!(simulation.dates().iter().all(|date| *date > horizon));
        assert_eq!(simulation.n_paths(), 3);
        assert_eq!(
            source.constructed_elements().simulations()[&MarketIndex::SOFR]
                .simulation()
                .borrow()
                .dates()[0],
            original_date
        );
        let cube = future
            .constructed_elements()
            .volatility_cube(&MarketIndex::SOFR)
            .unwrap()
            .cube();
        assert_eq!(cube.reference_date(), horizon);
        assert!(
            (cube
                .volatility_from_period(
                    Period::new(1, TimeUnit::Years),
                    Period::new(5, TimeUnit::Years),
                    0.0
                )?
                .value()
                - 0.20)
                .abs()
                < 1e-12
        );
        Ok(())
    }

    #[test]
    fn component_curve_advancement_retains_native_pillar_derivatives() -> Result<()> {
        let base = Fixture::context()?;
        let mut source = base.clone();
        source.initialize()?;
        let horizon = source.evaluation_date() + 180;
        let date = horizon + Period::new(1, TimeUnit::Years);
        let config = ForwardMarketConfiguration::new(horizon);
        let mut advanced = source
            .constructed_elements()
            .discount_curve(&MarketIndex::SOFR)
            .unwrap()
            .advance_in_time(&config, &ForwardMarketSource::new(&source))?;
        Tape::rewind_to_init_fwd();
        Tape::start_recording_fwd();
        advanced.curve_mut().put_pillars_on_tape();
        let value = advanced.curve().discount_factor(date)?;
        value.backward()?;
        let derivative = advanced
            .curve()
            .pillars()
            .unwrap()
            .iter()
            .find(|(label, _)| label == "ZeroRate_USD_SOFR_1Y")
            .unwrap()
            .1
            .adjoint()?
            .value();
        Tape::stop_recording_fwd();
        Tape::rewind_to_init_fwd();
        let evaluate = |bump| -> Result<f64> {
            let mut quotes = base.quote_store().clone();
            let original = quotes.quote("ZeroRate_USD_SOFR_1Y").unwrap();
            let quote = Quote::new(
                original.details().clone(),
                QuoteLevels::with_mid(0.03 + bump),
            );
            quotes.add_quote(quote);
            let future = base.clone().with_quote_store(quotes).advance_to(&config)?;
            Ok(Fixture::curve(&future)?
                .curve()
                .discount_factor(date)?
                .value())
        };
        let finite_difference = (evaluate(1e-6)? - evaluate(-1e-6)?) / 2e-6;
        assert!(
            (derivative - finite_difference).abs() < 1e-8,
            "AD {derivative} FD {finite_difference}"
        );
        Ok(())
    }

    #[test]
    fn composites_follow_advanced_components_or_an_explicit_independent_override() -> Result<()> {
        let mut source = Fixture::context()?;
        let mut quotes = source.quote_store().clone();
        let spread = MarketIndex::Other("SPREAD".into());
        let funding = MarketIndex::Other("FUNDING".into());
        let mut identifiers = Vec::new();
        for (tenor, value) in [("0D", 0.001), ("1Y", 0.005), ("3Y", 0.008), ("10Y", 0.01)] {
            let id = format!("ZeroRate_USD_OTHER:SPREAD_{tenor}");
            quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(value)));
            identifiers.push(id);
        }
        let quote_id = "DiscountFactor_USD_OTHER:FUNDING_3Y";
        quotes.add_quote(Quote::new(quote_id.parse()?, QuoteLevels::with_mid(0.9)));
        let mut specs = source.curve_configurations().clone();
        specs.push(CurveConfiguration::new(
            spread.clone(),
            DayCounter::Actual365,
            Interpolator::Linear,
            true,
            identifiers,
        ));
        specs.push(CurveConfiguration::composite(
            funding.clone(),
            MarketIndex::SOFR,
            spread.clone(),
        ));
        source = source
            .with_quote_store(quotes)
            .with_curve_configurations(specs);
        let horizon = source.evaluation_date() + 180;
        let mixed = ForwardMarketConfiguration::new(horizon)
            .with_curve_override(spread, CurveRoll::ConstantTenor);
        for config in [
            mixed.clone(),
            mixed
                .with_curve_override(funding.clone(), CurveRoll::ConstantTenor)
                .with_curve_grid(
                    funding.clone(),
                    vec!["0D".parse()?, "3Y".parse()?, "5Y".parse()?],
                ),
        ] {
            let mut future = source.advance_to(&config)?;
            future.initialize()?;
            let factor = future
                .constructed_elements()
                .discount_curve(&funding)
                .unwrap()
                .curve()
                .discount_factor(horizon + Period::new(3, TimeUnit::Years))?
                .value();
            let quoted = future
                .quote_store()
                .quote(quote_id)
                .unwrap()
                .levels()
                .value(Level::Mid)?;
            assert!(
                (factor - quoted).abs() < 1e-12,
                "composite {factor} quote {quoted}"
            );
        }
        Ok(())
    }

    #[test]
    fn bond_forward_quote_matches_source_discounted_future_cashflows() -> Result<()> {
        let base = Fixture::context()?;
        let mut quotes = base.quote_store().clone();
        let id = "FixedRateBond_USD_SOFR_2Y_Annual_0.04_AnchorPrice";
        quotes.add_quote(Quote::new(id.parse()?, QuoteLevels::with_mid(100.0)));
        let source = base.with_quote_store(quotes);
        let future = source.advance_to(&ForwardMarketConfiguration::new(
            source.evaluation_date() + 180,
        ))?;
        let quote = future.quote_store().quote(id).unwrap();
        let built = quote.build_instrument(future.evaluation_date(), Level::Mid, None)?;
        let CalibrationInstrumentType::FixedRateBond(bond, _) = built else {
            panic!("Expected a calibration bond")
        };
        let original_curve = Fixture::curve(&source)?;
        let mut pv = 0.0;
        let mut principal = 0.0;
        let mut settlement = None;
        for cashflow in bond.leg().cashflows() {
            match cashflow {
                CashflowType::Disbursement(flow) => {
                    principal += flow.amount()?;
                    settlement = Some(flow.payment_date());
                }
                CashflowType::Redemption(flow) => {
                    pv += flow.amount()?
                        * original_curve
                            .curve()
                            .discount_factor(flow.payment_date())?
                            .value()
                }
                CashflowType::FixedRateCoupon(flow) => {
                    pv += flow.amount()?
                        * original_curve
                            .curve()
                            .discount_factor(flow.payment_date())?
                            .value()
                }
                _ => {}
            }
        }
        let settlement = settlement.unwrap();
        assert_eq!(settlement, future.evaluation_date());
        let expected = pv * bond.units()
            / (principal * original_curve.curve().discount_factor(settlement)?.value());
        assert!((quote.levels().value(Level::Mid)? - expected).abs() < 1e-10);
        Ok(())
    }
}
