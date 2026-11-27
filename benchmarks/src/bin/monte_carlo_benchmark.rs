//! Cross-branch benchmarks of existing standalone Monte Carlo APIs.
//!
//! Run with `kind paths samples repeats output.json`. Kinds: equity-call,
//! equity-put, equity-call-value, script-equity, script-rates, build-brownian, build-hullwhite,
//! build-lgm. The same file is used on the old and migration branches.

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    env::args,
    error::Error,
    fs,
    hint::black_box,
    rc::Rc,
    result::Result as StdResult,
    time::Instant,
};

use quantsupport::prelude::*;
use serde_json::{json, Value};

type BenchmarkScalar = Recorded;

type BenchResult<T> = StdResult<T, Box<dyn Error>>;

/// Returns the fixed evaluation date shared by both benchmark branches.
fn reference() -> Date {
    Date::new(2025, 1, 2)
}
/// Returns the fixed one-year expiry used by the Monte Carlo fixtures.
fn expiry() -> Date {
    Date::new(2026, 1, 2)
}
/// Returns the shared continuous-compounding rate convention.
const fn definition() -> RateDefinition {
    RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    )
}

struct Market {
    data: MarketData,
}
impl MarketDataProvider for Market {
    /// Returns the fixture market's evaluation date.
    fn evaluation_date(&self) -> Date {
        reference()
    }
    /// Supplies the already constructed fixture market without changing pricing inputs.
    fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
        Ok(MarketData::new(
            self.data.fixings().clone(),
            self.data.constructed_elements().clone(),
        ))
    }
}

/// Constructs the labelled market curves used by the Monte Carlo fixtures.
fn store(index: &MarketIndex) -> Result<ConstructedElementStore> {
    let dates = vec![
        reference(),
        expiry(),
        reference() + Period::new(5, TimeUnit::Years),
    ];
    let discounts = dates
        .iter()
        .map(|date| {
            BenchmarkScalar::from(
                (-0.03 * DayCounter::Actual365.year_fraction(reference(), *date)).exp(),
            )
        })
        .collect();
    let curve = DiscountTermStructure::<BenchmarkScalar>::new(
        dates,
        discounts,
        DayCounter::Actual365,
        Interpolator::LogLinear,
        true,
    )?
    .with_pillar_labels(vec!["df_0d".into(), "df_1y".into(), "df_5y".into()])?;
    let mut result = ConstructedElementStore::default();
    result.discount_curves_mut().insert(
        index.clone(),
        DiscountCurveElement::new(index.clone(), Rc::new(RefCell::new(curve))),
    );
    Ok(result)
}

/// Builds a seeded simulation configuration with the requested model and path count.
fn configuration(kind: &str, paths: usize) -> SimulationConfiguration {
    let model = match kind {
        "build-hullwhite" => ModelConfiguration::HullWhite {
            alpha: 0.03,
            parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.01)),
        },
        "build-lgm" => ModelConfiguration::Lgm {
            lambda: 0.03,
            parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.01)),
        },
        _ => ModelConfiguration::BrownianMotion {
            parameter_source: ParameterSource::Fixed(LognormalModelParameters::new(0.20)),
            dividend_rate: Some(0.01),
        },
    };
    SimulationConfiguration::new(
        MarketIndex::Equity("BENCH".into()),
        model,
        paths,
        42,
        Period::new(1, TimeUnit::Years),
        Frequency::Monthly,
    )
}

/// Provides the fixture's reference-date spot observation.
fn fixing_store(index: &MarketIndex) -> FixingStore {
    let mut fixings = FixingStore::default();
    fixings.add_fixing(index, reference(), 100.0);
    fixings
}

/// Materializes the common seeded equity paths and their discount market.
fn equity_market(paths: usize) -> Result<Market> {
    let index = MarketIndex::Equity("BENCH".into());
    let mut elements = store(&index)?;
    let builder = SimulationBuilder::new(vec![configuration("build-brownian", paths)]);
    let simulations = builder.build(
        &elements,
        &QuoteStore::new(reference()),
        &fixing_store(&index),
        Level::Mid,
    )?;
    for (index, element) in simulations {
        elements.simulations_mut().insert(index, element);
    }
    Ok(Market {
        data: MarketData::new(
            HashMap::from([(index, BTreeMap::from([(reference(), 100.0)]))]),
            elements,
        ),
    })
}

/// Captures the existing pricer's finite value and requested first risks.
fn result_snapshot(result: &EvaluationResults, with_risk: bool) -> Result<Value> {
    let price = result
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("MC price".into()))?;
    if !price.is_finite() {
        return Err(QSError::InvalidValueErr("nonfinite MC price".into()));
    }
    if !with_risk {
        return Ok(json!({"price":price,"sensitivities":{}}));
    }
    let sensitivities = result
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("MC sensitivities".into()))?;
    let mut risks = BTreeMap::new();
    for (label, value) in sensitivities
        .instrument_keys()
        .iter()
        .zip(sensitivities.exposure())
    {
        if !value.is_finite() {
            return Err(QSError::InvalidValueErr("nonfinite MC risk".into()));
        }
        *risks.entry(label.clone()).or_insert(0.0) += value;
    }
    if !risks.contains_key("df_1y") {
        return Err(QSError::ValueNotSetErr("rate risk".into()));
    }
    Ok(json!({"price": price, "sensitivities":risks}))
}

struct ScriptSetup {
    paths: usize,
    equity: bool,
}
impl ScriptModelSetup for ScriptSetup {
    /// Returns the configured number of independently indexed simulation paths.
    fn n_paths(&self) -> usize {
        self.paths
    }
    /// Binds the shared LGM model and its labelled market inputs for valuation.
    fn with_model<R>(&self, callback: &mut ScriptModelCallback<'_, R>) -> Result<R> {
        let curve =
            FlatForwardTermStructure::new(reference(), BenchmarkScalar::new(0.03), definition())
                .with_pillar_label("rate".into());
        let mut leaves = curve
            .pillars()
            .unwrap_or_default()
            .into_iter()
            .map(|(label, value)| (label, *value))
            .collect::<Vec<_>>();
        let rate = LgmRateModel::new(
            BenchmarkScalar::scalar(0.03),
            BenchmarkScalar::scalar(0.01),
            &curve,
        );
        let equity_rate = LgmRateModel::new(
            BenchmarkScalar::scalar(0.03),
            BenchmarkScalar::scalar(0.01),
            &curve,
        );
        let mut model = LgmMarketModel::new(
            Currency::USD,
            MarketIndex::SOFR,
            reference(),
            DayCounter::Actual365,
        )
        .with_n_paths(self.paths)
        .with_seed(42);
        model.add_curve_model(MarketIndex::SOFR, rate);
        if self.equity {
            let spot = BenchmarkScalar::new(100.0);
            let volatility = BenchmarkScalar::new(0.20);
            leaves.extend([("spot".into(), spot), ("volatility".into(), volatility)]);
            model.add_equity_model(
                "BENCH".into(),
                LgmEquityModel::new(
                    &equity_rate,
                    volatility,
                    spot,
                    BenchmarkScalar::scalar(0.01),
                    BenchmarkScalar::zero(),
                ),
            );
        }
        callback(&mut model, &leaves)
    }
}

/// Builds a dated equity, rate or conditional digital payoff fixture.
fn script(equity: bool, digital: bool) -> Result<ScriptEngine> {
    let events = if equity {
        vec![CodedEvent::new(
            expiry(),
            if digital {
                "price = 0; if Spot(\"BENCH\") > 100 { price pays 1000; }".into()
            } else {
                "price = 0; price pays 1000 * max(Spot(\"BENCH\") - 100, 0);".into()
            },
        )]
    } else {
        (0..4).map(|period| {
            let start = reference() + Period::new(period * 3, TimeUnit::Months);
            let end = reference() + Period::new((period + 1) * 3, TimeUnit::Months);
            let init = if period == 0 { "price = 0;" } else { "" };
            CodedEvent::new(start, format!(
                "{init} fixing = RateIndex(\"SOFR\", \"{start}\", \"{end}\"); price pays 1000000 * (0.04 - fixing) * cvg(\"{start}\", \"{end}\", \"Actual365\") on \"{end}\";"))
        }).collect()
    };
    ScriptEngine::new(
        EventStream::try_from(events)?,
        reference(),
        Currency::USD,
        MarketIndex::SOFR,
    )
}

/// Captures numeric variables, payments and first risks from script evaluation.
fn script_snapshot(result: &ParallelScriptEvaluation) -> Value {
    let values = result.values.iter().collect::<BTreeMap<_, _>>();
    let mut risks = BTreeMap::new();
    for (label, value) in &result.sensitivities {
        *risks.entry(label).or_insert(0.0) += value;
    }
    json!({"values":values, "sensitivities":risks,
        "cashflows":result.cashflows.iter().map(|cf|
            (cf.date.to_string(), cf.amount, cf.present_value)).collect::<Vec<_>>()})
}

/// Measures the first call and repeated public calls without including JSON serialization.
fn time_calls<T>(
    samples: usize,
    repeats: u32,
    mut call: impl FnMut() -> Result<T>,
) -> Result<(T, f64, f64)> {
    let start = Instant::now();
    let first = call()?;
    let first_ms = start.elapsed().as_secs_f64() * 1e3;
    let mut times = Vec::new();
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..repeats {
            black_box(call()?);
        }
        times.push(start.elapsed().as_secs_f64() * 1e3 / f64::from(repeats));
    }
    times.sort_by(f64::total_cmp);
    Ok((first, first_ms, times[times.len() / 2]))
}

/// Runs the standalone comparison driver and emits its timing and numerical results.
fn main() -> BenchResult<()> {
    let args = args().skip(1).collect::<Vec<_>>();
    if args.len() != 5 {
        return Err("usage: kind paths samples repeats output.json".into());
    }
    let kind = &args[0];
    let paths: usize = args[1].parse()?;
    let samples: usize = args[2].parse()?;
    let repeats: u32 = args[3].parse()?;
    if paths == 0 || samples == 0 || repeats == 0 {
        return Err("counts must be positive".into());
    }
    let start = Instant::now();
    let (snapshot, first_ms, median_ms, setup_ms) = match kind.as_str() {
        "equity-call" | "equity-put" | "equity-call-value" => {
            let market = equity_market(paths)?;
            let option = EquityEuropeanOption::new(
                MarketIndex::Equity("BENCH".into()),
                expiry(),
                Strike::Absolute(100.0),
                if kind == "equity-put" {
                    EuroOptionType::Put
                } else {
                    EuroOptionType::Call
                },
                "MC-BENCH".into(),
            );
            let trade =
                EquityEuropeanOptionTrade::new(option, 1000.0, reference(), Side::LongReceive);
            let pricer = BlackMCEuropeanOptionPricer::new();
            let setup_ms = start.elapsed().as_secs_f64() * 1e3;
            let with_risk = kind != "equity-call-value";
            let requests = if with_risk {
                vec![Request::Value, Request::Sensitivities]
            } else {
                vec![Request::Value]
            };
            let (result, first_ms, median_ms) = time_calls(samples, repeats, || {
                pricer.evaluate(&trade, &requests, &market)
            })?;
            (
                result_snapshot(&result, with_risk)?,
                first_ms,
                median_ms,
                setup_ms,
            )
        }
        "script-equity" | "script-rates" | "script-digital" => {
            let equity = kind != "script-rates";
            let engine = script(equity, kind == "script-digital")?;
            let setup = ScriptSetup { paths, equity };
            let setup_ms = start.elapsed().as_secs_f64() * 1e3;
            let (result, first_ms, median_ms) = time_calls(samples, repeats, || {
                engine.evaluate_parallel(&setup, Some("price"))
            })?;
            (script_snapshot(&result), first_ms, median_ms, setup_ms)
        }
        "build-brownian" | "build-hullwhite" | "build-lgm" => {
            let index = MarketIndex::Equity("BENCH".into());
            let elements = store(&index)?;
            let builder = SimulationBuilder::new(vec![configuration(kind, paths)]);
            let quotes = QuoteStore::new(reference());
            let fixings = fixing_store(&index);
            let setup_ms = start.elapsed().as_secs_f64() * 1e3;
            let (result, first_ms, median_ms) = time_calls(samples, repeats, || {
                builder.build(&elements, &quotes, &fixings, Level::Mid)
            })?;
            let simulation = result[&index].simulation().borrow();
            let numerical = simulation
                .path()
                .iter()
                .map(|path| path.iter().map(BenchmarkScalar::value).collect::<Vec<_>>())
                .collect::<Vec<_>>();
            (
                json!({"generated_paths":numerical,"dates":simulation.dates().iter().map(ToString::to_string).collect::<Vec<_>>()}),
                first_ms,
                median_ms,
                setup_ms,
            )
        }
        _ => return Err("unknown Monte Carlo case".into()),
    };
    let report = json!({"case":kind,"path_count":paths,"samples":samples,"repeats":repeats,
        "setup_ms":setup_ms,"first_call_ms":first_ms,"median_call_ms":median_ms,"snapshot":snapshot});
    fs::write(&args[4], serde_json::to_vec(&report)?)?;
    println!("{kind}: {paths} paths, setup={setup_ms:.3} ms, first={first_ms:.3} ms, warm={median_ms:.3} ms");
    Ok(())
}
