use std::{fs::File, io::BufReader, path::PathBuf, str::FromStr};

use quantsupport::prelude::*;
use serde::Deserialize;

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
    curve_specs: Vec<CurveConfiguration>,
}

/// Utility functions for constructing example trades.
pub fn create_swap() -> SwapTrade<DualFwd> {
    let start_date = Date::new(2024, 1, 15);
    let maturity_date = Date::new(2029, 1, 15);
    let notional = 10_000_000.0;
    let fixed_rate = 0.030;

    let rate_definition = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Simple,
        Frequency::Semiannual,
    );

    let swap = MakeSwap::<DualFwd>::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(start_date)
        .with_maturity_date(maturity_date)
        .with_fixed_rate(fixed_rate)
        .with_notional(notional)
        .with_rate_definition(rate_definition)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive) // receive fixed, pay floating
        .with_fixed_leg_frequency(Frequency::Semiannual)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()
        .expect("Failed to build swap");

    SwapTrade::new(swap, start_date, notional, Side::LongReceive)
}

/// Loads quotes from a JSON file into a [`QuoteStore`].
pub fn load_quotes(path: &PathBuf) -> Result<QuoteStore> {
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
pub fn load_curve_specs(path: &PathBuf) -> Result<Vec<CurveConfiguration>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    Ok(json.curve_specs)
}
