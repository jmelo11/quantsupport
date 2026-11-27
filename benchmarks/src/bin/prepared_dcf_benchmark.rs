//! Explicit prepared DCF snapshots versus ordinary valuation of the same swap book.
//!
//! Usage: `prepared_dcf_benchmark trades samples result.json [value|risk]`.
//! A single trade uses the single-contract API; larger books use portfolio aggregation.
use quantsupport::prelude::*;
use serde_json::{json, Value};
use std::{
    cell::RefCell, collections::BTreeMap, env::args, error::Error, fs, hint::black_box, rc::Rc,
    time::Instant,
};

type PricerType = DiscountedCashflowPricer<Swap<Recorded>, SwapTrade<Recorded>>;
type BenchResult<T> = std::result::Result<T, Box<dyn Error>>;
fn reference() -> Date {
    Date::new(2025, 1, 2)
}
fn date(years: i32) -> Date {
    reference() + Period::new(years, TimeUnit::Years)
}
fn market(rate: f64) -> PricingContext {
    let curve = FlatForwardTermStructure::new(
        reference(),
        Recorded::from(rate),
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        ),
    )
    .with_pillar_label("USD.rate".into());
    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
    );
    PricingContext::new()
        .with_quote_store(QuoteStore::new(reference()))
        .with_constructed_elements(elements)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
}
fn swap() -> Result<SwapTrade<Recorded>> {
    let instrument = MakeSwap::<Recorded>::default()
        .with_identifier("SWAP".into())
        .with_start_date(date(1))
        .with_maturity_date(date(4))
        .with_notional(1e6)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_fixed_rate(0.04)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Quarterly,
        ))
        .build()?;
    Ok(SwapTrade::new(
        instrument,
        reference(),
        1e6,
        Side::LongReceive,
    ))
}
enum Binding {
    Trade(PreparedDcfValuation),
    Book(PreparedDcfPortfolio),
}
impl Binding {
    fn evaluate(&mut self, requests: &[Request]) -> Result<EvaluationResults> {
        match self {
            Self::Trade(p) => p.evaluate(requests),
            Self::Book(p) => p.evaluate(requests),
        }
    }
}
fn snapshot(result: &EvaluationResults) -> BenchResult<Value> {
    let price = result.price().ok_or("missing price")?;
    let risk = result.sensitivities().map_or_else(BTreeMap::new, |s| {
        s.instrument_keys()
            .iter()
            .cloned()
            .zip(s.exposure().iter().copied())
            .collect::<BTreeMap<_, _>>()
    });
    if !price.is_finite() || risk.values().any(|v| !v.is_finite()) {
        return Err("nonfinite valuation".into());
    }
    Ok(json!({"price":price,"sensitivities":risk}))
}
fn compare(left: &EvaluationResults, right: &EvaluationResults) -> BenchResult<()> {
    let l = snapshot(left)?;
    let r = snapshot(right)?;
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-8 + 1e-11 * a.abs().max(b.abs());
    if !near(
        l["price"].as_f64().ok_or("missing PV")?,
        r["price"].as_f64().ok_or("missing PV")?,
    ) {
        return Err("PV mismatch".into());
    }
    let l = l["sensitivities"].as_object().ok_or("missing risks")?;
    let r = r["sensitivities"].as_object().ok_or("missing risks")?;
    if l.keys().ne(r.keys())
        || l.iter().any(|(key, v)| {
            !near(
                v.as_f64().unwrap_or(f64::NAN),
                r[key].as_f64().unwrap_or(f64::NAN),
            )
        })
    {
        return Err("risk mismatch".into());
    }
    Ok(())
}
fn timed<T>(function: impl FnOnce() -> Result<T>) -> Result<(T, f64)> {
    let start = Instant::now();
    let result = function()?;
    Ok((result, start.elapsed().as_secs_f64() * 1e3))
}
fn samples<T>(
    count: usize,
    repeats: usize,
    mut function: impl FnMut() -> Result<T>,
) -> Result<Value> {
    let mut times = Vec::new();
    for _ in 0..count {
        let start = Instant::now();
        for _ in 0..repeats {
            black_box(function()?);
        }
        times.push(start.elapsed().as_secs_f64() * 1e3 / repeats as f64);
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    Ok(json!({"sample_ms":times,"median_ms":sorted[sorted.len()/2],"repeats":repeats}))
}
fn main() -> BenchResult<()> {
    let arguments = args().skip(1).collect::<Vec<_>>();
    if !(3..=4).contains(&arguments.len()) {
        return Err("usage: trades samples result.json [value|risk]".into());
    }
    let trades: usize = arguments[0].parse()?;
    let count: usize = arguments[1].parse()?;
    if trades == 0 || count == 0 {
        return Err("counts must be positive".into());
    }
    let mode = arguments.get(3).map_or("risk", String::as_str);
    let requests = match mode {
        "value" => vec![Request::Value],
        "risk" => vec![Request::Value, Request::Sensitivities],
        _ => return Err("invalid mode".into()),
    };
    let portfolio = Portfolio::new(
        "PORTFOLIO".into(),
        (0..trades).map(|_| swap()).collect::<Result<Vec<_>>>()?,
    )?;
    let pricer = PricerType::new();
    let ordinary_pricer = PortfolioPricer::new(PricerType::new());
    let context = market(0.03);
    let changed = market(0.04);
    let prepare = || {
        if trades == 1 {
            pricer
                .prepare(&portfolio.trades()[0], &context)
                .map(Binding::Trade)
        } else {
            pricer
                .prepare_portfolio(&portfolio, &context)
                .map(Binding::Book)
        }
    };
    let ordinary = |market: &PricingContext| {
        if trades == 1 {
            pricer.evaluate(&portfolio.trades()[0], &requests, market)
        } else {
            ordinary_pricer.evaluate(&portfolio, &requests, market)
        }
    };
    let refresh = |binding: &mut Binding, market: &PricingContext| match binding {
        Binding::Trade(p) => pricer.refresh(p, &portfolio.trades()[0], market),
        Binding::Book(p) => pricer.refresh_portfolio(p, &portfolio, market),
    };
    let (mut binding, prepare_ms) = timed(prepare)?;
    let (first, first_prepared_ms) = timed(|| binding.evaluate(&requests))?;
    let prepared_warm = samples(count, (20000 / trades).max(10), || {
        binding.evaluate(&requests)
    })?;
    let (ordinary_first, first_ordinary_ms) = timed(|| ordinary(&context))?;
    compare(&first, &ordinary_first)?;
    let ordinary_warm = samples(count, (2000 / trades).max(2), || ordinary(&context))?;
    // Alternate actual rate changes; rebind even though the executable graph stays compatible.
    let mut changed_market = false;
    let refresh_only = samples(count, (200 / trades).max(2), || {
        changed_market = !changed_market;
        let reused = refresh(
            &mut binding,
            if changed_market { &changed } else { &context },
        )?;
        if !reused {
            return Err(QSError::EvaluationErr("unexpected graph change".into()));
        }
        Ok(())
    })?;
    let refresh_and_evaluate = samples(count, (200 / trades).max(2), || {
        changed_market = !changed_market;
        refresh(
            &mut binding,
            if changed_market { &changed } else { &context },
        )?;
        binding.evaluate(&requests)
    })?;
    for market in [&changed, &context] {
        refresh(&mut binding, market)?;
        compare(&binding.evaluate(&requests)?, &ordinary(market)?)?;
    }
    let report = json!({"trades":trades,"request_mode":mode,"samples":count,"prepare_ms":prepare_ms,
        "first_prepared_ms":first_prepared_ms,"first_ordinary_ms":first_ordinary_ms,
        "prepared_warm":prepared_warm,"ordinary_warm":ordinary_warm,"refresh":refresh_only,
        "refresh_and_evaluate":refresh_and_evaluate,"snapshot":snapshot(&first)?,"changed_snapshot":snapshot(&ordinary(&changed)?)?});
    fs::write(&arguments[2], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{}",
        json!({"trades":trades,"mode":mode,"prepare_ms":prepare_ms,"prepared_ms":report["prepared_warm"]["median_ms"],
        "ordinary_ms":report["ordinary_warm"]["median_ms"],"refresh_ms":report["refresh"]["median_ms"]})
    );
    Ok(())
}
