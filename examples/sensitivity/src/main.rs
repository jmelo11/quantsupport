mod curves;
mod output;
mod pricing;

use quantsupport::prelude::*;

use curves::{build_curves, load_curve_specs, load_quotes};
use output::{extract_curve_nodes, CurveOutput, OutputResults, ProductOutput};
use pricing::price_product;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let data_dir = cwd.join("examples/sensitivity/data");

    // ── 1. Load market data and curve specs ───────────────────────────
    let quote_store = load_quotes(&data_dir.join("quotes.json"))?;
    let rd = quote_store.reference_date();
    let curve_specs = load_curve_specs(&data_dir.join("curve_specs.json"))?;

    println!("Reference date : {rd}");
    println!("Curve specs    : {}\n", curve_specs.len());

    // ── 2. Bootstrap curves and build pricing context ─────────────────
    let env = build_curves(&quote_store, curve_specs)?;

    let start = rd + Period::from_str("2D")?; // T+2 settlement
    let maturity = start + Period::from_str("5Y")?;
    let notional = 10_000_000.0;
    let dc = DayCounter::Actual360;

    let rate_def = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Simple,
        Frequency::Semiannual,
    );

    let mut products: Vec<ProductOutput> = Vec::new();

    // ── 3a. SOFR swap: receive fixed 3.78% vs pay SOFR quarterly ─────
    {
        let swap = MakeSwap::<ADReal>::default()
            .with_identifier("USD_SOFR_IRS_5Y".to_string())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_fixed_rate(0.0378)
            .with_notional(notional)
            .with_rate_definition(rate_def)
            .with_currency(Currency::USD)
            .with_market_index(MarketIndex::SOFR)
            .with_side(Side::LongReceive)
            .with_fixed_leg_frequency(Frequency::Semiannual)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .build()?;

        let trade = SwapTrade::new(swap, start, notional, Side::LongReceive);
        let output = price_product::<Swap<ADReal>, SwapTrade<ADReal>>(
            "SOFR OIS 5Y Swap",
            &trade,
            &env.context,
            MarketIndex::SOFR,
            Currency::USD,
            &env.curve_lookup,
        )?;
        products.push(output);
    }

    // ── 3b. TermSOFR3m swap: receive fixed 4.00% vs pay TermSOFR3m quarterly
    {
        let swap = MakeSwap::<ADReal>::default()
            .with_identifier("USD_TermSOFR3m_IRS_5Y".to_string())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_fixed_rate(0.04)
            .with_notional(notional)
            .with_rate_definition(rate_def)
            .with_currency(Currency::USD)
            .with_market_index(MarketIndex::TermSOFR3m)
            .with_side(Side::LongReceive)
            .with_fixed_leg_frequency(Frequency::Semiannual)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .build()?;

        let trade = SwapTrade::new(swap, start, notional, Side::LongReceive);
        let output = price_product::<Swap<ADReal>, SwapTrade<ADReal>>(
            "TermSOFR3m 5Y Swap",
            &trade,
            &env.context,
            MarketIndex::SOFR,
            Currency::USD,
            &env.curve_lookup,
        )?;
        products.push(output);
    }

    // ── 3c. CLP ICP OIS swap: receive fixed 4.40% vs pay ICP quarterly ──
    {
        let clp_notional = 5_000_000_000.0;
        let clp_rate_def = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        );

        let swap = MakeSwap::<ADReal>::default()
            .with_identifier("CLP_ICP_OIS_5Y".to_string())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_fixed_rate(0.0440)
            .with_notional(clp_notional)
            .with_rate_definition(clp_rate_def)
            .with_currency(Currency::CLP)
            .with_market_index(MarketIndex::ICP)
            .with_side(Side::LongReceive)
            .with_fixed_leg_frequency(Frequency::Semiannual)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .build()?;

        let trade = SwapTrade::new(swap, start, clp_notional, Side::LongReceive);
        let output = price_product::<Swap<ADReal>, SwapTrade<ADReal>>(
            "CLP ICP OIS 5Y Swap (USD collateral)",
            &trade,
            &env.context,
            MarketIndex::SOFR,
            Currency::USD,
            &env.curve_lookup,
        )?;
        products.push(output);
    }

    // ── 3d. Cross-currency swap: receive CLP ICP vs pay USD SOFR ──
    {
        let usd_notional = 10_000_000.0;
        let clp_notional_xccy = usd_notional * 935.0;

        let xccy = MakeFloatFloatCrossCurrencySwap::<ADReal>::default()
            .with_identifier("XCCY_CLP_ICP_SOFR_USD_5Y".to_string())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_domestic_notional(clp_notional_xccy)
            .with_foreign_notional(usd_notional)
            .with_domestic_spread(0.0050)
            .with_domestic_currency(Currency::CLP)
            .with_foreign_currency(Currency::USD)
            .with_domestic_market_index(MarketIndex::ICP)
            .with_foreign_market_index(MarketIndex::SOFR)
            .with_side(Side::LongReceive)
            .with_domestic_leg_frequency(Frequency::Quarterly)
            .with_foreign_leg_frequency(Frequency::Quarterly)
            .build()?;

        let trade = FloatFloatCrossCurrencySwapTrade::new(
            xccy,
            start,
            clp_notional_xccy,
            usd_notional,
            Side::LongReceive,
        );
        let output = price_product::<
            FloatFloatCrossCurrencySwap<ADReal>,
            FloatFloatCrossCurrencySwapTrade<ADReal>,
        >(
            "Cross-Currency Swap CLP/USD 5Y (receive CLP ICP, pay USD SOFR)",
            &trade,
            &env.context,
            MarketIndex::SOFR,
            Currency::USD,
            &env.curve_lookup,
        )?;
        products.push(output);
    }

    // ── 4. Extract curve nodes ─────────────────────────────────────
    let mut curve_outputs: Vec<CurveOutput> = Vec::new();
    for (index, elem) in &env.curve_lookup {
        let name = format!("{index}");
        curve_outputs.push(extract_curve_nodes(&name, elem, rd, dc));
    }
    curve_outputs.sort_by(|a, b| a.name.cmp(&b.name));

    // ── 5. Write JSON results ──────────────────────────────────────
    let results = OutputResults {
        reference_date: rd.to_string(),
        curves: curve_outputs,
        products,
    };

    let output_path = data_dir.join("rust_results.json");
    output::write_results(&results, &output_path)?;

    Ok(())
}
