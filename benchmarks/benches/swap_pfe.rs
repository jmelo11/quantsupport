use std::collections::HashMap;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use quantsupport::prelude::*;

/// Build a flat f64 discount curve at the given continuously-compounded rate.
fn flat_curve(ref_date: Date, rate: f64, max_years: u32) -> DiscountTermStructure<f64> {
    let dc = DayCounter::Actual365;
    let n_points = (max_years * 4) as usize; // quarterly
    let mut dates = Vec::with_capacity(n_points + 1);
    let mut dfs = Vec::with_capacity(n_points + 1);

    dates.push(ref_date);
    dfs.push(1.0);

    for i in 1..=n_points {
        let d = ref_date.advance(3 * i as i32, TimeUnit::Months);
        let t = dc.year_fraction(ref_date, d);
        dates.push(d);
        dfs.push((-rate * t).exp());
    }

    DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)
        .expect("Failed to build flat curve")
}

/// Build N vanilla 5Y USD IRS trades (receive fixed, pay SOFR).
fn create_swaps(n: usize, ref_date: Date) -> Vec<SwapTrade<f64>> {
    let maturities = [2, 3, 5, 7, 10]; // years
    let fixed_rates = [0.025, 0.030, 0.035, 0.0378, 0.042];
    let notionals = [
        1_000_000.0,
        5_000_000.0,
        10_000_000.0,
        25_000_000.0,
        50_000_000.0,
    ];
    let sides = [Side::LongReceive, Side::PayShort];
    let fixed_freqs = [
        Frequency::Semiannual,
        Frequency::Quarterly,
        Frequency::Annual,
    ];
    let float_freqs = [Frequency::Semiannual, Frequency::Quarterly];

    (0..n)
        .map(|i| {
            let maturity = maturities[i % maturities.len()];
            let rate = fixed_rates[i % fixed_rates.len()];
            let notional = notionals[i % notionals.len()];
            let side = sides[i % sides.len()];
            let fixed_freq = fixed_freqs[i % fixed_freqs.len()];
            let float_freq = float_freqs[i % float_freqs.len()];

            let swap = MakeSwap::<f64>::default()
                .with_identifier(format!("USD_IRS_{maturity}Y_{i}"))
                .with_start_date(ref_date)
                .with_maturity_date(ref_date.advance(maturity as i32, TimeUnit::Years))
                .with_fixed_rate(rate)
                .with_notional(notional)
                .with_rate_definition(RateDefinition::new(
                    DayCounter::Actual360,
                    Compounding::Simple,
                    Frequency::Semiannual,
                ))
                .with_currency(Currency::USD)
                .with_market_index(MarketIndex::SOFR)
                .with_side(side)
                .with_fixed_leg_frequency(fixed_freq)
                .with_floating_leg_frequency(float_freq)
                .build()
                .expect("Failed to build swap");
            SwapTrade::new(swap, ref_date, notional, side)
        })
        .collect()
}

/// Run the full PFE pipeline: decompose → inspect → simulate → evaluate.
fn run_pfe(trades: &[SwapTrade<f64>], curve: &DiscountTermStructure<f64>, ref_date: Date) {
    let dc = DayCounter::Actual365;

    // 1. Decompose all trades into contingent claims
    let mut all_claims = Vec::new();
    let mut trade_ranges: Vec<(String, usize, usize)> = Vec::with_capacity(trades.len());

    for trade in trades {
        let claims = trade
            .into_contingent_claims()
            .expect("Failed to decompose IRS");
        let start = all_claims.len();
        let n = claims.len();
        all_claims.extend(claims);
        trade_ranges.push((
            trade.instrument().identifier().to_string(),
            start,
            start + n,
        ));
    }

    // 2. Inspector: assign indices & collect simulation requests
    let discount_policy = SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
    let mut inspector = PreprocessorExecutor::with_discount_policy(Box::new(discount_policy));
    inspector.visit(&mut all_claims);
    let requests: Vec<_> = inspector.requests().to_vec();

    // 3. Build LGM market model (single-currency USD)
    let usd_rate = LgmRateModel::new(0.05, 0.01, curve);

    let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, ref_date, dc)
        .with_n_paths(500)
        .with_seed(42);

    model.add_curve_model(MarketIndex::SOFR, usd_rate);

    // 4. Evaluation dates (monthly out to 5Y)
    let max_maturity = ref_date.advance(10, TimeUnit::Years);
    let schedule = MakeSchedule::new(ref_date, max_maturity)
        .with_frequency(Frequency::Monthly)
        .build()
        .expect("Failed to build evaluation schedule");
    let dates = schedule.dates().clone();

    model.set_evaluation_dates(dates.clone());
    model.set_requests(requests);

    // 5. Run ExposureEvaluator
    let evaluator = ExposureEvaluator::<f64>::new(dates, &model);

    let mut trades_map: HashMap<String, &[_]> = HashMap::new();
    for (id, start, end) in &trade_ranges {
        trades_map.insert(id.clone(), &all_claims[*start..*end]);
    }

    let results = evaluator.evaluate(&trades_map);

    // Prevent dead-code elimination
    assert!(results.is_ok());
}

fn bench_swap_pfe(c: &mut Criterion) {
    let ref_date = Date::new(2024, 1, 15);
    let curve = flat_curve(ref_date, 0.04, 15);

    let mut group = c.benchmark_group("swap_pfe");
    // PFE is heavier — reduce sample size so each iteration is measured well
    group.sample_size(10);

    for n in [100, 1000] {
        let trades = create_swaps(n, ref_date);

        group.bench_with_input(BenchmarkId::new("pfe", n), &trades, |b, trades| {
            b.iter(|| run_pfe(trades, &curve, ref_date));
        });
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_swap_pfe
}
criterion_main!(benches);
