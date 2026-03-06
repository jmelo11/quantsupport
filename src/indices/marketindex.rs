use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

use crate::indices::rateindex::RateIndexDetails;
use crate::indices::{quotetype::QuoteType, rateindices::sofr::SOFRIndex};
use crate::utils::errors::{QSError, Result};

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, Clone)]
/// # `InterestRateIndex`
pub enum MarketIndex {
    /// SOFR Index.
    SOFR,
    /// SOFR Compounded Index.
    SOFRCompounded,
    /// Term-SOFR 1m Index.
    TermSOFR1m,
    /// Term-SOFR 12m Index.
    TermSOFR3m,
    /// Term-SOFR 12m Index.
    TermSOFR6m,
    /// Term-SOFR 12m Index.
    TermSOFR12m,
    /// Indice camara promedio Index.
    ICP,
    /// VIX Index
    VIX,
    /// Equity index or price. Used to identify volatilities.
    Equity(String),
    /// Other indices.
    Other(String),
}

impl Display for MarketIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SOFR => write!(f, "SOFR"),
            Self::SOFRCompounded => write!(f, "SOFRCompounded"),
            Self::TermSOFR1m => write!(f, "TermSOFR1m"),
            Self::TermSOFR3m => write!(f, "TermSOFR3m"),
            Self::TermSOFR6m => write!(f, "TermSOFR6m"),
            Self::TermSOFR12m => write!(f, "TermSOFR12m"),
            Self::ICP => write!(f, "ICP"),
            Self::VIX => write!(f, "VIX"),
            Self::Equity(name) => write!(f, "{name}"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::str::FromStr for MarketIndex {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let resutls = match s {
            "SOFR" => Self::SOFR,
            "SOFRCompounded" => Self::SOFRCompounded,
            "TermSOFR1m" => Self::TermSOFR1m,
            "TermSOFR3m" => Self::TermSOFR3m,
            "TermSOFR6m" => Self::TermSOFR6m,
            "TermSOFR12m" => Self::TermSOFR12m,
            "ICP" => Self::ICP,
            "VIX" => Self::VIX,
            other => Self::Other(other.to_string()), // this should handle equity, fx
        };
        Ok(resutls)
    }
}

impl Default for MarketIndex {
    fn default() -> Self {
        Self::Other("UNKNOWN".to_string())
    }
}

/// # `MarketIndex`
/// Base trait for indices that contain market values.
pub trait MarketIndexDetails {
    /// Name of the index.
    fn name(&self) -> &'static str;
    /// Type of value that the index contains.
    fn quote_type(&self) -> QuoteType;
}

impl MarketIndex {
    /// Details
    ///
    /// ## Errors
    /// Returns an error if the index does not contain market details.
    pub fn details(&self) -> Result<impl MarketIndexDetails> {
        match self {
            Self::SOFR => Ok(SOFRIndex),
            _ => Err(QSError::InvalidValueErr(
                "Index does not contain market details".into(),
            )),
        }
    }
    /// Rate index details.
    ///
    /// ## Errors
    /// Returns an error if the index is not a rate index.
    pub fn rate_index_details(&self) -> Result<impl RateIndexDetails> {
        match self {
            Self::SOFR => Ok(SOFRIndex),
            _ => Err(QSError::InvalidValueErr(
                "Index is not rate index".into(),
            )),
        }
    }
}
