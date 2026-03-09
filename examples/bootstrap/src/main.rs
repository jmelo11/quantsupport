use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use quantsupport::prelude::*;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// JSON deserialization helpers
// ---------------------------------------------------------------------------

/// A single quote record as stored in the JSON file.
#[derive(Deserialize)]
struct QuoteRecord {
    identifier: String,
    mid: f64,
}

/// Top-level JSON structure for the quotes file.
#[derive(Deserialize)]
struct JsonQuotes {
    reference_date: Date,
    quotes: Vec<QuoteRecord>,
}

/// Top-level JSON structure for the curve-specs file.
#[derive(Deserialize)]
struct JsonCurveSpecs {
    curve_specs: Vec<CurveSpec>,
}

/// Loads quotes from a JSON file into a [`QuoteStore`].
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

/// Loads curve specifications from a JSON file.
fn load_curve_specs(path: &PathBuf) -> Result<Vec<CurveSpec>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    Ok(json.curve_specs)
}

/// Prints the curve details
fn print_bootstrap_results(
    rd: Date,
    curves: &HashMap<MarketIndex, DiscountCurveElement>,
) -> Result<()> {
    for (index, elem) in curves {
        let curve = elem.curve();
        println!("=== {index} ===\n");
        println!(
            "{:<42} {:>12} {:>14} {:>14}",
            "Pillar", "Quote (%)", "DF", "Zero Rate (%)"
        );
        println!("{}", "-".repeat(84));

        if let Some(pillars) = curve.pillars() {
            for (label, quote_val) in &pillars {
                let tenor_str = label.rsplit('_').next().unwrap_or("0D");
                let pillar_date = rd + Period::from_str(tenor_str).unwrap();
                let df = curve.discount_factor(pillar_date)?.value();
                let yf = DayCounter::Actual360.year_fraction(rd, pillar_date);
                let zero = if yf > 0.0 { -df.ln() / yf * 100.0 } else { 0.0 };
                println!(
                    "{label:<42} {:>12.4} {df:>14.8} {zero:>14.4}",
                    quote_val.value() * 100.0
                );
            }
        }

        // Interpolated discount factors
        println!("\n  --- Interpolated DFs ---");
        for tenor in &["6M", "4Y", "15Y", "20Y"] {
            let d = rd + Period::from_str(tenor)?;
            let df = curve.discount_factor(d)?.value();
            println!("    DF({tenor:>3}) = {df:.8}");
        }
        println!();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let data_dir = cwd.join("examples/bootstrap/data");

    // 1. Load market quotes
    let quote_store = load_quotes(&data_dir.join("quotes.json"))?;
    let rd = quote_store.reference_date();
    println!("Reference date: {rd}");

    // 2. Load curve specifications from JSON
    let curve_specs = load_curve_specs(&data_dir.join("curve_specs.json"))?;
    println!("Loaded {} curve spec(s)\n", curve_specs.len());

    // 3. Bootstrap all curves
    //    The discount policy uses the first curve as the CSA / collateral
    //    curve (typically SOFR for USD).
    //    CLP cashflows are discounted with the Collateral(CLP, USD) curve,
    //    which is bootstrapped from cross-currency swaps.
    let csa_index = curve_specs[0].market_index().clone();
    let csa_currency = curve_specs[0].currency();
    let policy = BootstrapDiscountPolicy::new(csa_index, csa_currency).with_collateral_curve(
        Currency::CLP,
        MarketIndex::Collateral(Currency::CLP, Currency::USD),
    );

    // FX spot from fixings: 1 USD = 935 CLP
    let mut fx_store = ExchangeRateStore::new();
    fx_store.add_exchange_rate(Currency::USD, Currency::CLP, ADReal::new(935.0));

    let bootstrapper =
        MultiCurveBootstrapper::new(curve_specs, policy).with_exchange_rate_store(fx_store);
    let curves = bootstrapper.bootstrap(&quote_store, Level::Mid)?;

    // 4. Display results for each bootstrapped curve
    print_bootstrap_results(rd, &curves)?;

    Ok(())
}
