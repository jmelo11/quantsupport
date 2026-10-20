//! Typed worksheet values shared by range readers and portable input validation.

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum Cell {
    Blank,
    Number(f64),
    Text(String),
    Boolean(bool),
}

pub type Table = Vec<Vec<Cell>>;

impl Cell {
    pub fn is_blank(&self) -> bool {
        matches!(self, Self::Blank) || matches!(self, Self::Text(value) if value.trim().is_empty())
    }

    pub fn text(&self, label: &str) -> Result<&str, String> {
        match self {
            Self::Text(value) => Ok(value.trim()),
            _ => Err(format!("{label} must be text")),
        }
    }

    pub fn number(&self, label: &str) -> Result<f64, String> {
        match self {
            Self::Number(value) if value.is_finite() => Ok(*value),
            _ => Err(format!("{label} must be a finite numeric cell")),
        }
    }

    pub fn count(&self, label: &str) -> Result<usize, String> {
        let value = self.number(label)?;
        if value < 0.0 || value.fract() != 0.0 || value > f64::from(i32::MAX) {
            return Err(format!("{label} must be a nonnegative whole number"));
        }
        Ok(value as usize)
    }

    pub fn flag(&self, label: &str) -> Result<bool, String> {
        match self {
            Self::Boolean(value) => Ok(*value),
            Self::Number(0.0) => Ok(false),
            Self::Number(1.0) => Ok(true),
            _ => Err(format!("{label} must be TRUE/FALSE or numeric 0/1")),
        }
    }
}

pub fn label(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn rows<'a>(table: &'a Table, columns: usize, name: &str) -> Result<Vec<&'a [Cell]>, String> {
    if table.iter().flatten().all(Cell::is_blank) {
        return Ok(Vec::new());
    }
    if table.iter().any(|row| row.len() != columns) {
        return Err(format!("{name} must be a {columns}-column worksheet range"));
    }
    Ok(table
        .iter()
        .filter(|row| !row.iter().all(Cell::is_blank))
        .map(Vec::as_slice)
        .collect())
}

/// Canonical worksheet index text, including parameterized native index variants.
pub fn market_index(value: &str) -> Result<quantsupport::prelude::MarketIndex, String> {
    use quantsupport::prelude::{Currency, FxPair, MarketIndex};
    let value = value.trim();
    let currency = |text: &str| {
        text.trim()
            .parse::<Currency>()
            .map_err(|error| error.to_string())
    };
    for (prefix, equity) in [("Equity:", true), ("Credit:", false)] {
        if let Some(name) = value.strip_prefix(prefix) {
            return Ok(if equity {
                MarketIndex::Equity(name.to_string())
            } else {
                MarketIndex::Credit(name.to_string())
            });
        }
    }
    if let Some(pair) = value.strip_prefix("FX:") {
        let (base, quote) = pair
            .split_once('/')
            .ok_or_else(|| format!("invalid FX market index '{value}'"))?;
        return FxPair::new(currency(base)?, currency(quote)?)
            .map(MarketIndex::FxPair)
            .map_err(|error| error.to_string());
    }
    if let Some(pair) = value.strip_prefix("Collateral:") {
        let (base, collateral) = pair
            .split_once('/')
            .ok_or_else(|| format!("invalid collateral market index '{value}'"))?;
        return Ok(MarketIndex::Collateral(
            currency(base)?,
            currency(collateral)?,
        ));
    }
    Ok(value.parse().unwrap_or_else(|never| match never {}))
}
