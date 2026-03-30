use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use quantsupport::prelude::*;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// JSON helpers for loading market data
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

// ---------------------------------------------------------------------------
// Bootstrapping and context creation
// ---------------------------------------------------------------------------

pub struct CurveEnvironment {
    pub context: ContextManager,
    pub curve_lookup: HashMap<MarketIndex, DiscountCurveElement>,
}

pub fn build_curves(
    quote_store: &QuoteStore,
    curve_specs: Vec<CurveConfiguration>,
) -> std::result::Result<CurveEnvironment, Box<dyn std::error::Error>> {
    let rd = quote_store.reference_date();

    let csa_index = MarketIndex::SOFR;
    let csa_currency = Currency::USD;
    let policy = BootstrapDiscountPolicy::new(csa_index.clone(), csa_currency);

    // FX spot: 1 USD = 935 CLP
    let mut fx_store = ExchangeRateStore::new();
    fx_store.add_exchange_rate(Currency::USD, Currency::CLP, DualFwd::new(935.0));

    let bootstrapper =
        MultiCurveBootstrapper::new(curve_specs, policy).with_exchange_rate_store(fx_store);
    let curves = bootstrapper.bootstrap(quote_store, Level::Mid)?;

    // Keep a lookup of curves for DF extraction in cashflow details
    let curve_lookup: HashMap<MarketIndex, DiscountCurveElement> = curves.clone();

    // Set up the pricing context
    let mut constructed_elements = ConstructedElementStore::default();
    for (index, elem) in curves {
        constructed_elements
            .discount_curves_mut()
            .insert(index, elem);
    }

    let fixing_store = FixingStore::default();
    let mut pricing_fx_store = ExchangeRateStore::new();
    pricing_fx_store.add_exchange_rate(Currency::USD, Currency::CLP, DualFwd::new(935.0));

    let context = ContextManager::new(QuoteStore::new(rd), fixing_store)
        .with_base_currency(Currency::USD)
        .with_constructed_elements(constructed_elements)
        .with_exchange_rate_store(pricing_fx_store);

    Ok(CurveEnvironment {
        context,
        curve_lookup,
    })
}
