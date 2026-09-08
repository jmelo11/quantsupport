mod utils;

use std::collections::HashMap;

use quantsupport::prelude::*;

use utils::{load_curve_specs, load_quotes, load_vol_specs};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // ── 1. Build PricingContext from market data ────────────────
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = manifest_dir.join("data");

    let quote_store = load_quotes(&data_dir.join("quotes.json"))?;
    let ref_date = quote_store.reference_date();
    let curve_specs = load_curve_specs(&data_dir.join("curve_specs.json"))?;
    
    // Only bootstrap curves we need: SOFR, ICP, Collateral(CLP, USD) and the
    // TermSOFR3m funding-index curve, bootstrapped from a 3M deposit plus
    // OIS-vs-TermSOFR basis swaps.
    let curve_specs: Vec<_> = curve_specs
        .into_iter()
        .filter(|s| *s.market_index() != MarketIndex::ESTR)
        .collect();

    // Volatility inputs: a SOFR caplet surface and an ICP swaption cube. The
    // XVA engine calibrates its LGM sigma schedules against them.
    let vol_specs = load_vol_specs(&data_dir.join("vol_specs.json"))?;

    println!("Reference date: {ref_date}");

    // FX spot: 1 CLP = 1/900 USD  (i.e. 900 CLP per 1 USD)
    let mut fx_store = FxStore::new();
    fx_store.add_fx_rate(Currency::CLP, Currency::USD, DualFwd::from(1.0 / 900.0));

    let mut ctx = PricingContext::new()
        .with_quote_store(quote_store)
        .with_fx_store(fx_store)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
        .with_curve_configurations(curve_specs)
        .with_volatility_surface_configurations(vol_specs.volatility_surfaces)
        .with_volatility_cube_configurations(vol_specs.volatility_cubes);
    ctx.initialize()?;

    println!("Curves bootstrapped (SOFR, ICP, Collateral CLP/USD, TermSOFR3m funding).");
    println!("Volatility surface (SOFR caplets) and cube (ICP swaptions) built.");

    // ── 2. Build trades ─────────────────────────────────────────
    let swap = MakeSwap::<f64>::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(ref_date)
        .with_maturity_date(ref_date.advance(5, TimeUnit::Years))
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
        .with_maturity_date(ref_date.advance(5, TimeUnit::Years))
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
    let config: XvaEngineConfig =
        serde_json::from_reader(std::fs::File::open(data_dir.join("xva_config.json"))?)?;

    let mut engine = XvaEngine::new(&ctx, config)?;

    // Both trades share the same CSA → single netting set.
    // The client's CSA terms carry the collateral treatment and the
    // credit/funding parameters used for CVA/FVA. FVA spreads combine the
    // OIS-vs-TermSOFR basis implied by the bootstrapped funding curve
    // (`funding_index`) with the bank's own funding spread curve over
    // TermSOFR (`funding_spread_curve`).
    let csa: CsaTerms =
        serde_json::from_reader(std::fs::File::open(data_dir.join("csa_terms.json"))?)?;
    let n_claims = irs_claims.len() + xccy_claims.len();
    let mut all_claims = irs_claims;
    all_claims.extend(xccy_claims);

    let mut netting_sets = HashMap::new();
    netting_sets.insert(
        "portfolio".to_string(),
        NettingSet::with_csa_terms(all_claims, csa),
    );

    println!("Running XVA engine with {n_claims} claims...");
    let result = engine.run(&mut netting_sets)?;

    // ── 4. Display results ──────────────────────────────────────
    println!("\nXVA Values:");
    if let Some(ref xva_values) = result.xva_values {
        for v in xva_values {
            println!(
                "  {:<12} {:<6} = {:>12.2}",
                v.netting_set, v.measure, v.value
            );
        }
    }

    println!("\nCurve pillar sensitivities (dXVA/dquote):");
    if let Some(ref sensitivities) = result.sensitivities {
        let mut sens = sensitivities.clone();
        sens.sort_by(|a, b| a.0.cmp(&b.0));
        let is_credit = |label: &str| label.contains(".CVA.") || label.contains(".FVA.");
        for (label, value) in &sens {
            if !is_credit(label) && value.abs() > 1e-10 {
                println!("  {label:<30} = {value:>12.6}");
            }
        }

        println!("\nCredit & funding sensitivities:");
        for (label, value) in &sens {
            if is_credit(label) {
                println!("  {label:<30} = {value:>12.6}");
            }
        }
    }

    println!("\nExposure profile (from cube):");
    for cube in &result.cubes {
        let epe = cube.epe();
        let ene = cube.ene();
        let ee = cube.ee();
        println!("  Trade: {}", cube.trade_id);
        println!("  {:<8} {:>14} {:>14} {:>14}", "Date", "EPE", "ENE", "EE");
        for (i, d) in cube.dates.iter().enumerate().take(12) {
            println!(
                "  {:<8} {:>14.2} {:>14.2} {:>14.2}",
                d, epe[i], ene[i], ee[i]
            );
        }
        if cube.dates.len() > 12 {
            println!("  ... ({} more dates)", cube.dates.len() - 12);
        }
    }

    Ok(())
}
