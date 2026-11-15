//! Shared end-to-end swap PFE workload for Criterion and cross-branch checks.

use quantsupport::prelude::*;
use std::{collections::HashMap, time::Instant};

/// Build a flat f64 discount curve at the given continuously-compounded rate.
fn flat_curve(ref_date: Date, rate: f64, max_years: u32) -> DiscountTermStructure<f64> {
    let dc = DayCounter::Actual365;
    let n_points = (max_years * 4) as usize; // quarterly
    let mut dates = Vec::with_capacity(n_points + 1);
    let mut dfs = Vec::with_capacity(n_points + 1);

    dates.push(ref_date);
    dfs.push(1.0);

    for i in 1..=n_points {
        let d = ref_date
            .advance(3 * i as i32, TimeUnit::Months)
            .expect("valid date advancement");
        let t = dc.year_fraction(ref_date, d);
        dates.push(d);
        dfs.push((-rate * t).exp());
    }

    DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)
        .expect("Failed to build flat curve")
}

/// Builds USD IRS trades with alternating sides and 2/3/5/7/10-year maturities.
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
                .with_maturity_date(
                    ref_date
                        .advance(maturity, TimeUnit::Years)
                        .expect("valid date advancement"),
                )
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

/// Fixed trades, discount curve and path count used by repeated full-pipeline evaluations.
pub struct SwapPfeFixture {
    trades: Vec<SwapTrade<f64>>,
    curve: DiscountTermStructure<f64>,
    reference: Date,
    paths: usize,
}

/// Cubes and cost boundaries from one complete inspection/simulation/evaluation.
pub struct SwapPfeRun {
    result: ExposureResult,
    claims: usize,
    requests: usize,
    setup_ms: f64,
    evaluation_ms: f64,
}

impl SwapPfeRun {
    /// Borrows all per-trade cubes without normalizing or aggregating them.
    #[must_use]
    pub const fn result(&self) -> &ExposureResult {
        &self.result
    }
    /// Returns the number of original claims, including every trade's cashflows.
    #[must_use]
    pub const fn claims(&self) -> usize {
        self.claims
    }
    /// Returns the number of market-response slots produced by preprocessing.
    #[must_use]
    pub const fn requests(&self) -> usize {
        self.requests
    }
    /// Returns milliseconds spent decomposing, inspecting and configuring the model.
    #[must_use]
    pub const fn setup_ms(&self) -> f64 {
        self.setup_ms
    }
    /// Returns milliseconds spent generating paths and populating all trade cubes.
    #[must_use]
    pub const fn evaluation_ms(&self) -> f64 {
        self.evaluation_ms
    }
}

impl SwapPfeFixture {
    /// Constructs the original mixed-maturity portfolio and fixed-seed path workload.
    #[must_use]
    pub fn new(trades: usize, paths: usize) -> Self {
        let reference = Date::new(2024, 1, 15);
        Self {
            trades: create_swaps(trades, reference),
            curve: flat_curve(reference, 0.04, 15),
            reference,
            paths,
        }
    }

    /// Runs decomposition, request inspection, simulation and full per-trade cube evaluation.
    ///
    /// The monthly grid spans ten years. Enabling expiration removes observations
    /// only after a claim pays; every trade, path and output cube is retained.
    ///
    /// # Errors
    /// Propagates path-generation and claim-evaluation failures.
    pub fn run(&self, expire_requests: bool) -> Result<SwapPfeRun> {
        let start = Instant::now();
        let dc = DayCounter::Actual365;

        // 1. Decompose all trades into contingent claims
        let mut all_claims = Vec::new();
        let mut trade_ranges: Vec<(String, usize, usize)> = Vec::with_capacity(self.trades.len());

        for trade in &self.trades {
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
        let mut netting_set = NettingSet::new(all_claims, Box::new(discount_policy));

        let mut inspector = PreprocessorExecutor::new();
        inspector.visit(std::iter::once(&mut netting_set));
        let mut requests = inspector.requests().to_vec();
        if !expire_requests {
            for request in &mut requests {
                request.expiration_date = None;
            }
        }

        // 3. Build LGM market model (single-currency USD)
        let usd_rate = LgmRateModel::new(0.05, 0.005, &self.curve);

        let mut model = LgmMarketModel::new(Currency::USD, MarketIndex::SOFR, self.reference, dc)
            .with_n_paths(self.paths)
            .with_seed(42);

        model.add_curve_model(MarketIndex::SOFR, usd_rate);

        // 4. Evaluation dates (monthly out to 10Y)
        let max_maturity = self
            .reference
            .advance(10, TimeUnit::Years)
            .expect("valid date advancement");
        let schedule = MakeSchedule::new(self.reference, max_maturity)
            .with_frequency(Frequency::Monthly)
            .build()
            .expect("Failed to build evaluation schedule");
        let dates = schedule.dates().clone();

        model.set_evaluation_dates(dates.clone());
        let request_count = requests.len();
        model.set_requests(requests);

        // 5. Run ExposureEvaluator
        let evaluator = ExposureEvaluator::<f64>::new(dates, &model);

        let mut trades_map: HashMap<String, &[_]> = HashMap::new();
        let claims = netting_set.claims();
        for (id, start, end) in &trade_ranges {
            trades_map.insert(id.clone(), &claims[*start..*end]);
        }

        let setup_ms = start.elapsed().as_secs_f64() * 1e3;
        let evaluation = Instant::now();
        let result = evaluator.evaluate(&trades_map)?;
        Ok(SwapPfeRun {
            result,
            claims: claims.len(),
            requests: request_count,
            setup_ms,
            evaluation_ms: evaluation.elapsed().as_secs_f64() * 1e3,
        })
    }
}
