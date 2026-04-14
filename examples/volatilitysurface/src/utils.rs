use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use quantsupport::prelude::*;
use serde::Deserialize;

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
