//! Cross-branch timings and complete numerical snapshots of XvaEngine::run.

use std::{
    collections::HashMap,
    env::args,
    error::Error,
    fs::{self, File},
    hint::black_box,
    path::PathBuf,
    result::Result as StdResult,
    time::Instant,
};

use quantsupport::prelude::*;

type BenchmarkScalar = Recorded;

use benchmarks::xva_fixture::{
    load_credit_curve_specs, load_curve_specs, load_quotes, load_vol_specs,
};

/// Runs the standalone comparison driver and emits its timing and numerical results.
fn main() -> StdResult<(), Box<dyn Error>> {
    let setup_start = Instant::now();
    let args: Vec<_> = args().skip(1).collect();
    let mode = args
        .first()
        .map(String::as_str)
        .ok_or("missing benchmark mode")?;
    if !matches!(
        mode,
        "--benchmark" | "--benchmark-prepared" | "--benchmark-refresh"
    ) {
        return Err("expected --benchmark, --benchmark-prepared or --benchmark-refresh".into());
    }
    let refresh = mode == "--benchmark-refresh";
    let benchmark_paths: usize = args.get(1).ok_or("missing path count")?.parse()?;
    let benchmark_samples: usize = args.get(2).ok_or("missing sample count")?.parse()?;
    if benchmark_paths == 0 || benchmark_samples == 0 || !(4..=6).contains(&args.len()) {
        return Err("usage: <mode> <positive paths> <positive samples> <result.json> [margin] [FX multiplier]".into());
    }
    let margin_override = args.get(4).map(|value| value.parse::<bool>()).transpose()?;
    let fx_multiplier = args.get(5).map_or(Ok(1.0), |value| value.parse::<f64>())?;
    if !fx_multiplier.is_finite() || fx_multiplier <= 0.0 {
        return Err("FX multiplier must be finite and positive".into());
    }
    // ── 1. Build PricingContext from market data ────────────────
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = manifest_dir.join("data/xva");

    let quote_store = load_quotes(&data_dir.join("quotes.json"))?;
    let ref_date = quote_store.reference_date();
    let curve_specs = load_curve_specs(&data_dir.join("curve_specs.json"))?;
    let credit_curve_specs = load_credit_curve_specs(&data_dir.join("credit_curve_specs.json"))?;

    // Only bootstrap curves we need: SOFR, ICP, Collateral(CLP, USD) and the
    // TermSOFR3m funding-index curve, bootstrapped from a 3M deposit plus
    // OIS-vs-TermSOFR basis swaps.
    let curve_specs: Vec<_> = curve_specs
        .into_iter()
        .filter(|s| *s.market_index() != MarketIndex::ESTR)
        .collect();

    // Volatility inputs: a SOFR caplet surface and an ICP swaption cube. The
    // XVA engine calibrates its LGM sigma schedules against them.
    let (volatility_surfaces, volatility_cubes) =
        load_vol_specs(&data_dir.join("vol_specs.json"))?.into_configurations();

    println!("Reference date: {ref_date}");

    // FX spot: 1 CLP = 1/900 USD  (i.e. 900 CLP per 1 USD)
    let mut fx_store = FxStore::new();
    fx_store.add_fx_rate(
        Currency::CLP,
        Currency::USD,
        BenchmarkScalar::from((if refresh { 1.0 } else { fx_multiplier }) / 900.0),
    );

    let mut ctx = PricingContext::new()
        .with_quote_store(quote_store)
        .with_fx_store(fx_store)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
        .with_curve_configurations(curve_specs)
        .with_credit_curve_configurations(credit_curve_specs)
        .with_volatility_surface_configurations(volatility_surfaces)
        .with_volatility_cube_configurations(volatility_cubes);
    ctx.initialize()?;

    println!("Curves bootstrapped (SOFR, ICP, Collateral CLP/USD, TermSOFR3m funding).");
    println!("CORPORATE-A USD credit curve bootstrapped from 1Y, 3Y, 5Y and 10Y CDS quotes.");
    println!("Volatility surface (SOFR caplets) and cube (ICP swaptions) built.");

    // ── 2. Build trades ─────────────────────────────────────────
    let swap = MakeSwap::<f64>::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(ref_date)
        .with_maturity_date(ref_date.advance(5, TimeUnit::Years)?)
        .with_fixed_rate(0.0378)
        .with_notional(10_000_000.0)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        ))
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Quarterly)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()?;
    let irs_trade = SwapTrade::new(swap, ref_date, 10_000_000.0, Side::LongReceive);
    let irs_claims = irs_trade.into_contingent_claims()?;
    println!("IRS claims: {}", irs_claims.len());

    // Cross-currency float/float swap: receive CLP/ICP, pay USD/SOFR
    let fx_clpusd = 900.0_f64; // 900 CLP per 1 USD
    let xccy = MakeFloatFloatCrossCurrencySwap::<f64>::default()
        .with_identifier("CLPUSD_XCCY_5Y".to_string())
        .with_start_date(ref_date)
        .with_maturity_date(ref_date.advance(5, TimeUnit::Years)?)
        .with_domestic_notional(10_000_000.0)
        .with_foreign_notional(10_000_000.0 * fx_clpusd)
        .with_domestic_spread(0.0)
        .with_foreign_spread(0.002)
        .with_domestic_currency(Currency::USD)
        .with_foreign_currency(Currency::CLP)
        .with_domestic_market_index(MarketIndex::SOFR)
        .with_foreign_market_index(MarketIndex::ICP)
        .with_side(Side::LongReceive)
        .with_domestic_leg_frequency(Frequency::Semiannual)
        .with_foreign_leg_frequency(Frequency::Semiannual)
        .build()?;

    let xccy_claims = xccy
        .legs()
        .to_vec()
        .into_contingent_claims("CLPUSD_XCCY_5Y")?;
    println!("XCCY claims: {}", xccy_claims.len());

    // ── 3. Configure & run XVA engine ───────────────────────────
    // The LGM models for SOFR and ICP are calibrated against the caplet
    // surface and swaption cube constructed in the pricing context. The
    // Collateral(CLP, USD) curve has no free parameters: it sets
    // `"driver": "ICP"`, so it is simulated with the ICP model's factor and
    // calibrated vols on top of its own FX-implied initial term structure
    // (deterministic cross-currency basis).
    let mut config: XvaEngineConfig =
        serde_json::from_reader(File::open(data_dir.join("xva_config.json"))?)?;
    config.n_paths = benchmark_paths;

    let mut engine = XvaEngine::new(&ctx, config)?;

    // Both trades share the same CSA → single netting set.
    // The client's CSA terms carry the collateral treatment and the
    // credit/funding parameters used for CVA/FVA. FVA spreads combine the
    // OIS-vs-TermSOFR basis implied by the bootstrapped funding curve
    // (`funding_index`) with the bank's own funding spread curve over
    // TermSOFR (`funding_spread_curve`).
    let mut csa: CsaTerms = serde_json::from_reader(File::open(data_dir.join("csa_terms.json"))?)?;
    if let Some(margin) = margin_override {
        csa.variation_margin = margin;
    }
    let margin_enabled = csa.variation_margin;
    println!(
        "CSA: variation margin = {}, thresholds = {}/{}, MTA = {} {}.",
        csa.variation_margin,
        csa.counterparty_threshold,
        csa.own_threshold,
        csa.minimum_transfer_amount,
        csa.collateral_currency
    );
    let n_claims = irs_claims.len() + xccy_claims.len();
    let mut all_claims = irs_claims;
    all_claims.extend(xccy_claims);

    let mut netting_sets = HashMap::new();
    netting_sets.insert(
        "portfolio".to_string(),
        NettingSet::with_csa_terms(all_claims, csa),
    );

    println!("Running XVA engine with {n_claims} claims...");
    let setup_ms = setup_start.elapsed().as_secs_f64() * 1e3;
    // BEGIN CANDIDATE PREPARATION
    let prepared = mode != "--benchmark";
    let mut preparation_ms = 0.0;
    let mut initial_preparation_ms = 0.0;
    let mut market_context_update_ms = 0.0;
    let mut engine_market_update_ms = 0.0;
    let mut reused_code = false;
    if prepared {
        let start = Instant::now();
        engine.prepare(&mut netting_sets)?;
        preparation_ms = start.elapsed().as_secs_f64() * 1e3;
    }
    if refresh {
        initial_preparation_ms = preparation_ms;
        let _ = engine.run(&mut netting_sets)?;
        let start = Instant::now();
        let mut current_fx = FxStore::new();
        current_fx.add_fx_rate(
            Currency::CLP,
            Currency::USD,
            BenchmarkScalar::from(fx_multiplier / 900.0),
        );
        ctx = ctx.with_fx_store(current_fx);
        ctx.initialize()?;
        market_context_update_ms = start.elapsed().as_secs_f64() * 1e3;
        let start = Instant::now();
        engine.update_market(&ctx)?;
        engine_market_update_ms = start.elapsed().as_secs_f64() * 1e3;
        let start = Instant::now();
        engine.prepare(&mut netting_sets)?;
        preparation_ms = start.elapsed().as_secs_f64() * 1e3;
        reused_code = engine.preparation_reused_code();
        if !reused_code {
            return Err("market refresh unexpectedly recompiled the path program".into());
        }
    }
    // END CANDIDATE PREPARATION
    let start = Instant::now();
    let result = engine.run(&mut netting_sets)?;
    let first_run_ms = start.elapsed().as_secs_f64() * 1e3;
    let mut times = Vec::new();
    for _ in 0..benchmark_samples {
        let start = Instant::now();
        black_box(engine.run(&mut netting_sets)?);
        times.push(start.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(f64::total_cmp);
    let output = serde_json::json!({
        "paths":benchmark_paths, "samples":benchmark_samples, "setup_ms":setup_ms,
        "margin":margin_enabled, "fx_multiplier":fx_multiplier,
        "preparation_ms":preparation_ms, "initial_preparation_ms":initial_preparation_ms,
        "market_context_update_ms":market_context_update_ms,
        "engine_market_update_ms":engine_market_update_ms, "reused_code":reused_code,
        "first_run_ms":first_run_ms, "median_run_ms":times[times.len()/2],
        "sensitivities":result.sensitivities,
        "values":result.xva_values.as_ref().map(|values| values.iter().map(|v| serde_json::json!({"netting_set":v.netting_set,"measure":v.measure,"value":v.value})).collect::<Vec<_>>()),
        "cubes":result.cubes.iter().map(|cube| serde_json::json!({"trade_id":cube.trade_id,"dates":cube.dates.iter().map(ToString::to_string).collect::<Vec<_>>(),"npvs":cube.npvs})).collect::<Vec<_>>()
    });
    if let Some(path) = args.get(3) {
        fs::write(path, serde_json::to_vec(&output)?)?;
    }
    println!("Same XvaEngine::run; setup={setup_ms:.3} ms; first run={first_run_ms:.3} ms; median run={:.3} ms", times[times.len()/2]);
    Ok(())
}
