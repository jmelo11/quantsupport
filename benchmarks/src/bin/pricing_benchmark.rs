//! Backend-neutral benchmark of public pricing calls for cross-branch comparisons.
//!
//! Run: `cargo run --release -p benchmarks --bin pricing_benchmark -- [bond|equity|all] [samples] [repeats]`
//! Build each branch with the same pinned compiler and run binaries sequentially.

use quantsupport::{prelude::*, volatility::volatilityindexing::F64Key};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    env::args,
    error::Error,
    hint::black_box,
    rc::Rc,
    result::Result as StdResult,
    time::Instant,
};
type BenchmarkScalar = Recorded;

type BenchResult<T> = StdResult<T, Box<dyn Error>>;
struct Market {
    reference: Date,
    data: MarketData,
}
impl MarketDataProvider for Market {
    /// Returns the fixture market's evaluation date.
    fn evaluation_date(&self) -> Date {
        self.reference
    }
    /// Supplies the already constructed fixture market without changing pricing inputs.
    fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
        Ok(MarketData::new(
            self.data.fixings().clone(),
            self.data.constructed_elements().clone(),
        ))
    }
}

/// Returns the fixed evaluation date shared by both benchmark branches.
fn reference() -> Date {
    Date::new(2025, 1, 2)
}
/// Returns the shared continuous-compounding rate convention.
const fn definition() -> RateDefinition {
    RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    )
}
/// Builds a labelled flat discount curve for the benchmark market.
fn curve(index: MarketIndex, rate: f64, label: &str) -> DiscountCurveElement {
    DiscountCurveElement::new(
        index,
        Rc::new(RefCell::new(
            FlatForwardTermStructure::new(reference(), BenchmarkScalar::from(rate), definition())
                .with_pillar_label(label.to_owned()),
        )),
    )
}
/// Builds the shared fixed-rate bond trade for cross-branch pricing.
fn bond() -> Result<FixedRateBondTrade<BenchmarkScalar>> {
    let instrument = MakeFixedRateBond::<BenchmarkScalar>::default()
        .with_identifier("JIT-5Y-FIXED".into())
        .with_start_date(reference())
        .with_maturity_date(Date::new(2030, 1, 2))
        .with_rate(0.04)
        .with_notional(1_000_000.0)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Simple,
            Frequency::Semiannual,
        ))
        .with_discount_index(MarketIndex::SOFR)
        .with_currency(Currency::USD)
        .with_payment_frequency(Frequency::Semiannual)
        .with_payment_structure(PaymentStructure::Bullet)
        .build()?;
    Ok(FixedRateBondTrade::new(
        instrument,
        reference(),
        1_000_000.0,
        Side::LongReceive,
        BondPricingAnchor::ZSpread(0.0),
    ))
}
/// Builds a bond market at the requested continuous discount rate.
fn bond_market(rate: f64) -> Market {
    let mut elements = ConstructedElementStore::default();
    elements
        .discount_curves_mut()
        .insert(MarketIndex::SOFR, curve(MarketIndex::SOFR, rate, "rate"));
    Market {
        reference: reference(),
        data: MarketData::new(HashMap::new(), elements),
    }
}
/// Builds the shared European equity option trade.
fn option() -> EquityEuropeanOptionTrade {
    let instrument = EquityEuropeanOption::new(
        MarketIndex::Equity("BENCH".into()),
        Date::new(2026, 1, 2),
        Strike::Absolute(100.0),
        EuroOptionType::Call,
        "JIT-BLACK-CALL".into(),
    )
    .with_day_counter(DayCounter::Actual365);
    EquityEuropeanOptionTrade::new(instrument, 1_000.0, reference(), Side::LongReceive)
}
/// Binds the option market's spot, rate, volatility and dividend inputs.
fn option_market(inputs: &[f64]) -> Market {
    let [spot, rate, volatility, dividend] = [inputs[0], inputs[1], inputs[2], inputs[3]];
    let index = MarketIndex::Equity("BENCH".into());
    let mut elements = ConstructedElementStore::default();
    elements
        .discount_curves_mut()
        .insert(index.clone(), curve(index.clone(), rate, "rate"));
    elements.dividend_curves_mut().insert(
        index.clone(),
        DividendCurveElement::new(
            index.clone(),
            Rc::new(RefCell::new(
                FlatForwardTermStructure::new(
                    reference(),
                    BenchmarkScalar::from(dividend),
                    definition(),
                )
                .with_pillar_label("dividend".into()),
            )),
        ),
    );
    let points = BTreeMap::from([(
        Period::new(365, TimeUnit::Days),
        BTreeMap::from([(F64Key::new(100.0), BenchmarkScalar::from(volatility))]),
    )]);
    let surface = InterpolatedVolatilitySurface::new(
        reference(),
        index.clone(),
        points,
        VolatilityType::Black,
        SmileType::Strike,
    )
    .with_labels(&["volatility".into()]);
    elements.volatility_surfaces_mut().insert(
        index.clone(),
        VolatilitySurfaceElement::new(index.clone(), Rc::new(RefCell::new(surface))),
    );
    let fixings = HashMap::from([(index, BTreeMap::from([(reference(), spot)]))]);
    Market {
        reference: reference(),
        data: MarketData::new(fixings, elements),
    }
}
/// Extracts the price and first risks in a fixed published-label order.
fn outputs(result: &EvaluationResults, labels: &[&str]) -> BenchResult<(f64, Vec<f64>)> {
    let value = result.price().ok_or("missing engine price")?;
    let sensitivities = result
        .sensitivities()
        .ok_or("missing engine sensitivities")?;
    for label in labels {
        if !sensitivities
            .instrument_keys()
            .iter()
            .any(|key| key.as_str() == *label)
        {
            return Err(format!("missing engine risk label {label}").into());
        }
    }
    let gradients = labels
        .iter()
        .map(|label| {
            sensitivities
                .instrument_keys()
                .iter()
                .zip(sensitivities.exposure())
                .filter(|(key, _)| key.as_str() == *label)
                .map(|(_, value)| *value)
                .sum()
        })
        .collect();
    Ok((value, gradients))
}

/// Times the existing pricing method and captures the shared market scenarios.
fn measure(case: &str, samples: usize, repeats: u32) -> BenchResult<serde_json::Value> {
    let requests = [Request::Value, Request::Sensitivities];
    let cold;
    let scenarios;
    let mut times = Vec::new();
    match case {
        "bond" => {
            let trade = bond()?;
            let pricer = DiscountedCashflowPricer::<
                FixedRateBond<BenchmarkScalar>,
                FixedRateBondTrade<BenchmarkScalar>,
            >::new();
            let market = bond_market(0.03);
            let start = Instant::now();
            black_box(pricer.evaluate(&trade, &requests, &market)?);
            cold = start.elapsed().as_secs_f64() * 1e3;
            scenarios = [0.01, 0.03, 0.07]
                .into_iter()
                .map(|rate| {
                    outputs(
                        &pricer.evaluate(&trade, &requests, &bond_market(rate))?,
                        &["rate"],
                    )
                })
                .collect::<BenchResult<Vec<_>>>()?;
            for _ in 0..samples {
                let start = Instant::now();
                for _ in 0..repeats {
                    black_box(pricer.evaluate(black_box(&trade), &requests, &market)?);
                }
                times.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(repeats));
            }
        }
        "equity" => {
            let trade = option();
            let pricer = BlackEuropeanOptionPricer::new();
            let inputs = [100.0, 0.03, 0.20, 0.01];
            let market = option_market(&inputs);
            let start = Instant::now();
            black_box(pricer.evaluate(&trade, &requests, &market)?);
            cold = start.elapsed().as_secs_f64() * 1e3;
            scenarios = [80.0, 100.0, 120.0]
                .into_iter()
                .map(|spot| {
                    outputs(
                        &pricer.evaluate(
                            &trade,
                            &requests,
                            &option_market(&[spot, inputs[1], inputs[2], inputs[3]]),
                        )?,
                        &["EQ:BENCH", "rate", "volatility", "dividend"],
                    )
                })
                .collect::<BenchResult<Vec<_>>>()?;
            for _ in 0..samples {
                let start = Instant::now();
                for _ in 0..repeats {
                    black_box(pricer.evaluate(black_box(&trade), &requests, &market)?);
                }
                times.push(start.elapsed().as_secs_f64() * 1e6 / f64::from(repeats));
            }
        }
        _ => return Err("unknown pricing case".into()),
    }
    times.sort_by(f64::total_cmp);
    Ok(
        serde_json::json!({"case":case, "first_call_ms":cold, "median_us":times[times.len()/2], "scenarios":scenarios}),
    )
}

/// Runs the standalone comparison driver and emits its timing and numerical results.
fn main() -> BenchResult<()> {
    let mut args = args().skip(1);
    let case = args.next().unwrap_or_else(|| "all".to_owned());
    let samples: usize = args.next().map_or(Ok(9), |s| s.parse())?;
    let repeats: u32 = args.next().map_or(Ok(20_000), |s| s.parse())?;
    if samples == 0 || repeats == 0 || args.next().is_some() {
        return Err("usage: [bond|equity|all] [positive samples] [positive repeats]".into());
    }
    for name in if case == "all" {
        vec!["bond", "equity"]
    } else {
        vec![case.as_str()]
    } {
        println!("{}", measure(name, samples, repeats)?);
    }
    Ok(())
}
