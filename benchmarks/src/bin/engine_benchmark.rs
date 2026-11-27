//! Cross-branch benchmarks of the existing non-Monte-Carlo pricing methods.
//!
//! Run: `engine_benchmark case samples repeats result.json [value|risk] [trades]`.

use quantsupport::{
    prelude::*,
    pricers::rates::{
        closedformhullwhitecapletpricer::ClosedFormHullWhiteCapletPricer,
        closedformhullwhitecappricer::ClosedFormHullWhiteCapPricer,
    },
    volatility::volatilityindexing::F64Key,
};
use serde_json::{json, Value};
use std::{
    cell::RefCell, collections::BTreeMap, env::args, error::Error, fs, hint::black_box, rc::Rc,
    result::Result as StdResult, time::Instant,
};

type BenchmarkScalar = Recorded;
type BenchResult<T> = StdResult<T, Box<dyn Error>>;
type Valuation = Box<dyn FnMut() -> Result<EvaluationResults>>;

/// Returns the fixed evaluation date shared by both benchmark branches.
fn reference() -> Date {
    Date::new(2025, 1, 2)
}
/// Returns a contract date a whole number of years after the fixture reference.
fn date(years: i32) -> Date {
    reference() + Period::new(years, TimeUnit::Years)
}
/// Returns the shared continuous-compounding rate convention.
const fn rate_definition() -> RateDefinition {
    RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    )
}
struct BenchmarkDiscountPolicy;
impl DiscountPolicy for BenchmarkDiscountPolicy {
    /// Selects the benchmark discount curve for the instrument currency.
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex> {
        match target.currency() {
            Currency::USD => Ok(MarketIndex::SOFR),
            Currency::EUR => Ok(MarketIndex::ESTR),
            _ => Err(QSError::InvalidValueErr(
                "unsupported benchmark currency".into(),
            )),
        }
    }
    /// Lists the discount indices required by the benchmark policy.
    fn discount_indices(&self) -> Vec<MarketIndex> {
        vec![MarketIndex::SOFR, MarketIndex::ESTR]
    }
}
/// Creates the common currency-based discount policy for the pricing fixtures.
fn policy() -> Box<dyn DiscountPolicy> {
    Box::new(BenchmarkDiscountPolicy)
}

/// Builds the shared curve, FX and volatility market for a pricing family.
fn context(implementation: Option<PricerImplementation>) -> Result<PricingContext> {
    let mut elements = ConstructedElementStore::default();
    let equity = MarketIndex::Equity("BENCH".into());
    for (index, rate, label) in [
        (MarketIndex::SOFR, 0.03, "USD.rate"),
        (MarketIndex::ESTR, 0.02, "EUR.rate"),
        (equity.clone(), 0.03, "equity.rate"),
    ] {
        let curve = FlatForwardTermStructure::new(
            reference(),
            BenchmarkScalar::from(rate),
            rate_definition(),
        )
        .with_pillar_label(label.into());
        elements.discount_curves_mut().insert(
            index.clone(),
            DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
        );
    }
    let dividend =
        FlatForwardTermStructure::new(reference(), BenchmarkScalar::from(0.01), rate_definition())
            .with_pillar_label("dividend".into());
    elements.dividend_curves_mut().insert(
        equity.clone(),
        DividendCurveElement::new(equity.clone(), Rc::new(RefCell::new(dividend))),
    );
    let credit = MarketIndex::Credit("BENCH".into());
    let survival =
        FlatForwardTermStructure::new(reference(), BenchmarkScalar::from(0.02), rate_definition())
            .with_pillar_label("hazard".into());
    elements.credit_curves_mut().insert(
        credit.clone(),
        CreditCurveElement::new(credit, 0.4, Rc::new(RefCell::new(survival))),
    );
    for (index, strike, volatility, label) in [
        (equity.clone(), 100.0, 0.2, "equity.vol"),
        (MarketIndex::SOFR, 0.04, 0.2, "rate.vol"),
        (
            MarketIndex::FxPair(FxPair::new(Currency::EUR, Currency::USD)?),
            1.1,
            0.15,
            "fx.vol",
        ),
    ] {
        let points = [1, 4]
            .into_iter()
            .map(|years| {
                (
                    Period::new(years, TimeUnit::Years),
                    [strike * 0.8, strike * 1.2]
                        .into_iter()
                        .map(|key| (F64Key::new(key), BenchmarkScalar::from(volatility)))
                        .collect(),
                )
            })
            .collect();
        let labels: Vec<_> = (0..4).map(|point| format!("{label}.{point}")).collect();
        let surface = InterpolatedVolatilitySurface::new(
            reference(),
            index.clone(),
            points,
            VolatilityType::Black,
            SmileType::Strike,
        )
        .with_labels(&labels);
        elements.volatility_surfaces_mut().insert(
            index.clone(),
            VolatilitySurfaceElement::new(index, Rc::new(RefCell::new(surface))),
        );
    }
    let mut fx = FxStore::new();
    fx.add_fx_rate(Currency::EUR, Currency::USD, BenchmarkScalar::from(1.1));
    let mut fixings = FixingStore::default();
    fixings.add_fixing(&equity, reference(), 100.0);
    let mut context = PricingContext::new()
        .with_quote_store(QuoteStore::new(reference()))
        .with_constructed_elements(elements)
        .with_fx_store(fx)
        .with_fixing_store(fixings)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR);
    if let Some(implementation) = implementation {
        context = context.with_pricer_configurations(vec![PricerConfiguration::new(
            InstrumentType::SteppedCouponBond,
            implementation,
            vec![PricingModelConfiguration::new(
                MarketIndex::SOFR,
                ModelConfiguration::HullWhite {
                    alpha: 0.03,
                    parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(
                        0.01,
                    )),
                },
            )],
        )]);
    }
    Ok(context)
}

/// Builds a fixed-for-floating swap with shared schedules and trade conventions.
fn swap(start: Date, end: Date) -> Result<Swap<BenchmarkScalar>> {
    MakeSwap::<BenchmarkScalar>::default()
        .with_identifier("SWAP".into())
        .with_start_date(start)
        .with_maturity_date(end)
        .with_notional(1e6)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_fixed_rate(0.04)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Quarterly,
        ))
        .build()
}

/// Constructs an existing pricer and its fixed contract/market valuation closure.
#[allow(clippy::too_many_lines)] // One common fixture registry for the two branches.
fn fixture(kind: &str, with_risk: bool, portfolio_trades: usize) -> Result<Valuation> {
    let requests = if with_risk {
        vec![Request::Value, Request::Sensitivities]
    } else {
        vec![Request::Value]
    };
    let context = context(match kind {
        "tree" => Some(PricerImplementation::TreeBased {
            time_grid: TimeGridConfiguration::new(12, 256, DayCounter::Actual365),
        }),
        "fourier" => Some(PricerImplementation::BackwardFourierEvolution {
            time_grid: TimeGridConfiguration::new(12, 256, DayCounter::Actual365),
            fourier_grid: FourierGridConfiguration::new(64, 10.0),
        }),
        _ => None,
    })?;
    match kind {
        "black-equity" => {
            let option = EquityEuropeanOption::new(
                MarketIndex::Equity("BENCH".into()),
                date(1),
                Strike::Absolute(100.0),
                EuroOptionType::Call,
                "EQ".into(),
            );
            let trade =
                EquityEuropeanOptionTrade::new(option, 1000.0, reference(), Side::LongReceive);
            let pricer = BlackEuropeanOptionPricer::new();
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "equity-forward" => {
            let instrument = EquityForward::new(
                "EQF".into(),
                MarketIndex::Equity("BENCH".into()),
                date(1),
                100.0,
                Currency::USD,
                DayCounter::Actual365,
            );
            let trade = EquityForwardTrade::new(instrument, reference(), 1000.0, Side::LongReceive);
            let pricer = EquityForwardPricer::new();
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "fx-option" => {
            let instrument = FxEuropeanOption::new(
                "FXO".into(),
                MarketIndex::FxPair(FxPair::new(Currency::EUR, Currency::USD)?),
                date(1),
                Strike::Absolute(1.1),
                EuroOptionType::Call,
                Currency::EUR,
                Currency::USD,
                DayCounter::Actual365,
            );
            let trade = FxEuropeanOptionTrade::new(instrument, reference(), 1e6, Side::LongReceive);
            let mut pricer = FxEuropeanOptionPricer::new();
            pricer.set_discount_policy(policy());
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "fx-forward" => {
            let instrument = FxForward::new(
                "FXF".into(),
                date(1),
                Some(1.1),
                None,
                Currency::EUR,
                Currency::USD,
                DayCounter::Actual365,
                FxForwardSettlement::Deliverable,
            );
            let trade = FxForwardTrade::new(instrument, reference(), 1e6, Side::LongReceive);
            let mut pricer = FxForwardPricer::new();
            pricer.set_discount_policy(policy());
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "dcf" | "portfolio" => {
            let underlying = swap(date(1), date(4))?;
            let trade = SwapTrade::new(underlying, reference(), 1e6, Side::LongReceive);
            let mut pricer = DiscountedCashflowPricer::new();
            pricer.set_discount_policy(policy());
            if kind == "portfolio" {
                let mut trades = Vec::with_capacity(portfolio_trades);
                for _ in 0..portfolio_trades {
                    trades.push(SwapTrade::new(
                        swap(date(1), date(4))?,
                        reference(),
                        1e6,
                        Side::LongReceive,
                    ));
                }
                let portfolio = Portfolio::new("PORTFOLIO".into(), trades)?;
                let pricer = PortfolioPricer::new(pricer);
                Ok(Box::new(move || {
                    pricer.evaluate(&portfolio, &requests, &context)
                }))
            } else {
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            }
        }
        "loan" => {
            let instrument = MakeLoan::<BenchmarkScalar>::default()
                .with_identifier("LOAN".into())
                .with_start_date(reference())
                .with_maturity_date(date(3))
                .with_notional(1e6)
                .with_currency(Currency::USD)
                .with_payment_frequency(Frequency::Quarterly)
                .with_coupons(vec![CashflowType::fixed(0.05)])
                .with_repayment_structure(LoanRepaymentStructure::French)
                .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.06))
                .build()?;
            let trade = LoanTrade::new(instrument, reference(), Side::LongReceive);
            let mut pricer = LoanPricer::new();
            pricer.set_discount_policy(policy());
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "bond" => {
            let instrument = MakeFixedRateBond::<BenchmarkScalar>::default()
                .with_identifier("BOND".into())
                .with_start_date(reference())
                .with_maturity_date(date(3))
                .with_notional(1e6)
                .with_currency(Currency::USD)
                .with_rate(0.05)
                .with_rate_definition(rate_definition())
                .with_discount_index(MarketIndex::SOFR)
                .build()?;
            let trade = FixedRateBondTrade::new(
                instrument,
                reference(),
                1e6,
                Side::LongReceive,
                BondPricingAnchor::CleanPrice(100.0),
            );
            let mut pricer = BondPricer::new();
            pricer.set_discount_policy(policy());
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "tree" | "fourier" => {
            let instrument = MakeSteppedCouponBond::<BenchmarkScalar>::default()
                .with_identifier("CALLABLE".into())
                .with_start_date(reference())
                .with_maturity_date(date(3))
                .with_notional(1e6)
                .with_units(100.0)
                .with_notional_percentages(vec![1.0])
                .with_currency(Currency::USD)
                .with_payment_frequency(Frequency::Semiannual)
                .with_coupons(vec![CashflowType::fixed(0.05)])
                .with_call_schedule(vec![CallScheduleEntry::window(date(1), date(2), 100.0)])
                .build()?;
            let trade = SteppedCouponBondTrade::new(
                instrument,
                reference(),
                1e6,
                Side::LongReceive,
                BondPricingAnchor::OAS(0.005),
            );
            if kind == "tree" {
                let mut pricer = TreeBasedBondPricer::new();
                pricer.set_discount_policy(policy());
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            } else {
                let mut pricer = BackwardFourierEvolutionBondPricer::new();
                pricer.set_discount_policy(policy());
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            }
        }
        "cds" => {
            let instrument = CreditDefaultSwap::new(
                "CDS".into(),
                MarketIndex::Credit("BENCH".into()),
                MarketIndex::SOFR,
                Currency::USD,
                reference(),
                date(3),
                0.012,
                0.4,
                Frequency::Quarterly,
                DayCounter::Actual360,
            )?;
            let trade = CdsTrade::new(instrument, reference(), 1e6, Side::LongReceive);
            let pricer = CdsPricer::new();
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "futures" => {
            let instrument = MakeRateFutures::default()
                .with_identifier("FUT".into())
                .with_market_index(MarketIndex::SOFR)
                .with_start_date(date(1))
                .with_end_date(date(1) + Period::new(3, TimeUnit::Months))
                .with_futures_price(96.0)
                .build()?;
            let trade = RateFuturesTrade::new(instrument, reference(), 1.0, Side::LongReceive);
            let pricer = RateFuturesPricer::new();
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        "black-caplet" | "hw-caplet" => {
            let instrument = CapletFloorlet::new(
                "CAPLET".into(),
                MarketIndex::SOFR,
                Currency::USD,
                date(1),
                date(1),
                date(1) + Period::new(3, TimeUnit::Months),
                date(1) + Period::new(3, TimeUnit::Months),
                CapletFloorletType::Caplet,
                Strike::Absolute(0.04),
            );
            let trade = CapletFloorletTrade::new(instrument, reference(), 1e6, Side::LongReceive);
            if kind == "black-caplet" {
                let mut pricer = ClosedFormBlackCapletPricer::new();
                pricer.set_discount_policy(policy());
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            } else {
                let mut pricer = ClosedFormHullWhiteCapletPricer::new(0.03, 0.01);
                pricer.set_discount_policy(policy());
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            }
        }
        "black-cap" | "hw-cap" => {
            let instrument = MakeCapFloor::default()
                .with_identifier("CAP".into())
                .with_start_date(date(1))
                .with_maturity_date(date(3))
                .with_strike(0.04)
                .with_notional(1e6)
                .with_currency(Currency::USD)
                .with_market_index(MarketIndex::SOFR)
                .with_cap_floor_type(CapFloorType::Cap)
                .with_frequency(Frequency::Quarterly)
                .build()?;
            let trade = CapFloorTrade::new(instrument, reference(), 1e6, Side::LongReceive);
            if kind == "black-cap" {
                let mut pricer = ClosedFormBlackCapPricer::new();
                pricer.set_discount_policy(policy());
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            } else {
                let mut pricer = ClosedFormHullWhiteCapPricer::new(0.03, 0.01);
                pricer.set_discount_policy(policy());
                Ok(Box::new(move || {
                    pricer.evaluate(&trade, &requests, &context)
                }))
            }
        }
        "hw-swaption" => {
            let instrument = EuropeanSwaption::new(
                "SWAPTION".into(),
                swap(date(1), date(4))?,
                date(1),
                SwaptionType::Payer,
                0.04,
                MarketIndex::SOFR,
                Currency::USD,
            );
            let trade = EuropeanSwaptionTrade::new(instrument, reference(), 1e6, Side::LongReceive);
            let mut pricer = ClosedFormHullWhiteSwaptionPricer::new(0.03, 0.01);
            pricer.set_discount_policy(policy());
            Ok(Box::new(move || {
                pricer.evaluate(&trade, &requests, &context)
            }))
        }
        _ => Err(QSError::InvalidValueErr("unknown engine benchmark".into())),
    }
}

/// Captures the finite price and every labelled first sensitivity for comparison.
fn snapshot(result: &EvaluationResults, with_risk: bool) -> BenchResult<Value> {
    let value = result.price().ok_or("missing engine price")?;
    if !value.is_finite() {
        return Err("nonfinite price".into());
    }
    if !with_risk {
        if result.sensitivities().is_some() {
            return Err("value-only request unexpectedly returned sensitivities".into());
        }
        return Ok(json!({"price": value, "sensitivities": {}}));
    }
    let risk = result
        .sensitivities()
        .ok_or("missing engine sensitivities")?;
    let mut sensitivities = BTreeMap::new();
    for (label, &value) in risk.instrument_keys().iter().zip(risk.exposure()) {
        if !value.is_finite() {
            return Err("nonfinite risk".into());
        }
        *sensitivities.entry(label.clone()).or_insert(0.0) += value;
    }
    if sensitivities.is_empty() {
        return Err("empty risk".into());
    }
    Ok(json!({"price":value,"sensitivities":sensitivities}))
}

/// Runs the standalone comparison driver and emits its timing and numerical results.
fn main() -> BenchResult<()> {
    let arguments = args().skip(1).collect::<Vec<_>>();
    if !(4..=6).contains(&arguments.len()) {
        return Err(
            "usage: case samples repeats result.json [value|risk] [positive trades]".into(),
        );
    }
    let samples: usize = arguments[1].parse()?;
    let repeats: u32 = arguments[2].parse()?;
    let mode = arguments.get(4).map_or("risk", String::as_str);
    let with_risk = match mode {
        "value" => false,
        "risk" => true,
        _ => return Err("request mode must be value or risk".into()),
    };
    let portfolio_trades: usize = arguments.get(5).map_or(Ok(2), |value| value.parse())?;
    if samples == 0 || repeats == 0 || portfolio_trades == 0 {
        return Err("counts must be positive".into());
    }
    if arguments[0] != "portfolio" && portfolio_trades != 2 {
        return Err("trade count applies only to the portfolio case".into());
    }
    let start = Instant::now();
    let mut evaluate = fixture(&arguments[0], with_risk, portfolio_trades)?;
    let setup = start.elapsed().as_secs_f64() * 1e3;
    let start = Instant::now();
    let result = evaluate()?;
    let first = start.elapsed().as_secs_f64() * 1e3;
    let mut times = Vec::new();
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..repeats {
            black_box(evaluate()?);
        }
        times.push(start.elapsed().as_secs_f64() * 1e3 / f64::from(repeats));
    }
    times.sort_by(f64::total_cmp);
    let report = json!({"case":arguments[0],"request_mode":mode,"portfolio_trades":portfolio_trades,"samples":samples,"repeats":repeats,"sample_ms":times,"setup_ms":setup,"first_call_ms":first,"median_call_ms":times[times.len()/2],"snapshot":snapshot(&result, with_risk)?});
    fs::write(&arguments[3], serde_json::to_vec(&report)?)?;
    println!(
        "{}",
        json!({"case":arguments[0],"setup_ms":setup,"first_call_ms":first,"median_call_ms":times[times.len()/2]})
    );
    Ok(())
}
