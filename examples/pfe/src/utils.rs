use quantsupport::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
// ---------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct QuoteRecord {
    identifier: String,
    mid: f64,
}

#[derive(serde::Deserialize)]
struct JsonQuotes {
    reference_date: Date,
    quotes: Vec<QuoteRecord>,
}

#[derive(serde::Deserialize)]
struct JsonCurveSpecs {
    curve_specs: Vec<CurveConfiguration>,
}

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

pub fn load_curve_specs(path: &PathBuf) -> Result<Vec<CurveConfiguration>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    Ok(json.curve_specs)
}

/// Loads historical fixings from a JSON file into a [`FixingStore`].
///
/// Expected format:
/// ```json
/// { "SOFR": [{"date": "2025-05-12", "rate": 0.043}, ...], ... }
/// ```
pub fn load_fixings(
    path: &PathBuf,
) -> std::result::Result<FixingStore, Box<dyn std::error::Error>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: HashMap<String, Vec<FixingRecord>> =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;

    let mut store = FixingStore::default();
    for (index_name, records) in json {
        let market_index = parse_market_index(&index_name)?;
        for rec in records {
            store.add_fixing(&market_index, rec.date, rec.rate);
        }
    }
    Ok(store)
}

#[derive(serde::Deserialize)]
struct FixingRecord {
    date: Date,
    rate: f64,
}

fn parse_market_index(name: &str) -> std::result::Result<MarketIndex, Box<dyn std::error::Error>> {
    match name {
        "SOFR" => Ok(MarketIndex::SOFR),
        "ESTR" => Ok(MarketIndex::ESTR),
        "SONIA" => Ok(MarketIndex::SONIA),
        "ICP" => Ok(MarketIndex::ICP),
        other => Err(format!("Unknown market index: {other}").into()),
    }
}

// ---------------------------------------------------------------------------
// Bootstrapping
// ---------------------------------------------------------------------------

pub fn bootstrap_curves(
    quote_store: &QuoteStore,
    curve_specs: Vec<CurveConfiguration>,
) -> std::result::Result<HashMap<MarketIndex, DiscountCurveElement>, Box<dyn std::error::Error>> {
    let mut all_curves = HashMap::new();

    // Bootstrap each curve independently with its own self-discounting policy,
    // so that e.g. ESTR is bootstrapped with EUR discount, not USD.
    for spec in curve_specs {
        let idx = spec.market_index().clone();
        let ccy = idx
            .rate_index_details()
            .map_err(|e| format!("Cannot resolve currency for {idx}: {e}"))?
            .currency();
        let policy = BootstrapDiscountPolicy::new(idx.clone(), ccy);
        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
        let curves = bootstrapper.bootstrap(quote_store, Level::Mid)?;
        all_curves.extend(curves);
    }

    Ok(all_curves)
}

/// Extract an f64 discount term structure from a bootstrapped `DualFwd` curve.
///
/// Samples discount factors on a fine grid and builds a `DiscountTermStructure<f64>`.
pub fn extract_f64_curve(
    curve_elem: &DiscountCurveElement,
    ref_date: Date,
    max_years: u32,
) -> std::result::Result<DiscountTermStructure<f64>, Box<dyn std::error::Error>> {
    let curve = curve_elem.curve();
    let dc = DayCounter::Actual365;

    // Sample every 3 months up to max_years
    let n_points = (max_years * 4) as usize;
    let mut dates = Vec::with_capacity(n_points + 1);
    let mut dfs = Vec::with_capacity(n_points + 1);

    dates.push(ref_date);
    dfs.push(1.0_f64);

    for i in 1..=n_points {
        let d = ref_date.advance(3 * i as i32, TimeUnit::Months);
        let df = curve.discount_factor(d)?;
        dates.push(d);
        dfs.push(df.value());
    }

    let ts = DiscountTermStructure::<f64>::new(dates, dfs, dc, Interpolator::LogLinear, true)?;
    Ok(ts)
}
