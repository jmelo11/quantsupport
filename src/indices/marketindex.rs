use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

use crate::indices::quotetype::QuoteType;

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, Clone)]
/// # `InterestRateIndex`
pub enum MarketIndex {
    /// SOFR Index.
    SOFR,
    /// SOFR Compounded Index.
    SOFRCompounded,
    /// TermSOFR1m Index.
    TermSOFR1m,
    /// TermSOFR3m Index.
    TermSOFR3m,
    /// TermSOFR6m Index.
    TermSOFR6m,
    /// TermSOFR12m Index.
    TermSOFR12m,
    /// Indice camara promedio Index.
    ICP,
    /// VIX Index
    VIX,
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
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl MarketIndex {
    /// Creates a market index from a string identifier.
    #[must_use]
    pub fn from_str(identifier: &str) -> Self {
        match identifier {
            "SOFR" => Self::SOFR,
            "SOFRCompounded" => Self::SOFRCompounded,
            "TermSOFR1m" => Self::TermSOFR1m,
            "TermSOFR3m" => Self::TermSOFR3m,
            "TermSOFR6m" => Self::TermSOFR6m,
            "TermSOFR12m" => Self::TermSOFR12m,
            "ICP" => Self::ICP,
            "VIX" => Self::VIX,
            other => Self::Other(other.to_string()),
        }
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
