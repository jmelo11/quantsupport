use super::{
    backwardfourierevolutionbondpricer::backward_fourier_value, treebasedbondpricer::lattice_value,
};
use crate::{
    models::hullwhite::parameters::{hull_white_piecewise_variance, HullWhiteParameters},
    rates::yieldtermstructure::primalcurve::PrimalCurve,
};

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use super::{
    backwardfourierevolutionbondpricer::BackwardFourierEvolutionBondPricer,
    bondanalytics::*,
    bondevents::{bond_event_grid, BondEventGrid, CallStep},
    bondpricer::BondPricer,
    treebasedbondpricer::TreeBasedBondPricer,
};
use crate::prelude::*;
use crate::{
    core::{
        collateral::FixedIncomeDiscountPolicy,
        elements::curveelement::DiscountCurveElement,
        marketdatahandling::constructedelementstore::ConstructedElementStore,
        pricerconfiguration::{PricerConfiguration, PricingModelConfiguration},
        pricingcontext::PricingContext,
    },
    currencies::currency::Currency,
    instruments::{
        cashflows::makeleg::PaymentStructure,
        fixedincome::{
            callschedule::CallScheduleEntry, makefixedratebond::MakeFixedRateBond,
            makesteppedcouponbond::MakeSteppedCouponBond,
        },
    },
    models::{
        hullwhite::hullwhitemodel::HullWhite,
        lgm::lgmcomponents::LgmRateModel,
        modelconfiguration::{GaussianRateModelParameters, ParameterSource},
    },
    quotes::{
        quote::{Level, Quote, QuoteLevels},
        quotestore::{QuoteStore, QuoteStoreRecords},
    },
    rates::{
        compounding::Compounding, interestrate::RateDefinition,
        yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
    },
    time::{
        enums::{Frequency, TimeUnit},
        period::Period,
    },
    volatility::{
        modelcalibration::{CalibrationBasket, ModelCalibrationConfiguration},
        volatilitycubeconfiguration::VolatilityCubeConfiguration,
        volatilityindexing::{SmileType, Strike, VolatilityType},
        volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
    },
};

const FACE: f64 = 100.0;
const RATE: f64 = 0.03;
const ALPHA: f64 = 0.05;
const SIGMA: f64 = 0.01;

#[test]
fn piecewise_native_engines_match_bond_option_and_straight_curve_fit() -> Result<()> {
    let evaluation = Date::new(2025, 1, 1);
    let curve = FlatForwardTermStructure::new(
        evaluation,
        RATE,
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        ),
    );
    let schedule = [(0.5, 0.01), (1.0, 0.02), (2.5, 0.005)];
    let mut terms = BondEventGrid {
        maturity_time: 3.0,
        steps: 384,
        cashflows: vec![0.0; 385],
        calls: vec![CallStep::new(); 385],
    };
    terms.cashflows[384] = FACE;
    terms.calls[256].call_price = Some(95.0);
    let variance = hull_white_piecewise_variance(ALPHA, &schedule, 0.0, 2.0);
    let unit_variance = hull_white_piecewise_variance(ALPHA, &[(0.0, 1.0)], 0.0, 2.0);
    let equivalent_sigma = (variance / unit_variance).sqrt();
    let benchmark = FACE
        * (curve.discount_factor_from_time(3.0)?
            - HullWhite::new(ALPHA, &curve).bond_call_price(
                2.0,
                3.0,
                0.95,
                equivalent_sigma,
                &curve,
            )?);
    let tree = lattice_value(
        |time| curve.discount_factor_from_time(time),
        &terms,
        ALPHA,
        &schedule,
        0.0,
    )?;
    let fourier = backward_fourier_value(
        |time| curve.discount_factor_from_time(time),
        &terms,
        ALPHA,
        &schedule,
        0.0,
        FourierGridConfiguration::new(1024, 10.0),
    )?;
    assert!(
        (tree - benchmark).abs() < 0.08,
        "piecewise tree {tree} vs bond-option benchmark {benchmark}"
    );
    assert!(
        (fourier - benchmark).abs() < 0.10,
        "piecewise Fourier {fourier} vs bond-option benchmark {benchmark}"
    );
    terms.calls[256].call_price = None;
    let straight = FACE * curve.discount_factor_from_time(3.0)?;
    assert!(
        (lattice_value(
            |time| curve.discount_factor_from_time(time),
            &terms,
            ALPHA,
            &schedule,
            0.0
        )? - straight)
            .abs()
            < 1.0e-10
    );
    assert!(
        (backward_fourier_value(
            |time| curve.discount_factor_from_time(time),
            &terms,
            ALPHA,
            &schedule,
            0.0,
            FourierGridConfiguration::new(1024, 10.0)
        )? - straight)
            .abs()
            < 0.10
    );
    Ok(())
}

fn example_calibrated_context(
    use_fourier: bool,
    bump: Option<(&str, f64)>,
) -> Result<PricingContext> {
    let mut records: QuoteStoreRecords =
        serde_json::from_str(include_str!("../../../examples/hullwhite/data/quotes.json"))
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
    if let Some((identifier, amount)) = bump {
        let quote = records
            .quotes
            .iter_mut()
            .find(|quote| quote.identifier == identifier)
            .ok_or_else(|| QSError::NotFoundErr(identifier.into()))?;
        quote.mid = quote.mid.map(|value| value + amount);
    }
    let quotes = QuoteStore::try_from(records)?;
    let curve_json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/hullwhite/data/curve_specs.json"
    ))
    .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
    let curves = serde_json::from_value(curve_json["curve_specs"].clone())
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
    let surface_json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../examples/hullwhite/data/vol_specs.json"
    ))
    .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
    let surfaces: Vec<VolatilitySurfaceConfiguration> =
        serde_json::from_value(surface_json["volatility_surfaces"].clone())
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
    let time_grid = TimeGridConfiguration::new(48, 4096, DayCounter::Actual365);
    let pricer = if use_fourier {
        PricerImplementation::BackwardFourierEvolution {
            time_grid,
            fourier_grid: FourierGridConfiguration::new(512, 10.0),
        }
    } else {
        PricerImplementation::TreeBased { time_grid }
    };
    let configuration = PricerConfiguration::new(
        InstrumentType::SteppedCouponBond,
        pricer,
        vec![PricingModelConfiguration::new(
            MarketIndex::SOFR,
            ModelConfiguration::HullWhite {
                alpha: 0.05,
                parameter_source: ParameterSource::Calibrated(
                    ModelCalibrationConfiguration::new(CalibrationSource::Surface {
                        market_index: MarketIndex::SOFR,
                    })
                    .with_strike(Strike::Atm),
                ),
            },
        )],
    );
    let mut context = PricingContext::new()
        .with_quote_store(quotes)
        .with_curve_configurations(curves)
        .with_volatility_surface_configurations(surfaces)
        .with_pricer_configurations(vec![configuration]);
    context.initialize()?;
    Ok(context)
}

struct RecordingProvider<'a> {
    context: &'a PricingContext,
    calibrated_request_seen: RefCell<bool>,
}

impl MarketDataProvider for RecordingProvider<'_> {
    fn evaluation_date(&self) -> Date {
        self.context.evaluation_date()
    }
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
        if request.calibration_request().is_some() {
            let elements = request
                .constructed_elements_request()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibration elements request".into()))?;
            assert!(elements.iter().any(|element| matches!(
                element,
                ConstructedElementRequest::VolatilitySurface {
                    market_index: MarketIndex::SOFR
                }
            )));
            assert!(elements.iter().any(|element| matches!(
                element,
                ConstructedElementRequest::DiscountCurve {
                    market_index: MarketIndex::SOFR
                }
            )));
            *self.calibrated_request_seen.borrow_mut() = true;
        }
        self.context.handle_request(request)
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn calibrated_surface_native_engines_preserve_schedule_and_quote_risk() -> Result<()> {
    let quote = "CapletFloorlet_USD_SOFR_3M_2Y_Absolute_0.045_Straddle_Black";
    let bump = 1.0e-4;
    let mut prices = Vec::new();
    for use_fourier in [false, true] {
        Tape::rewind_to_init_fwd();
        let context = example_calibrated_context(use_fourier, None)?;
        let market = MarketData::new(HashMap::new(), context.constructed_elements().clone())
            .with_calibration_quotes(context.quote_store().clone())
            .with_pricer_configurations(HashMap::from([(
                InstrumentType::SteppedCouponBond,
                context.pricer_configurations()[0].clone(),
            )]));
        let configuration = market
            .pricer_configurations()
            .get(&InstrumentType::SteppedCouponBond)
            .ok_or_else(|| QSError::NotFoundErr("Stepped bond configuration".into()))?
            .model(&MarketIndex::SOFR)
            .ok_or_else(|| QSError::NotFoundErr("Hull-White model".into()))?
            .clone();
        let mut resolved = HullWhiteParameters::new(configuration.clone())?;
        resolved.resolve_calibration(&market, &MarketIndex::SOFR)?;
        assert_eq!(resolved.sigma_schedule.len(), 9);
        assert!(resolved
            .sigma_schedule
            .windows(2)
            .any(|entries| (entries[0].1 - entries[1].1).abs() > 1.0e-5));
        let curve = market
            .constructed_elements()
            .discount_curve(&MarketIndex::SOFR)
            .ok_or_else(|| QSError::NotFoundErr("SOFR curve".into()))?
            .curve();
        let primal = PrimalCurve(&*curve);
        let ModelConfiguration::HullWhite {
            parameter_source, ..
        } = &configuration
        else {
            return Err(QSError::UnexpectedErr(
                "Expected Hull-White configuration".into(),
            ));
        };
        let lgm = LgmRateModel::from_configuration(
            &ModelConfiguration::Lgm {
                lambda: 0.05,
                parameter_source: parameter_source.clone(),
            },
            &primal,
            market.constructed_elements(),
            context.quote_store(),
            Level::Mid,
        )?;
        assert_eq!(resolved.sigma_schedule, lgm.sigma_schedule());
        drop(curve);
        let evaluation = context.evaluation_date();
        let trade = bond(
            evaluation,
            evaluation + 6 * 365,
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::window(
                evaluation + 3 * 365,
                evaluation + 5 * 365,
                100.0,
            )],
        );
        let provider = RecordingProvider {
            context: &context,
            calibrated_request_seen: RefCell::new(false),
        };
        let evaluate = |provider: &RecordingProvider<'_>, requests: &[Request]| {
            if use_fourier {
                fourier_pricer().evaluate(
                    &trade
                        .clone()
                        .with_pricing_anchor(BondPricingAnchor::OAS(0.002)),
                    requests,
                    provider,
                )
            } else {
                tree_pricer().evaluate(
                    &trade
                        .clone()
                        .with_pricing_anchor(BondPricingAnchor::OAS(0.002)),
                    requests,
                    provider,
                )
            }
        };
        let results = evaluate(&provider, &[Request::Value, Request::Sensitivities])?;
        assert!(*provider.calibrated_request_seen.borrow());
        let price = results
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Calibrated bond price".into()))?;
        assert!(price.is_finite() && price > 0.0);
        prices.push(price);
        let risk = results
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Calibrated bond sensitivities".into()))?;
        assert!(risk
            .instrument_keys()
            .iter()
            .any(|label| label.starts_with("OIS_")));
        assert!(risk
            .instrument_keys()
            .iter()
            .any(|label| label == "TEST-BOND.OAS"));
        let adjoint = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .find_map(|(label, value)| (label == quote).then_some(*value))
            .ok_or_else(|| QSError::NotFoundErr("Calibrated caplet quote risk".into()))?;
        assert!(
            adjoint.abs() > 1.0e-4,
            "vol quote sensitivity must not be discarded"
        );
        let mut bumped_prices = Vec::new();
        for amount in [bump, -bump] {
            Tape::rewind_to_init_fwd();
            let bumped = example_calibrated_context(use_fourier, Some((quote, amount)))?;
            let bumped_provider = RecordingProvider {
                context: &bumped,
                calibrated_request_seen: RefCell::new(false),
            };
            bumped_prices.push(
                evaluate(&bumped_provider, &[Request::Value])?
                    .price()
                    .ok_or_else(|| QSError::ValueNotSetErr("Bumped bond price".into()))?,
            );
        }
        let finite_difference = (bumped_prices[0] - bumped_prices[1]) / (2.0 * bump);
        eprintln!("Calibrated HullWhite Fourier={use_fourier}: PV={price:.8}, caplet AD={adjoint:.8}, rebuilt-market bump={finite_difference:.8}");
        assert!(
            (adjoint - finite_difference).abs() < 0.03 * finite_difference.abs().max(0.01),
            "Fourier={use_fourier}: AD {adjoint} vs rebuilt surface bump {finite_difference}"
        );
    }
    assert!(
        (prices[0] - prices[1]).abs() < 0.20,
        "calibrated tree {} vs Fourier {}",
        prices[0],
        prices[1]
    );
    Ok(())
}

#[test]
fn calibrated_cube_native_engines_report_swaption_risk() -> Result<()> {
    for use_fourier in [false, true] {
        Tape::rewind_to_init_fwd();
        let evaluation = Date::new(2025, 1, 1);
        let (base, _) = provider_with_method(evaluation, 24, use_fourier);
        let mut quotes = QuoteStore::new(evaluation);
        let mut identifiers = Vec::new();
        for expiry in ["1Y", "2Y", "3Y"] {
            for tenor in ["5Y", "10Y"] {
                for strike in ["0.02", "0.06"] {
                    let identifier =
                        format!("Swaption_USD_SOFR_{expiry}_{tenor}_Absolute_{strike}_Black");
                    quotes.add_quote(Quote::new(
                        identifier.parse()?,
                        QuoteLevels::with_mid(if expiry == "1Y" { 0.20 } else { 0.30 }),
                    ));
                    identifiers.push(identifier);
                }
            }
        }
        let time_grid = TimeGridConfiguration::new(24, 4096, DayCounter::Actual365);
        let implementation = if use_fourier {
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid: FourierGridConfiguration::new(256, 10.0),
            }
        } else {
            PricerImplementation::TreeBased { time_grid }
        };
        let calibration = ModelCalibrationConfiguration::new(CalibrationSource::Cube {
            market_index: MarketIndex::SOFR,
        })
        .with_calibration_basket(
            CalibrationBasket::all()
                .with_expiries(vec![
                    Period::new(1, TimeUnit::Years),
                    Period::new(2, TimeUnit::Years),
                ])
                .with_tenors(vec![Period::new(5, TimeUnit::Years)])
                .with_strike(Strike::Atm),
        );
        let configuration = PricerConfiguration::new(
            InstrumentType::SteppedCouponBond,
            implementation,
            vec![PricingModelConfiguration::new(
                MarketIndex::SOFR,
                ModelConfiguration::HullWhite {
                    alpha: ALPHA,
                    parameter_source: ParameterSource::Calibrated(calibration),
                },
            )],
        );
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
            .with_constructed_elements(base.market_data.constructed_elements().clone())
            .with_volatility_cube_configurations(vec![VolatilityCubeConfiguration::new(
                MarketIndex::SOFR,
                VolatilityType::Black,
                SmileType::Strike,
                identifiers,
            )])
            .with_pricer_configurations(vec![configuration]);
        context.initialize()?;
        let trade = bond(
            evaluation,
            evaluation + 4 * 365,
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(evaluation + 3 * 365, 100.0)],
        );
        let results = if use_fourier {
            fourier_pricer().evaluate(
                &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
                &[Request::Value, Request::Sensitivities],
                &context,
            )?
        } else {
            tree_pricer().evaluate(
                &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
                &[Request::Value, Request::Sensitivities],
                &context,
            )?
        };
        assert!(results
            .price()
            .is_some_and(|price| price.is_finite() && price > 0.0));
        let risk = results
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Cube bond sensitivities".into()))?;
        assert!(risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .any(|(label, exposure)| label.starts_with("Swaption_") && exposure.abs() > 1.0e-4));
    }
    Tape::rewind_to_init_fwd();
    Ok(())
}

fn policy() -> Box<dyn DiscountPolicy> {
    Box::new(
        FixedIncomeDiscountPolicy::new(false)
            .with_risk_free_index(Currency::USD, MarketIndex::SOFR),
    )
}

fn fixed_pricer() -> BondPricer {
    let mut pricer = BondPricer::new();
    pricer.set_discount_policy(policy());
    pricer
}

fn tree_pricer() -> TreeBasedBondPricer {
    let mut pricer = TreeBasedBondPricer::new();
    pricer.set_discount_policy(policy());
    pricer
}

fn fourier_pricer() -> BackwardFourierEvolutionBondPricer {
    let mut pricer = BackwardFourierEvolutionBondPricer::new();
    pricer.set_discount_policy(policy());
    pricer
}

#[derive(Clone)]
struct TestProvider {
    evaluation_date: Date,
    market_data: MarketData,
}

impl MarketDataProvider for TestProvider {
    fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
        Ok(self.market_data.clone())
    }

    fn evaluation_date(&self) -> Date {
        self.evaluation_date
    }
}

fn provider(
    evaluation_date: Date,
    steps_per_year: usize,
) -> (TestProvider, FlatForwardTermStructure<f64>) {
    provider_with_method(evaluation_date, steps_per_year, false)
}

fn fourier_provider(
    evaluation_date: Date,
    steps_per_year: usize,
) -> (TestProvider, FlatForwardTermStructure<f64>) {
    provider_with_method(evaluation_date, steps_per_year, true)
}

fn provider_with_method(
    evaluation_date: Date,
    steps_per_year: usize,
    use_fourier: bool,
) -> (TestProvider, FlatForwardTermStructure<f64>) {
    let definition = RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    );
    let ad_curve = FlatForwardTermStructure::new(evaluation_date, DualFwd::from(RATE), definition);
    let benchmark_curve = FlatForwardTermStructure::new(evaluation_date, RATE, definition);
    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(ad_curve))),
    );
    let time_grid = TimeGridConfiguration::new(steps_per_year, 4096, DayCounter::Actual365);
    let pricer = if use_fourier {
        PricerImplementation::BackwardFourierEvolution {
            time_grid,
            fourier_grid: FourierGridConfiguration::new(256, 10.0),
        }
    } else {
        PricerImplementation::TreeBased { time_grid }
    };
    let configuration = PricerConfiguration::new(
        InstrumentType::SteppedCouponBond,
        pricer,
        vec![PricingModelConfiguration::new(
            MarketIndex::SOFR,
            ModelConfiguration::HullWhite {
                alpha: ALPHA,
                parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(SIGMA)),
            },
        )],
    );
    let market_data = MarketData::new(HashMap::new(), elements).with_pricer_configurations(
        HashMap::from([(InstrumentType::SteppedCouponBond, configuration)]),
    );
    (
        TestProvider {
            evaluation_date,
            market_data,
        },
        benchmark_curve,
    )
}

fn bond(
    evaluation_date: Date,
    maturity_date: Date,
    coupon: f64,
    _structure: PaymentStructure,
    call_schedule: Vec<CallScheduleEntry>,
) -> SteppedCouponBondTrade<DualFwd> {
    let pricing_anchor = if call_schedule.is_empty() {
        BondPricingAnchor::ZSpread(0.0)
    } else {
        BondPricingAnchor::OAS(0.0)
    };
    let instrument = MakeSteppedCouponBond::<DualFwd>::default()
        .with_identifier("TEST-BOND".into())
        .with_start_date(evaluation_date)
        .with_maturity_date(maturity_date)
        .with_units(FACE)
        .with_notional(FACE)
        .with_currency(Currency::USD)
        .with_payment_frequency(Frequency::Semiannual)
        .with_coupons(vec![CashflowType::fixed(coupon)])
        .with_notional_percentages(vec![1.0])
        .with_call_schedule(call_schedule)
        .build()
        .unwrap_or_else(|error| panic!("test bond: {error}"));
    SteppedCouponBondTrade::new(
        instrument,
        evaluation_date,
        FACE,
        crate::core::trade::Side::LongReceive,
        pricing_anchor,
    )
}

fn value(
    pricer: &TreeBasedBondPricer,
    trade: &SteppedCouponBondTrade<DualFwd>,
    provider: &TestProvider,
) -> f64 {
    pricer
        .evaluate(trade, &[Request::Value], provider)
        .unwrap_or_else(|error| panic!("bond value: {error}"))
        .price()
        .unwrap_or_else(|| panic!("bond value must be present"))
}

fn fixed_bond(
    evaluation_date: Date,
    maturity_date: Date,
    coupon: f64,
) -> FixedRateBondTrade<DualFwd> {
    let definition = RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Compounded,
        Frequency::Semiannual,
    );
    let instrument = MakeFixedRateBond::<DualFwd>::default()
        .with_identifier("TEST-BOND".into())
        .with_start_date(evaluation_date)
        .with_maturity_date(maturity_date)
        .with_rate(coupon)
        .with_units(FACE)
        .with_notional(FACE)
        .with_rate_definition(definition)
        .with_discount_index(MarketIndex::SOFR)
        .with_currency(Currency::USD)
        .with_payment_frequency(Frequency::Semiannual)
        .with_payment_structure(PaymentStructure::Bullet)
        .build()
        .unwrap_or_else(|error| panic!("test fixed bond: {error}"));
    FixedRateBondTrade::new(
        instrument,
        evaluation_date,
        FACE,
        crate::core::trade::Side::LongReceive,
        BondPricingAnchor::ZSpread(0.0),
    )
}

fn fixed_value(
    pricer: &BondPricer,
    trade: &FixedRateBondTrade<DualFwd>,
    provider: &TestProvider,
) -> f64 {
    pricer
        .evaluate(trade, &[Request::Value], provider)
        .unwrap_or_else(|error| panic!("fixed bond value: {error}"))
        .price()
        .unwrap_or_else(|| panic!("fixed bond value must be present"))
}

fn fourier_value(
    pricer: &BackwardFourierEvolutionBondPricer,
    trade: &SteppedCouponBondTrade<DualFwd>,
    provider: &TestProvider,
) -> f64 {
    pricer
        .evaluate(trade, &[Request::Value], provider)
        .unwrap_or_else(|error| panic!("Fourier bond value: {error}"))
        .price()
        .unwrap_or_else(|| panic!("Fourier bond value must be present"))
}

#[test]
#[allow(clippy::too_many_lines)]
fn discount_policy_controls_curve_requests_and_bond_values() -> Result<()> {
    let evaluation = Date::new(2025, 1, 1);
    let maturity = Date::new(2027, 1, 1);
    let selected_index = MarketIndex::Other("USD-TREASURY".into());
    let definition = RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    );
    let mut elements = ConstructedElementStore::default();
    for (index, rate) in [(MarketIndex::SOFR, RATE), (selected_index.clone(), 0.08)] {
        elements.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(
                index,
                Rc::new(RefCell::new(FlatForwardTermStructure::new(
                    evaluation,
                    DualFwd::from(rate),
                    definition,
                ))),
            ),
        );
    }
    let market = TestProvider {
        evaluation_date: evaluation,
        market_data: MarketData::new(HashMap::new(), elements),
    };
    let selected_policy = || -> Box<dyn DiscountPolicy> {
        Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_risk_free_index(Currency::USD, selected_index.clone()),
        )
    };
    let expected = FACE * (-0.08_f64 * 2.0).exp();
    let straight = fixed_bond(evaluation, maturity, 0.0);
    let stepped = bond(evaluation, maturity, 0.0, PaymentStructure::Zero, vec![]);
    let mut fixed = BondPricer::new();
    let mut tree = TreeBasedBondPricer::new();
    let mut fourier = BackwardFourierEvolutionBondPricer::new();
    assert!(fixed
        .evaluate(
            &straight
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
            &[Request::Value],
            &market
        )
        .is_err());
    assert!(tree
        .evaluate(
            &stepped
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
            &[Request::Value],
            &market
        )
        .is_err());
    assert!(fourier
        .evaluate(
            &stepped
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
            &[Request::Value],
            &market
        )
        .is_err());
    fixed.set_discount_policy(selected_policy());
    tree.set_discount_policy(selected_policy());
    fourier.set_discount_policy(selected_policy());
    for request in [
        fixed.market_data_request(&straight),
        tree.market_data_request(&stepped),
        fourier.market_data_request(&stepped),
    ] {
        let request = request.ok_or_else(|| QSError::NotFoundErr("bond curve request".into()))?;
        assert!(
            matches!(request.constructed_elements_request().and_then(|elements| elements.first()),
            Some(ConstructedElementRequest::DiscountCurve { market_index }) if market_index == &selected_index)
        );
    }
    for price in [
        fixed_value(
            &fixed,
            &straight
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
            &market,
        ),
        value(
            &tree,
            &stepped
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
            &market,
        ),
        fourier_value(
            &fourier,
            &stepped.with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
            &market,
        ),
    ] {
        assert!(
            (price - expected).abs() < 1.0e-10,
            "policy-selected {price} vs analytic {expected}"
        );
    }
    assert!(
        expected
            < fixed_value(
                &fixed_pricer(),
                &straight.with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &market
            )
    );
    Ok(())
}

#[test]
fn z_spread_price_anchor_round_trip_duration_and_risk() {
    let evaluation_date = Date::new(2025, 1, 1);
    let (provider, _) = provider(evaluation_date, 48);
    let trade = fixed_bond(evaluation_date, Date::new(2030, 1, 1), 0.04);
    let spread = 0.0125;
    let direct = fixed_pricer();
    let direct_results = direct
        .evaluate(
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::ZSpread(spread)),
            &[
                Request::Value,
                Request::ZSpread,
                Request::YieldToMaturity,
                Request::ModifiedDuration,
                Request::Sensitivities,
            ],
            &provider,
        )
        .unwrap_or_else(|error| panic!("direct bond price: {error}"));
    let direct_value = direct_results
        .price()
        .unwrap_or_else(|| panic!("price result"));
    assert_eq!(direct_results.z_spread(), Some(spread));
    assert!(direct_results.ytm().is_some_and(f64::is_finite));
    assert!(direct_results
        .modified_duration()
        .is_some_and(|duration| duration > 0.0));

    let quoted_price = direct_value / FACE * FACE;
    let implied = fixed_pricer();
    let implied_results = implied
        .evaluate(
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::DirtyPrice(quoted_price)),
            &[Request::Value, Request::ZSpread],
            &provider,
        )
        .unwrap_or_else(|error| panic!("price-anchored bond: {error}"));
    assert!((implied_results.z_spread().unwrap_or_default() - spread).abs() < 1.0e-10);
    assert!((implied_results.price().unwrap_or_default() - direct_value).abs() < 1.0e-9);

    let risk = direct_results
        .sensitivities()
        .unwrap_or_else(|| panic!("sensitivity result"));
    let spread_risk = risk
        .instrument_keys()
        .iter()
        .zip(risk.exposure())
        .find_map(|(label, exposure)| (label == "TEST-BOND.ZSpread").then_some(*exposure))
        .unwrap_or_else(|| panic!("Z-spread sensitivity"));
    let bump = 1.0e-6;
    let finite_difference = (fixed_value(
        &fixed_pricer(),
        &trade
            .clone()
            .with_pricing_anchor(BondPricingAnchor::ZSpread(spread + bump)),
        &provider,
    ) - fixed_value(
        &fixed_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::ZSpread(spread - bump)),
        &provider,
    )) / (2.0 * bump);
    assert!((spread_risk - finite_difference).abs() < 1.0e-4);
}

fn callable_zero_coupon_value(steps_per_year: usize) -> (f64, f64) {
    let evaluation_date = Date::new(2025, 1, 1);
    let call_date = Date::new(2026, 1, 1);
    let maturity_date = Date::new(2027, 1, 1);
    let (provider, curve) = provider(evaluation_date, steps_per_year);
    let strike = 0.95;
    let trade = bond(
        evaluation_date,
        maturity_date,
        0.0,
        PaymentStructure::Zero,
        vec![CallScheduleEntry::new(call_date, strike * FACE)],
    );
    let lattice = value(
        &tree_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &provider,
    );
    let model = HullWhite::new(ALPHA, &curve).with_constant_volatility(SIGMA);
    let option = model
        .bond_call_price(1.0, 2.0, strike, SIGMA, &curve)
        .unwrap_or_else(|error| panic!("closed-form bond call: {error}"));
    let benchmark = FACE
        * (curve
            .discount_factor_from_time(2.0)
            .unwrap_or_else(|error| panic!("discount factor: {error}"))
            - option);
    (lattice, benchmark)
}

#[test]
fn callable_zero_coupon_matches_hull_white_bond_option_formula() {
    let (lattice, benchmark) = callable_zero_coupon_value(192);
    assert!(
        (lattice - benchmark).abs() < 0.08,
        "{lattice} vs {benchmark}"
    );
}

#[test]
fn backward_fourier_zero_coupon_matches_hull_white_bond_option_formula() {
    let evaluation_date = Date::new(2025, 1, 1);
    let maturity_date = Date::new(2027, 1, 1);
    let (provider, curve) = fourier_provider(evaluation_date, 96);
    let strike = 0.95;
    let trade = bond(
        evaluation_date,
        maturity_date,
        0.0,
        PaymentStructure::Zero,
        vec![CallScheduleEntry::new(Date::new(2026, 1, 1), strike * FACE)],
    );
    let fourier = fourier_value(
        &fourier_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &provider,
    );
    let model = HullWhite::new(ALPHA, &curve).with_constant_volatility(SIGMA);
    let option = model
        .bond_call_price(1.0, 2.0, strike, SIGMA, &curve)
        .unwrap_or_else(|error| panic!("closed-form bond call: {error}"));
    let benchmark = FACE
        * (curve
            .discount_factor_from_time(2.0)
            .unwrap_or_else(|error| panic!("discount factor: {error}"))
            - option);
    assert!(
        (fourier - benchmark).abs() < 0.10,
        "{fourier} vs {benchmark}"
    );
}

#[test]
fn call_windows_cover_interior_grid_points_and_active_settlement() -> Result<()> {
    let start = Date::new(2025, 1, 1);
    let maturity = Date::new(2027, 1, 1);
    let trade = bond(start, maturity, 0.0, PaymentStructure::Zero, vec![]);
    let cashflows = vec![ResolvedCashflow {
        payment_date: maturity,
        amount: FACE,
    }];
    let terms = bond_event_grid(
        trade.bond_leg(),
        FACE,
        &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
        &cashflows,
        start,
        TimeGridConfiguration::new(2, 4, DayCounter::Actual365),
    )?;
    assert_eq!(terms.steps, 4);
    assert_eq!(
        terms
            .calls
            .iter()
            .map(|call| call.call_price)
            .collect::<Vec<_>>(),
        vec![None, Some(95.0), Some(95.0), Some(95.0), None]
    );
    let active = bond_event_grid(
        trade.bond_leg(),
        FACE,
        &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
        &cashflows,
        start + 365,
        TimeGridConfiguration::new(4, 4, DayCounter::Actual365),
    )?;
    assert_eq!(active.calls[0].call_price, Some(95.0));
    let amortizing = MakeSteppedCouponBond::<DualFwd>::default()
        .with_identifier("USD-AMORTIZING-WINDOW".into())
        .with_start_date(start)
        .with_maturity_date(maturity)
        .with_notional(FACE)
        .with_currency(Currency::USD)
        .with_coupons(vec![CashflowType::fixed(0.0)])
        .with_notional_percentages(vec![1.0, 0.8])
        .build()?;
    let amortizing_terms = bond_event_grid(
        amortizing.leg(),
        FACE,
        &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
        &cashflows,
        start,
        TimeGridConfiguration::new(2, 4, DayCounter::Actual365),
    )?;
    assert_eq!(amortizing_terms.calls[1].call_price, Some(76.0));
    assert_eq!(amortizing_terms.calls[2].call_price, Some(76.0));
    Ok(())
}

#[test]
fn interior_window_exercise_reduces_price_in_both_engines() {
    let evaluation = Date::new(2025, 1, 1);
    let first = Date::new(2026, 1, 1);
    let last = Date::new(2028, 1, 1);
    let maturity = Date::new(2030, 1, 1);
    let endpoints = bond(
        evaluation,
        maturity,
        0.045,
        PaymentStructure::Bullet,
        vec![
            CallScheduleEntry::new(first, 100.0),
            CallScheduleEntry::new(last, 100.0),
        ],
    );
    let window = bond(
        evaluation,
        maturity,
        0.045,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::window(first, last, 100.0)],
    );
    let (tree_market, _) = provider(evaluation, 96);
    let (fourier_market, _) = fourier_provider(evaluation, 96);
    let tree = tree_pricer();
    let fourier = fourier_pricer();
    let tree_window = value(
        &tree,
        &window
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &tree_market,
    );
    let tree_endpoints = value(
        &tree,
        &endpoints
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &tree_market,
    );
    let fourier_window = fourier_value(
        &fourier,
        &window.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &fourier_market,
    );
    let fourier_endpoints = fourier_value(
        &fourier,
        &endpoints.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &fourier_market,
    );
    assert!(
        tree_window < tree_endpoints - 1.0e-4,
        "{tree_window} vs {tree_endpoints}"
    );
    assert!(
        fourier_window < fourier_endpoints - 1.0e-4,
        "{fourier_window} vs {fourier_endpoints}"
    );
    assert!(
        (tree_window - fourier_window).abs() < 0.15,
        "window tree {tree_window} vs Fourier {fourier_window}"
    );
}

#[test]
fn tree_and_backward_fourier_prices_agree() {
    let evaluation_date = Date::new(2025, 1, 1);
    let trade = bond(
        evaluation_date,
        Date::new(2032, 1, 1),
        0.045,
        PaymentStructure::Bullet,
        vec![
            CallScheduleEntry::new(Date::new(2028, 1, 1), 101.0),
            CallScheduleEntry::new(Date::new(2030, 1, 1), 100.0),
        ],
    );
    let (tree_provider, _) = provider(evaluation_date, 96);
    let (fourier_provider, _) = fourier_provider(evaluation_date, 96);
    let tree = value(
        &tree_pricer(),
        &trade
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(0.006)),
        &tree_provider,
    );
    let fourier = fourier_value(
        &fourier_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.006)),
        &fourier_provider,
    );
    assert!((tree - fourier).abs() < 0.12, "{tree} vs {fourier}");
}

#[test]
fn engines_agree_for_positional_mixed_coupons_and_amortization() {
    let evaluation_date = Date::new(2025, 1, 1);
    let instrument = MakeSteppedCouponBond::<DualFwd>::default()
        .with_identifier("MIXED-STEP-BOND".into())
        .with_start_date(evaluation_date)
        .with_maturity_date(Date::new(2031, 1, 1))
        .with_units(FACE)
        .with_notional(FACE)
        .with_currency(Currency::USD)
        .with_payment_frequency(Frequency::Semiannual)
        .with_coupons(vec![
            CashflowType::fixed(0.045),
            CashflowType::floating(MarketIndex::SOFR, 0.006, Some(0.0), Some(0.06)),
        ])
        .with_notional_percentages(vec![1.0, 0.9, 0.75])
        .with_call_schedule(vec![
            CallScheduleEntry::new(Date::new(2028, 1, 1), 101.0),
            CallScheduleEntry::new(Date::new(2029, 1, 1), 100.0),
        ])
        .build()
        .unwrap_or_else(|error| panic!("mixed stepped bond: {error}"));
    let trade = SteppedCouponBondTrade::new(
        instrument,
        evaluation_date,
        FACE,
        crate::core::trade::Side::LongReceive,
        BondPricingAnchor::OAS(0.0),
    );
    let (tree_provider, _) = provider(evaluation_date, 96);
    let (fourier_provider, _) = fourier_provider(evaluation_date, 96);
    let tree = value(
        &tree_pricer(),
        &trade
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(0.0075)),
        &tree_provider,
    );
    let fourier = fourier_value(
        &fourier_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0075)),
        &fourier_provider,
    );
    assert!(tree.is_finite() && fourier.is_finite());
    assert!((tree - fourier).abs() < 0.15, "{tree} vs {fourier}");
}

#[test]
fn stepped_coupon_bond_rejects_yield_analytics() {
    let evaluation_date = Date::new(2025, 1, 1);
    let (provider, _) = provider(evaluation_date, 24);
    let trade = bond(
        evaluation_date,
        Date::new(2030, 1, 1),
        0.05,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0)],
    );
    let pricer = tree_pricer();
    for request in [Request::YieldToMaturity, Request::ModifiedDuration] {
        let Err(error) = pricer.evaluate(
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &[request],
            &provider,
        ) else {
            panic!("stepped bond yield analytics must be rejected");
        };
        assert!(error
            .to_string()
            .contains("not defined for SteppedCouponBond"));
    }
}

#[test]
fn backward_fourier_oas_sensitivity_is_in_common_table() {
    let evaluation_date = Date::new(2025, 1, 1);
    let (provider, _) = fourier_provider(evaluation_date, 24);
    let trade = bond(
        evaluation_date,
        Date::new(2028, 1, 1),
        0.04,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::new(Date::new(2026, 7, 1), 100.0)],
    );
    let oas = 0.005;
    let results = fourier_pricer()
        .evaluate(
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(oas)),
            &[Request::Value, Request::Sensitivities],
            &provider,
        )
        .unwrap_or_else(|error| panic!("Fourier sensitivities: {error}"));
    let oas_risk = results
        .sensitivities()
        .unwrap_or_else(|| panic!("Fourier sensitivity result"))
        .instrument_keys()
        .iter()
        .zip(
            results
                .sensitivities()
                .unwrap_or_else(|| panic!("Fourier sensitivity result"))
                .exposure(),
        )
        .find_map(|(label, exposure)| (label == "TEST-BOND.OAS").then_some(*exposure))
        .unwrap_or_else(|| panic!("Fourier OAS sensitivity"));
    let bump = 1.0e-5;
    let finite_difference = (fourier_value(
        &fourier_pricer(),
        &trade
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(oas + bump)),
        &provider,
    ) - fourier_value(
        &fourier_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::OAS(oas - bump)),
        &provider,
    )) / (2.0 * bump);
    assert!((oas_risk - finite_difference).abs() < 0.05);
}

#[test]
fn lattice_ladder_converges_towards_closed_form() {
    let (coarse, benchmark) = callable_zero_coupon_value(12);
    let (fine, _) = callable_zero_coupon_value(192);
    assert!((fine - benchmark).abs() < (coarse - benchmark).abs());
    assert!((fine - benchmark).abs() < 0.15);
}

#[test]
fn call_boundaries_and_extreme_oas_are_monotone() {
    let evaluation_date = Date::new(2025, 1, 1);
    let call_date = Date::new(2027, 1, 1);
    let maturity_date = Date::new(2030, 1, 1);
    let (provider, _) = provider(evaluation_date, 48);
    let low_call = bond(
        evaluation_date,
        maturity_date,
        0.05,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::new(call_date, 70.0)],
    );
    let high_call = bond(
        evaluation_date,
        maturity_date,
        0.05,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::new(call_date, 200.0)],
    );
    let low_call_value = value(
        &tree_pricer(),
        &low_call.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &provider,
    );
    let high_call_value = value(
        &tree_pricer(),
        &high_call
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
        &provider,
    );
    assert!(low_call_value < high_call_value);

    let negative = value(
        &tree_pricer(),
        &high_call
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(-0.05)),
        &provider,
    );
    let zero = high_call_value;
    let positive = value(
        &tree_pricer(),
        &high_call.with_pricing_anchor(BondPricingAnchor::OAS(0.20)),
        &provider,
    );
    assert!(negative.is_finite() && zero.is_finite() && positive.is_finite());
    assert!(negative > zero && zero > positive);
}

#[test]
fn oas_price_anchor_round_trip_and_sensitivity_share_the_risk_table() {
    let evaluation_date = Date::new(2025, 1, 1);
    let (provider, _) = provider(evaluation_date, 24);
    let trade = bond(
        evaluation_date,
        Date::new(2030, 1, 1),
        0.05,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0)],
    );
    let oas = 0.0075;
    let direct = tree_pricer();
    let results = direct
        .evaluate(
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(oas)),
            &[Request::Value, Request::OAS, Request::Sensitivities],
            &provider,
        )
        .unwrap_or_else(|error| panic!("callable bond: {error}"));
    assert_eq!(results.oas(), Some(oas));
    let direct_value = results.price().unwrap_or_default();
    let implied = tree_pricer();
    let implied_results = implied
        .evaluate(
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::DirtyPrice(direct_value)),
            &[Request::OAS],
            &provider,
        )
        .unwrap_or_else(|error| panic!("OAS solve: {error}"));
    assert!((implied_results.oas().unwrap_or_default() - oas).abs() < 1.0e-9);

    let risk = results
        .sensitivities()
        .unwrap_or_else(|| panic!("callable sensitivity result"));
    let oas_risk = risk
        .instrument_keys()
        .iter()
        .zip(risk.exposure())
        .find_map(|(label, exposure)| (label == "TEST-BOND.OAS").then_some(*exposure))
        .unwrap_or_else(|| panic!("OAS sensitivity"));
    let bump = 1.0e-5;
    let finite_difference = (value(
        &tree_pricer(),
        &trade
            .clone()
            .with_pricing_anchor(BondPricingAnchor::OAS(oas + bump)),
        &provider,
    ) - value(
        &tree_pricer(),
        &trade.with_pricing_anchor(BondPricingAnchor::OAS(oas - bump)),
        &provider,
    )) / (2.0 * bump);
    assert!((oas_risk - finite_difference).abs() < 0.02);
}

#[test]
fn rejects_mismatched_spread_family_and_bad_call_terms() {
    let evaluation_date = Date::new(2025, 1, 1);
    let (provider, _) = provider(evaluation_date, 24);
    let straight = bond(
        evaluation_date,
        Date::new(2028, 1, 1),
        0.04,
        PaymentStructure::Bullet,
        Vec::new(),
    );
    assert!(tree_pricer()
        .evaluate(
            &straight.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &[Request::Value],
            &provider
        )
        .is_err());
    assert!(CallScheduleEntry::new(Date::new(2026, 1, 1), 0.0)
        .validate()
        .is_err());
}
#[test]
fn trade_owns_anchor_settlement_and_side_across_scalar_conversion() -> Result<()> {
    let evaluation = Date::new(2025, 1, 1);
    let settlement = Date::new(2025, 7, 1);
    let maturity = Date::new(2027, 1, 1);
    let (provider, _) = provider(evaluation, 12);
    let base = fixed_bond(evaluation, maturity, 0.0);
    let long = FixedRateBondTrade::new(
        base.instrument().clone(),
        evaluation,
        FACE,
        Side::LongReceive,
        BondPricingAnchor::DirtyPrice(97.0),
    )
    .with_settlement_date(settlement)
    .with_spread_day_counter(DayCounter::Actual360);
    let scalar: FixedRateBondTrade<f64> = long.clone().into();
    let roundtrip: FixedRateBondTrade<DualFwd> = scalar.into();
    assert_eq!(roundtrip.pricing_anchor(), long.pricing_anchor());
    assert_eq!(roundtrip.settlement_date(), Some(settlement));
    assert_eq!(roundtrip.spread_day_counter(), Some(DayCounter::Actual360));
    let short = FixedRateBondTrade::new(
        base.instrument().clone(),
        evaluation,
        FACE,
        Side::PayShort,
        long.pricing_anchor(),
    )
    .with_settlement_date(settlement)
    .with_spread_day_counter(DayCounter::Actual360);
    let pricer = fixed_pricer();
    let requests = [Request::Value, Request::ZSpread, Request::Sensitivities];
    let long_result = pricer.evaluate(&roundtrip, &requests, &provider)?;
    let short_result = pricer.evaluate(&short, &requests, &provider)?;
    assert!(
        (long_result.price().unwrap_or_default() + short_result.price().unwrap_or_default()).abs()
            < 1.0e-10
    );
    assert_eq!(long_result.z_spread(), short_result.z_spread());
    let spread = long_result
        .z_spread()
        .ok_or_else(|| QSError::ValueNotSetErr("Z-spread".into()))?;
    let spread_result = pricer.evaluate(
        &long.with_pricing_anchor(BondPricingAnchor::ZSpread(spread)),
        &requests,
        &provider,
    )?;
    assert!(
        (spread_result.price().unwrap_or_default() - long_result.price().unwrap_or_default()).abs()
            < 1.0e-9
    );
    let long_risk = long_result
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("Long risk".into()))?;
    let short_risk = short_result
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("Short risk".into()))?;
    for (left, right) in long_risk.exposure().iter().zip(short_risk.exposure()) {
        assert!((left + right).abs() < 1.0e-9);
    }
    let short_scalar: FixedRateBondTrade<f64> = short.into();
    assert!(short_scalar
        .into_contingent_claims("SHORT")?
        .iter()
        .all(|claim| claim.side() == Side::PayShort));
    assert_eq!(base.instrument().leg().side(), Side::LongReceive);
    Ok(())
}

#[test]
fn callable_pricers_reuse_trade_terms_and_apply_side_once() -> Result<()> {
    let evaluation = Date::new(2025, 1, 1);
    let long = bond(
        evaluation,
        Date::new(2027, 1, 1),
        0.04,
        PaymentStructure::Bullet,
        vec![CallScheduleEntry::new(Date::new(2026, 1, 1), 99.0)],
    )
    .with_pricing_anchor(BondPricingAnchor::OAS(0.003));
    let short = SteppedCouponBondTrade::new(
        long.instrument().clone(),
        evaluation,
        FACE,
        Side::PayShort,
        long.pricing_anchor(),
    );
    let scalar: SteppedCouponBondTrade<f64> = short.clone().into();
    assert!(scalar
        .into_contingent_claims("SHORT-CALLABLE")?
        .iter()
        .all(|claim| claim.side() == Side::PayShort));
    let converted: SteppedCouponBondTrade<DualFwd> = scalar.into();
    assert_eq!(converted.pricing_anchor(), short.pricing_anchor());
    for use_fourier in [false, true] {
        let (provider, _) = provider_with_method(evaluation, 12, use_fourier);
        let tree = tree_pricer();
        let fourier = fourier_pricer();
        let price = |trade: &SteppedCouponBondTrade<DualFwd>| -> Result<f64> {
            let result = if use_fourier {
                fourier.evaluate(trade, &[Request::Value], &provider)?
            } else {
                tree.evaluate(trade, &[Request::Value], &provider)?
            };
            result
                .price()
                .ok_or_else(|| QSError::ValueNotSetErr("Bond value".into()))
        };
        let long_value = price(&long)?;
        assert!((long_value + price(&converted)?).abs() < 1.0e-9);
        assert!(
            (price(
                &long
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::DirtyPrice(long_value))
            )? - long_value)
                .abs()
                < 1.0e-8
        );
    }
    Ok(())
}

#[test]
fn csa_policy_resolves_collateral_curve_for_foreign_bond() -> Result<()> {
    let date = Date::new(2025, 1, 1);
    let trade = fixed_bond(date, Date::new(2027, 1, 1), 0.0);
    let mut pricer = BondPricer::new();
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        MarketIndex::Other("EUR-COLLATERAL".into()),
        Currency::EUR,
    )));
    let request = pricer
        .market_data_request(&trade)
        .ok_or_else(|| QSError::NotFoundErr("Foreign CSA request".into()))?;
    assert!(matches!(
        request
            .constructed_elements_request()
            .and_then(|elements| elements.first()),
        Some(ConstructedElementRequest::DiscountCurve {
            market_index: MarketIndex::Collateral(Currency::USD, Currency::EUR)
        })
    ));
    Ok(())
}
