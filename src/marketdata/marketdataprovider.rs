use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufRead, BufReader, Read},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    indices::marketindex::MarketIndex,
    marketdata::volatility::{VolatilityCube, VolatilitySurface},
    time::date::Date,
    time::period::Period,
    utils::errors::{AtlasError, Result},
};

/// # QuoteLevels
/// Quote levels associated with an instrument identifier.
///
/// When multiple levels are provided the `mid` is preferred, otherwise `bid/ask`
/// are used to compute a fallback representative value.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QuoteLevels {
    /// Mid price/level.
    #[serde(default)]
    mid: Option<f64>,
    /// Bid price/level.
    #[serde(default)]
    bid: Option<f64>,
    /// Ask price/level.
    #[serde(default)]
    ask: Option<f64>,
}

impl QuoteLevels {
    /// Returns the mid quote if available.
    #[must_use]
    pub const fn mid(&self) -> Option<f64> {
        self.mid
    }

    /// Returns the bid quote if available.
    #[must_use]
    pub const fn bid(&self) -> Option<f64> {
        self.bid
    }

    /// Returns the ask quote if available.
    #[must_use]
    pub const fn ask(&self) -> Option<f64> {
        self.ask
    }

    /// Resolves a representative quote value.
    ///
    /// # Errors
    /// Returns an error if none of mid, bid, or ask are available.
    pub fn value(&self) -> Result<f64> {
        if let Some(mid) = self.mid {
            return Ok(mid);
        }
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => Ok((bid + ask) * 0.5),
            (Some(bid), None) => Ok(bid),
            (None, Some(ask)) => Ok(ask),
            (None, None) => Err(AtlasError::ValueNotSetErr(
                "quote level missing mid/bid/ask".to_string(),
            )),
        }
    }
}

/// # QuoteRecord
/// Quote record compatible with serde deserialization.
///
/// This supports JSON rows of the form:
/// `{ "instrument": "USD-SWAP|maturity=2026-01-01|strike=0.02", "mid": 0.15 }`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteRecord {
    /// Instrument identifier containing embedded metadata.
    instrument: String,
    /// Quote levels for the instrument.
    #[serde(flatten)]
    levels: QuoteLevels,
}

/// # Quote
/// Quote structure stored by the market data provider.
#[derive(Clone, Debug)]
pub struct Quote {
    identifier: String,
    levels: QuoteLevels,
}

impl Quote {
    /// Returns the original identifier string.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the quote levels.
    #[must_use]
    pub const fn levels(&self) -> &QuoteLevels {
        &self.levels
    }
}

impl From<QuoteRecord> for Quote {
    fn from(value: QuoteRecord) -> Self {
        Self {
            identifier: value.instrument,
            levels: value.levels,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum QuoteSource {
    Map(BTreeMap<String, QuoteLevels>),
    List(Vec<QuoteRecord>),
}

impl QuoteSource {
    fn into_quotes(self) -> Vec<Quote> {
        match self {
            Self::Map(map) => map
                .into_iter()
                .map(|(identifier, levels)| Quote { identifier, levels })
                .collect(),
            Self::List(list) => list.into_iter().map(Quote::from).collect(),
        }
    }
}

/// Parsed quote identifier details.
///
/// Identifiers are parsed using the `INSTRUMENT|key=value|...` format. Keys are
/// case-insensitive and stored in the `attributes` map using lowercase keys.
/// Standard fields are `strike`, `shift`, `maturity` (aliases: `expiry`, `exp`),
/// and `tenor` (parsed as a [`Period`] with formats like `1Y` or `6M`).
#[derive(Clone, Debug, Default)]
pub struct QuoteDetails {
    instrument: MarketIndex,
    strike: Option<f64>,
    shift: Option<f64>,
    maturity: Option<Date>,
    tenor: Option<Period>,
    attributes: BTreeMap<String, String>,
}

impl QuoteDetails {
    /// Parses an instrument identifier of the form `INSTRUMENT|key=value|...`.
    ///
    /// # Errors
    /// Returns an error if the identifier is malformed.
    pub fn parse(identifier: &str) -> Result<Self> {
        let mut parts = identifier.split('|');
        let instrument = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AtlasError::InvalidValueErr("Quote identifier missing instrument".to_string())
            })?
            .to_string();

        let mut details = Self {
            instrument: MarketIndex::from_str(&instrument),
            ..Self::default()
        };

        for part in parts {
            let mut kv = part.splitn(2, '=');
            let key = kv.next().map(str::trim).unwrap_or_default();
            let value = kv.next().map(str::trim).unwrap_or_default();
            if key.is_empty() || value.is_empty() {
                return Err(AtlasError::InvalidValueErr(format!(
                    "Invalid instrument attribute segment '{part}'"
                )));
            }
            let key_lower = key.to_lowercase();
            match key_lower.as_str() {
                "strike" => {
                    details.strike = Some(parse_f64(value, "strike")?);
                }
                "shift" => {
                    details.shift = Some(parse_f64(value, "shift")?);
                }
                "maturity" | "expiry" | "exp" => {
                    details.maturity = Some(parse_date(value)?);
                }
                "tenor" => {
                    details.tenor = Some(parse_tenor(value)?);
                }
                _ => {}
            }
            details.attributes.insert(key_lower, value.to_string());
        }

        Ok(details)
    }

    /// Returns the instrument base identifier.
    #[must_use]
    pub fn instrument(&self) -> &MarketIndex {
        &self.instrument
    }

    /// Returns the strike, if present.
    #[must_use]
    pub const fn strike(&self) -> Option<f64> {
        self.strike
    }

    /// Returns the shift, if present.
    #[must_use]
    pub const fn shift(&self) -> Option<f64> {
        self.shift
    }

    /// Returns the maturity, if present.
    #[must_use]
    pub const fn maturity(&self) -> Option<Date> {
        self.maturity
    }

    /// Returns the tenor, if present.
    #[must_use]
    pub const fn tenor(&self) -> Option<Period> {
        self.tenor
    }

    /// Returns the additional attributes parsed from the identifier.
    #[must_use]
    pub const fn attributes(&self) -> &BTreeMap<String, String> {
        &self.attributes
    }

    /// Returns a specific attribute by key.
    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<&String> {
        self.attributes.get(&key.to_lowercase())
    }
}

/// # ExpandedQuote
/// Expanded quote that includes parsed instrument metadata.
#[derive(Clone, Debug)]
pub struct ExpandedQuote {
    identifier: String,
    details: QuoteDetails,
    levels: QuoteLevels,
}

impl ExpandedQuote {
    /// Expands a quote by parsing the identifier for metadata.
    ///
    /// # Errors
    /// Returns an error if the identifier cannot be parsed.
    pub fn try_from_quote(quote: &Quote) -> Result<Self> {
        let details = QuoteDetails::parse(&quote.identifier)?;
        Ok(Self {
            identifier: quote.identifier.clone(),
            details,
            levels: quote.levels.clone(),
        })
    }

    /// Returns the original identifier string.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the parsed quote details.
    #[must_use]
    pub const fn details(&self) -> &QuoteDetails {
        &self.details
    }

    /// Returns the quote levels.
    #[must_use]
    pub const fn levels(&self) -> &QuoteLevels {
        &self.levels
    }
}

/// # MarketDataProvider
/// Provider of market data loaded from serialized quotes.
///
/// JSON payloads may be provided as an array of quote records:
/// `[{ "instrument": "SPX|maturity=2026-01-01|strike=4500", "mid": 0.22 }]`
/// or as a map from identifier to quote levels:
/// `{ "SPX|maturity=2026-01-01|strike=4500": { "mid": 0.22 } }`.
///
/// Tenors are parsed into [`Period`] values, so use strings like `tenor=1Y` or
/// `tenor=6M` in the identifier metadata.
#[derive(Clone, Debug)]
pub struct MarketDataProvider {
    reference_date: Date,
    quotes: Vec<Quote>,
    expanded_quotes: Vec<ExpandedQuote>,
    vol_surfaces: HashMap<MarketIndex, VolatilitySurface>,
    vol_cubes: HashMap<MarketIndex, VolatilityCube>,
}

impl MarketDataProvider {
    /// Creates an empty market data provider.
    #[must_use]
    pub fn new(reference_date: Date) -> Self {
        Self {
            reference_date,
            quotes: Vec::new(),
            expanded_quotes: Vec::new(),
            vol_surfaces: HashMap::new(),
            vol_cubes: HashMap::new(),
        }
    }

    /// Creates a market data provider from a set of quotes.
    ///
    /// # Errors
    /// Returns an error if any quote identifiers are invalid.
    pub fn from_quotes(reference_date: Date, quotes: Vec<Quote>) -> Result<Self> {
        let expanded_quotes = quotes
            .iter()
            .map(ExpandedQuote::try_from_quote)
            .collect::<Result<Vec<_>>>()?;
        let (vol_surfaces, vol_cubes) = build_volatility_structures(&expanded_quotes)?;
        Ok(Self {
            reference_date,
            quotes,
            expanded_quotes,
            vol_surfaces,
            vol_cubes,
        })
    }

    /// Loads market data from a JSON string.
    ///
    /// # Errors
    /// Returns an error if deserialization fails or quotes are invalid.
    pub fn from_json_str(reference_date: Date, payload: &str) -> Result<Self> {
        let source: QuoteSource = serde_json::from_str(payload)
            .map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
        Self::from_quotes(reference_date, source.into_quotes())
    }

    /// Loads market data from a JSON reader.
    ///
    /// # Errors
    /// Returns an error if deserialization fails or quotes are invalid.
    pub fn from_json_reader<R: Read>(reference_date: Date, reader: R) -> Result<Self> {
        let source: QuoteSource = serde_json::from_reader(reader)
            .map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
        Self::from_quotes(reference_date, source.into_quotes())
    }

    /// Loads market data from a JSON file path.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or quotes are invalid.
    pub fn from_json_path(reference_date: Date, path: &str) -> Result<Self> {
        let file =
            File::open(path).map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
        let reader = BufReader::new(file);
        Self::from_json_reader(reference_date, reader)
    }

    /// Loads market data from a CSV reader.
    ///
    /// # Errors
    /// Returns an error if deserialization fails or quotes are invalid.
    pub fn from_csv_reader<R: Read>(reference_date: Date, reader: R) -> Result<Self> {
        let mut reader = BufReader::new(reader);
        let mut header_line = String::new();
        reader
            .read_line(&mut header_line)
            .map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
        if header_line.trim().is_empty() {
            return Err(AtlasError::DeserializationErr(
                "CSV header is missing".to_string(),
            ));
        }
        let headers: Vec<String> = header_line
            .trim_end()
            .split(',')
            .map(|h| h.trim().to_lowercase())
            .collect();
        let mut quotes = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let values: Vec<&str> = line.split(',').collect();
            let record = quote_record_from_csv(&headers, &values)?;
            quotes.push(Quote::from(record));
        }
        Self::from_quotes(reference_date, quotes)
    }

    /// Loads market data from a CSV file path.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or quotes are invalid.
    pub fn from_csv_path(reference_date: Date, path: &str) -> Result<Self> {
        let file =
            File::open(path).map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
        let reader = BufReader::new(file);
        Self::from_csv_reader(reference_date, reader)
    }

    /// Returns the reference date for the provider.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Returns the raw quotes.
    #[must_use]
    pub fn quotes(&self) -> &[Quote] {
        &self.quotes
    }

    /// Returns the expanded quotes with parsed metadata.
    #[must_use]
    pub fn expanded_quotes(&self) -> &[ExpandedQuote] {
        &self.expanded_quotes
    }

    /// Returns a volatility surface by instrument identifier.
    #[must_use]
    pub fn volatility_surface(&self, instrument: &MarketIndex) -> Option<&VolatilitySurface> {
        self.vol_surfaces.get(instrument)
    }

    /// Returns a volatility cube by instrument identifier.
    #[must_use]
    pub fn volatility_cube(&self, instrument: &MarketIndex) -> Option<&VolatilityCube> {
        self.vol_cubes.get(instrument)
    }
}

fn build_volatility_structures(
    quotes: &[ExpandedQuote],
) -> Result<(
    HashMap<MarketIndex, VolatilitySurface>,
    HashMap<MarketIndex, VolatilityCube>,
)> {
    let mut surfaces = HashMap::new();
    let mut cubes = HashMap::new();

    for quote in quotes {
        let details = quote.details();
        let (Some(maturity), Some(strike)) = (details.maturity(), details.strike()) else {
            continue;
        };
        let value = quote.levels().value()?;
        if let Some(tenor) = details.tenor() {
            let entry = cubes
                .entry(details.instrument().clone())
                .or_insert_with(|| VolatilityCube::new(details.instrument().clone()));
            entry.insert_point(maturity, tenor, strike, value);
        } else {
            let entry = surfaces
                .entry(details.instrument().clone())
                .or_insert_with(|| VolatilitySurface::new(details.instrument().clone()));
            entry.insert_point(maturity, strike, value);
        }
    }

    Ok((surfaces, cubes))
}

fn parse_f64(value: &str, label: &str) -> Result<f64> {
    value.parse::<f64>().map_err(|_| {
        AtlasError::InvalidValueErr(format!(
            "Invalid {label} value in quote identifier: {value}"
        ))
    })
}

fn parse_date(value: &str) -> Result<Date> {
    Date::from_str(value, "%Y-%m-%d").or_else(|_| Date::from_str(value, "%Y%m%d"))
}

fn parse_tenor(value: &str) -> Result<Period> {
    Period::from_str(value)
}

fn quote_record_from_csv(headers: &[String], values: &[&str]) -> Result<QuoteRecord> {
    let mut instrument = None;
    let mut mid = None;
    let mut bid = None;
    let mut ask = None;

    for (idx, header) in headers.iter().enumerate() {
        let value = values.get(idx).map(|v| v.trim()).unwrap_or("");
        if value.is_empty() {
            continue;
        }
        match header.as_str() {
            "instrument" | "identifier" => {
                instrument = Some(value.to_string());
            }
            "mid" => {
                mid = Some(parse_f64(value, "mid")?);
            }
            "bid" => {
                bid = Some(parse_f64(value, "bid")?);
            }
            "ask" => {
                ask = Some(parse_f64(value, "ask")?);
            }
            _ => {}
        }
    }

    let instrument = instrument.ok_or_else(|| {
        AtlasError::DeserializationErr("CSV record missing instrument column".to_string())
    })?;

    let mut map = serde_json::Map::new();
    map.insert("instrument".to_string(), Value::String(instrument));
    if let Some(mid) = mid {
        map.insert(
            "mid".to_string(),
            Value::Number(
                serde_json::Number::from_f64(mid)
                    .ok_or_else(|| AtlasError::InvalidValueErr("Invalid mid value".to_string()))?,
            ),
        );
    }
    if let Some(bid) = bid {
        map.insert(
            "bid".to_string(),
            Value::Number(
                serde_json::Number::from_f64(bid)
                    .ok_or_else(|| AtlasError::InvalidValueErr("Invalid bid value".to_string()))?,
            ),
        );
    }
    if let Some(ask) = ask {
        map.insert(
            "ask".to_string(),
            Value::Number(
                serde_json::Number::from_f64(ask)
                    .ok_or_else(|| AtlasError::InvalidValueErr("Invalid ask value".to_string()))?,
            ),
        );
    }

    serde_json::from_value(Value::Object(map))
        .map_err(|err| AtlasError::DeserializationErr(err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn parse_quote_details_extracts_fields() {
        let identifier = "SPX|maturity=2026-01-01|strike=4500|shift=0.01|tenor=1Y";
        let details = QuoteDetails::parse(identifier).expect("valid identifier");

        assert_eq!(details.instrument(), &MarketIndex::from_str("SPX"));
        assert_eq!(details.strike(), Some(4500.0));
        assert_eq!(details.shift(), Some(0.01));
        assert_eq!(
            details.tenor(),
            Some(Period::new(1, crate::time::enums::TimeUnit::Years))
        );
        assert_eq!(details.maturity(), Some(Date::new(2026, 1, 1)));
    }

    #[test]
    fn json_map_builds_vol_surface_and_cube() {
        let json = r#"
        {
            "SPX|maturity=2026-01-01|strike=4500": { "mid": 0.22 },
            "SPX|maturity=2026-01-01|strike=4500|tenor=1Y": { "mid": 0.18 }
        }
        "#;
        let provider =
            MarketDataProvider::from_json_str(Date::new(2024, 1, 1), json).expect("valid provider");

        let spx = MarketIndex::from_str("SPX");
        let surface = provider.volatility_surface(&spx).expect("surface exists");
        assert_eq!(
            surface.volatility(Date::new(2026, 1, 1), 4500.0).unwrap(),
            0.22
        );

        let cube = provider.volatility_cube(&spx).expect("cube exists");
        assert_eq!(
            cube.volatility(
                Date::new(2026, 1, 1),
                Period::new(1, crate::time::enums::TimeUnit::Years),
                4500.0
            )
            .unwrap(),
            0.18
        );
    }

    #[test]
    fn csv_reader_loads_quotes() {
        let csv = "instrument,mid,bid,ask\nSPX|maturity=2026-01-01|strike=4500,0.22,,\n";
        let provider = MarketDataProvider::from_csv_reader(Date::new(2024, 1, 1), Cursor::new(csv))
            .expect("valid provider");
        assert_eq!(provider.quotes().len(), 1);
        let quote = &provider.quotes()[0];
        assert_eq!(quote.identifier(), "SPX|maturity=2026-01-01|strike=4500");
        assert_eq!(quote.levels().mid(), Some(0.22));
    }
}
