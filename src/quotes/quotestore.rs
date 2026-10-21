use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    quotes::{
        quote::{Quote, QuoteDetails, QuoteLevels},
        quoteselector::QuoteSelector,
    },
    time::date::Date,
    utils::errors::QSError,
};

/// Serde-friendly single quote record, e.g. `{"identifier": "OIS_USD_SOFR_1Y", "mid": 0.041}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRecord {
    /// Quote identifier parseable into [`QuoteDetails`].
    pub identifier: String,
    /// Mid level.
    #[serde(default)]
    pub mid: Option<f64>,
    /// Bid level.
    #[serde(default)]
    pub bid: Option<f64>,
    /// Ask level.
    #[serde(default)]
    pub ask: Option<f64>,
}

/// Serde-friendly quote store input:
/// `{"reference_date": "2025-11-11", "quotes": [{"identifier": ..., "mid": ...}, ...]}`.
///
/// Deserializable from JSON files or Python dicts, and convertible into a
/// [`QuoteStore`] via [`TryFrom`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteStoreRecords {
    /// Reference date of the quotes.
    pub reference_date: Date,
    /// The quote records.
    pub quotes: Vec<QuoteRecord>,
}

impl TryFrom<QuoteStoreRecords> for QuoteStore {
    type Error = QSError;

    fn try_from(records: QuoteStoreRecords) -> Result<Self, Self::Error> {
        let mut store = Self::new(records.reference_date);
        for rec in records.quotes {
            let details = QuoteDetails::from_str(&rec.identifier)?;
            let levels = QuoteLevels::new(rec.mid, rec.bid, rec.ask);
            store.add_quote(Quote::new(details, levels));
        }
        Ok(store)
    }
}

/// Provider of market data loaded from quotes.
#[derive(Debug, Clone, Serialize, Default)]
pub struct QuoteStore {
    reference_date: Date,
    quotes: HashMap<String, Quote>,
    #[serde(skip)]
    identifiers: HashMap<String, String>,
}

impl<'de> Deserialize<'de> for QuoteStore {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct StoredQuotes {
            reference_date: Date,
            quotes: HashMap<String, Quote>,
        }
        let stored = StoredQuotes::deserialize(deserializer)?;
        let mut result = Self::new(stored.reference_date);
        for quote in stored.quotes.into_values() {
            let identifier = quote.details().identifier().to_ascii_lowercase();
            if result.identifiers.contains_key(&identifier) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate quote identifier (case insensitive): {identifier}"
                )));
            }
            result.add_quote(quote);
        }
        Ok(result)
    }
}

impl QuoteStore {
    /// Creates an empty market data provider.
    #[must_use]
    pub fn new(reference_date: Date) -> Self {
        Self {
            reference_date,
            quotes: HashMap::new(),
            identifiers: HashMap::new(),
        }
    }
    /// Returns the reference date for the provider.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Adds a market quote to the provider, indexed by its identifier.
    pub fn add_quote(&mut self, quote: Quote) {
        let id = quote.details().identifier();
        if let Some(previous) = self.identifiers.insert(id.to_ascii_lowercase(), id.clone()) {
            self.quotes.remove(&previous);
        }
        self.quotes.insert(id, quote);
    }

    /// Returns a quote by identifier.
    #[must_use]
    pub fn quote(&self, identifier: &str) -> Option<&Quote> {
        self.identifiers
            .get(&identifier.to_ascii_lowercase())
            .and_then(|key| self.quotes.get(key))
    }

    /// Removes a quote using a case-insensitive identifier.
    pub fn remove_quote(&mut self, identifier: &str) -> Option<Quote> {
        self.identifiers
            .remove(&identifier.to_ascii_lowercase())
            .and_then(|key| self.quotes.remove(&key))
    }

    /// Returns all stored quotes.
    #[must_use]
    pub const fn quotes(&self) -> &HashMap<String, Quote> {
        &self.quotes
    }
}

impl QuoteSelector for QuoteStore {
    fn select(&self, identifier: &str) -> Option<Quote> {
        self.quote(identifier).cloned()
    }

    fn reference_date(&self) -> Date {
        self.reference_date
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(identifier: &str, mid: f64) -> Quote {
        Quote::new(identifier.parse().unwrap(), QuoteLevels::with_mid(mid))
    }

    #[test]
    fn lookup_selection_and_replacement_ignore_identifier_case() {
        let mut store = QuoteStore::new(Date::new(2026, 1, 15));
        store.add_quote(quote("Swap_USD_SOFR_5Y_Annual", 0.03));
        assert_eq!(
            store
                .quote("sWaP_usd_sofr_5y_annual")
                .unwrap()
                .levels()
                .mid(),
            Some(0.03)
        );
        assert!(store.select("SWAP_USD_SOFR_5Y_ANNUAL").is_some());
        store.add_quote(quote("swap_usd_sofr_5y_annual", 0.04));
        assert_eq!(store.quotes().len(), 1);
        assert_eq!(
            store
                .quote("Swap_USD_SOFR_5Y_Annual")
                .unwrap()
                .levels()
                .mid(),
            Some(0.04)
        );
        assert_eq!(
            store
                .quote("SWAP_USD_SOFR_5Y_ANNUAL")
                .unwrap()
                .details()
                .identifier(),
            "swap_usd_sofr_5y_annual"
        );
        assert!(store.remove_quote("SWAP_USD_SOFR_5Y_ANNUAL").is_some());
        assert!(store.quotes().is_empty());
    }

    #[test]
    fn native_serde_rebuilds_the_lookup_index_and_preserves_display_names() {
        let mut store = QuoteStore::new(Date::new(2026, 1, 15));
        store.add_quote(quote(
            "FixedRateBond_USD_SOFR_5Y_Annual_0.04_AnchorPrice",
            99.0,
        ));
        let encoded = serde_json::to_value(&store).unwrap();
        assert!(encoded.get("identifiers").is_none());
        let decoded: QuoteStore = serde_json::from_value(encoded).unwrap();
        assert!(decoded
            .select("fixedratebond_usd_sofr_5y_annual_0.04_anchorprice")
            .is_some());
        assert!(decoded
            .quotes()
            .contains_key("FixedRateBond_USD_SOFR_5Y_Annual_0.04_AnchorPrice"));
    }

    #[test]
    fn record_inputs_replace_case_variants_and_native_maps_reject_duplicates() {
        let store = QuoteStore::try_from(QuoteStoreRecords {
            reference_date: Date::new(2026, 1, 15),
            quotes: vec![
                QuoteRecord {
                    identifier: "OIS_USD_SOFR_1Y".into(),
                    mid: Some(0.03),
                    bid: None,
                    ask: None,
                },
                QuoteRecord {
                    identifier: "ois_usd_sofr_1y".into(),
                    mid: Some(0.04),
                    bid: None,
                    ask: None,
                },
            ],
        })
        .unwrap();
        assert_eq!(store.quotes().len(), 1);
        assert_eq!(
            store.quote("OIS_USD_SOFR_1Y").unwrap().levels().mid(),
            Some(0.04)
        );
        let mut encoded = serde_json::to_value(&store).unwrap();
        encoded["quotes"]["OIS_USD_SOFR_1Y"] =
            serde_json::to_value(quote("OIS_USD_SOFR_1Y", 0.05)).unwrap();
        assert!(serde_json::from_value::<QuoteStore>(encoded).is_err());
    }
}
