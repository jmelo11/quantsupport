use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use quantsupport::prelude::*;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// JSON helpers (same as bootstrap example)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct QuoteRecord {
    identifier: String,
    mid: f64,
}

#[derive(Deserialize)]
struct JsonQuotes {
    reference_date: Date,
    quotes: Vec<QuoteRecord>,
}

#[derive(Deserialize)]
struct JsonCurveSpecs {
    curve_specs: Vec<CurveConfiguration>,
}

fn load_quotes(path: &PathBuf) -> Result<QuoteStore> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonQuotes =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;

    let mut store = QuoteStore::new(json.reference_date);
    for rec in json.quotes {
        let details = QuoteDetails::from_str(&rec.identifier)?;
        let levels = QuoteLevels::with_mid(rec.mid);
        store.add_quote(Quote::new(details, levels));
    }
    Ok(store)
}

fn load_curve_specs(path: &PathBuf) -> Result<Vec<CurveConfiguration>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    Ok(json.curve_specs)
}

// ---------------------------------------------------------------------------
// Pricing helper
// ---------------------------------------------------------------------------

fn price_and_display(
    label: &str,
    trade: &SwapTrade<ADReal>,
    context: &ContextManager,
    csa_index: MarketIndex,
    csa_currency: Currency,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut pricer = CashflowDiscountPricer::<Swap<ADReal>, SwapTrade<ADReal>>::new();
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        csa_index,
        csa_currency,
    )));

    let requests = vec![Request::Value, Request::Sensitivities];
    let results = pricer.evaluate(trade, &requests, context)?;

    let npv = results.price().unwrap_or(f64::NAN);
    println!("═══════════════════════════════════════════════════════════════");
    println!("  {label}");
    println!("  NPV = {npv:>14.2} USD");
    println!("═══════════════════════════════════════════════════════════════\n");

    if let Some(sens) = results.sensitivities() {
        let keys = sens.instrument_keys();
        let exposures = sens.exposure();

        println!("  {:<45} {:>16}", "Pillar", "DV01 (USD/bp)");
        println!("  {}", "-".repeat(63));

        let mut total = 0.0_f64;
        for (key, &exp) in keys.iter().zip(exposures.iter()) {
            let dv01 = exp * 1e-4;
            println!("  {key:<45} {dv01:>16.2}");
            total += dv01;
        }
        println!("  {}", "-".repeat(63));
        println!("  {:<45} {total:>16.2}", "TOTAL");
    }
    println!();
    Ok(())
}

fn price_and_display_xccy(
    label: &str,
    trade: &FloatFloatCrossCurrencySwapTrade<ADReal>,
    context: &ContextManager,
    csa_index: MarketIndex,
    csa_currency: Currency,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut pricer = CashflowDiscountPricer::<
        FloatFloatCrossCurrencySwap<ADReal>,
        FloatFloatCrossCurrencySwapTrade<ADReal>,
    >::new();
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        csa_index,
        csa_currency,
    )));

    let requests = vec![Request::Value, Request::Sensitivities];
    let results = pricer.evaluate(trade, &requests, context)?;

    let npv = results.price().unwrap_or(f64::NAN);
    println!("═══════════════════════════════════════════════════════════════");
    println!("  {label}");
    println!("  NPV = {npv:>14.2} USD");
    println!("═══════════════════════════════════════════════════════════════\n");

    if let Some(sens) = results.sensitivities() {
        let keys = sens.instrument_keys();
        let exposures = sens.exposure();

        println!("  {:<45} {:>16}", "Pillar", "DV01 (USD/bp)");
        println!("  {}", "-".repeat(63));

        let mut total = 0.0_f64;
        for (key, &exp) in keys.iter().zip(exposures.iter()) {
            let dv01 = exp * 1e-4;
            println!("  {key:<45} {dv01:>16.2}");
            total += dv01;
        }
        println!("  {}", "-".repeat(63));
        println!("  {:<45} {total:>16.2}", "TOTAL");
    }
    println!();
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let data_dir = cwd.join("examples/sensitivity/data");

    // ── 1. Load market data and curve specs ───────────────────────────
    let quote_store = load_quotes(&data_dir.join("quotes.json"))?;
    let rd = quote_store.reference_date();
    let curve_specs = load_curve_specs(&data_dir.join("curve_specs.json"))?;

    println!("Reference date : {rd}");
    println!("Curve specs    : {}\n", curve_specs.len());

    // ── 2. Bootstrap SOFR and TermSOFR3m curves ───────────────────────
    let csa_index = MarketIndex::SOFR;
    let csa_currency = Currency::USD;
    // Bootstrap policy: SOFR/USD primary CSA.
    // CLP cashflows discount off Collateral(CLP, USD) — the cross-currency basis curve.
    // ICP self-discounting is resolved automatically (ICP bootstraps after Collateral).
    let policy = BootstrapDiscountPolicy::new(csa_index, csa_currency);

    // FX spot: 1 USD = 935 CLP
    let mut fx_store = ExchangeRateStore::new();
    fx_store.add_exchange_rate(Currency::USD, Currency::CLP, ADReal::new(935.0));

    let bootstrapper =
        MultiCurveBootstrapper::new(curve_specs, policy).with_exchange_rate_store(fx_store);
    let curves = bootstrapper.bootstrap(&quote_store, Level::Mid)?;

    // ── 3. Set up the pricing context ───────────────────────────────
    let mut constructed_elements = ConstructedElementStore::default();
    for (index, elem) in curves {
        constructed_elements
            .discount_curves_mut()
            .insert(index, elem);
    }

    let fixing_store = FixingStore::default();
    let mut pricing_fx_store = ExchangeRateStore::new();
    pricing_fx_store.add_exchange_rate(Currency::USD, Currency::CLP, ADReal::new(935.0));

    let context = ContextManager::new(QuoteStore::new(rd), fixing_store)
        .with_base_currency(Currency::USD)
        .with_constructed_elements(constructed_elements)
        .with_exchange_rate_store(pricing_fx_store);

    let start = rd + Period::from_str("2D")?; // T+2 settlement
    let maturity = start + Period::from_str("5Y")?;
    let notional = 10_000_000.0;

    let rate_def = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Simple,
        Frequency::Semiannual,
    );

    // ── 4a. SOFR swap: receive fixed 3.78% vs pay SOFR quarterly ─────
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
        price_and_display(
            "SOFR OIS 5Y Swap",
            &trade,
            &context,
            MarketIndex::SOFR,
            Currency::USD,
        )?;
    }

    // ── 4b. TermSOFR3m swap: receive fixed 3.85% vs pay TermSOFR3m quarterly
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
        price_and_display(
            "TermSOFR3m 5Y Swap",
            &trade,
            &context,
            MarketIndex::SOFR,
            Currency::USD,
        )?;
    }

    // ── 4c. CLP ICP OIS swap: receive fixed 4.40% vs pay ICP quarterly ──
    {
        let clp_notional = 5_000_000_000.0; // 5 billion CLP
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
        price_and_display(
            "CLP ICP OIS 5Y Swap (USD collateral)",
            &trade,
            &context,
            MarketIndex::SOFR,
            Currency::USD,
        )?;
    }

    // ── 4d. Cross-currency swap: receive CLP ICP vs pay USD SOFR ──
    {
        let usd_notional = 10_000_000.0;
        let clp_notional_xccy = usd_notional * 935.0; // at FX spot

        let xccy = MakeFloatFloatCrossCurrencySwap::<ADReal>::default()
            .with_identifier("XCCY_CLP_ICP_SOFR_USD_5Y".to_string())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_domestic_notional(clp_notional_xccy)
            .with_foreign_notional(usd_notional)
            .with_domestic_spread(0.0050) // 50 bps spread on CLP leg
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
        price_and_display_xccy(
            "Cross-Currency Swap CLP/USD 5Y (receive CLP ICP, pay USD SOFR)",
            &trade,
            &context,
            MarketIndex::SOFR,
            Currency::USD,
        )?;
    }

    Ok(())
}
