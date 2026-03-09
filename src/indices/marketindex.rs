use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

use crate::currencies::currency::Currency;
use crate::indices::rateindex::RateIndexDetails;
use crate::indices::{
    quotetype::QuoteType,
    rateindices::{
        aonia::AONIAIndex, corra::CORRAIndex, estr::ESTRIndex, euribor::EuriborIndex,
        nowa::NOWAIndex, nzonia::NZONIAIndex, saron::SARONIndex, sofr::SOFRIndex,
        sonia::SONIAIndex, swestr::SWESTRIndex, tibor::TIBORIndex, tonar::TONARIndex,
    },
};
use crate::utils::errors::{QSError, Result};

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, Clone)]
/// # `InterestRateIndex`
pub enum MarketIndex {
    // ── USD ──────────────────────────────────────────────
    /// SOFR Index (overnight, USD).
    SOFR,
    /// SOFR Compounded Index.
    SOFRCompounded,
    /// Term-SOFR 1m Index.
    TermSOFR1m,
    /// Term-SOFR 3m Index.
    TermSOFR3m,
    /// Term-SOFR 6m Index.
    TermSOFR6m,
    /// Term-SOFR 12m Index.
    TermSOFR12m,

    // ── EUR ──────────────────────────────────────────────
    /// €STR Index (overnight, EUR).
    ESTR,
    /// EURIBOR 1-month Index (term, EUR).
    EURIBOR1m,
    /// EURIBOR 3-month Index (term, EUR).
    EURIBOR3m,
    /// EURIBOR 6-month Index (term, EUR).
    EURIBOR6m,
    /// EURIBOR 12-month Index (term, EUR).
    EURIBOR12m,

    // ── GBP ──────────────────────────────────────────────
    /// SONIA Index (overnight, GBP).
    SONIA,

    // ── JPY ──────────────────────────────────────────────
    /// TONAR Index (overnight, JPY).
    TONAR,
    /// TIBOR 3-month Index (term, JPY).
    TIBOR3m,
    /// TIBOR 6-month Index (term, JPY).
    TIBOR6m,

    // ── CHF ──────────────────────────────────────────────
    /// SARON Index (overnight, CHF).
    SARON,

    // ── CAD ──────────────────────────────────────────────
    /// CORRA Index (overnight, CAD).
    CORRA,

    // ── AUD ──────────────────────────────────────────────
    /// AONIA Index (overnight, AUD).
    AONIA,

    // ── NZD ──────────────────────────────────────────────
    /// NZONIA Index (overnight, NZD).
    NZONIA,

    // ── NOK ──────────────────────────────────────────────
    /// NOWA Index (overnight, NOK).
    NOWA,

    // ── SEK ──────────────────────────────────────────────
    /// SWESTR Index (overnight, SEK).
    SWESTR,

    // ── Other ────────────────────────────────────────────
    /// Indice camara promedio Index.
    ICP,
    /// VIX Index.
    VIX,
    /// Equity index or price. Used to identify volatilities.
    Equity(String),
    /// Collateral discount curve for cashflows in `ccy` posted under `collateral_ccy`.
    ///
    /// For example `Collateral(CLP, USD)` represents the CLP discount curve
    /// under a USD-denominated CSA.
    Collateral(Currency, Currency),
    /// Other indices. Could represent a particular issuer, a custom index, or any other index not covered by the above.
    Other(String),
}

impl Display for MarketIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            // USD
            Self::SOFR => write!(f, "SOFR"),
            Self::SOFRCompounded => write!(f, "SOFRCompounded"),
            Self::TermSOFR1m => write!(f, "TermSOFR1m"),
            Self::TermSOFR3m => write!(f, "TermSOFR3m"),
            Self::TermSOFR6m => write!(f, "TermSOFR6m"),
            Self::TermSOFR12m => write!(f, "TermSOFR12m"),
            // EUR
            Self::ESTR => write!(f, "ESTR"),
            Self::EURIBOR1m => write!(f, "EURIBOR1m"),
            Self::EURIBOR3m => write!(f, "EURIBOR3m"),
            Self::EURIBOR6m => write!(f, "EURIBOR6m"),
            Self::EURIBOR12m => write!(f, "EURIBOR12m"),
            // GBP
            Self::SONIA => write!(f, "SONIA"),
            // JPY
            Self::TONAR => write!(f, "TONAR"),
            Self::TIBOR3m => write!(f, "TIBOR3m"),
            Self::TIBOR6m => write!(f, "TIBOR6m"),
            // CHF
            Self::SARON => write!(f, "SARON"),
            // CAD
            Self::CORRA => write!(f, "CORRA"),
            // AUD
            Self::AONIA => write!(f, "AONIA"),
            // NZD
            Self::NZONIA => write!(f, "NZONIA"),
            // NOK
            Self::NOWA => write!(f, "NOWA"),
            // SEK
            Self::SWESTR => write!(f, "SWESTR"),
            // Other
            Self::ICP => write!(f, "ICP"),
            Self::VIX => write!(f, "VIX"),
            Self::Equity(name) => write!(f, "{name}"),
            Self::Collateral(ccy, coll) => write!(f, "Collateral({ccy}/{coll})"),
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
            "ESTR" | "€STR" => Self::ESTR,
            "EURIBOR1m" => Self::EURIBOR1m,
            "EURIBOR3m" => Self::EURIBOR3m,
            "EURIBOR6m" => Self::EURIBOR6m,
            "EURIBOR12m" => Self::EURIBOR12m,
            "SONIA" => Self::SONIA,
            "TONAR" => Self::TONAR,
            "TIBOR3m" => Self::TIBOR3m,
            "TIBOR6m" => Self::TIBOR6m,
            "SARON" => Self::SARON,
            "CORRA" => Self::CORRA,
            "AONIA" => Self::AONIA,
            "NZONIA" => Self::NZONIA,
            "NOWA" => Self::NOWA,
            "SWESTR" => Self::SWESTR,
            "ICP" => Self::ICP,
            "VIX" => Self::VIX,
            other if other.starts_with("Collateral(") && other.ends_with(')') => {
                let inner = &other["Collateral(".len()..other.len() - 1];
                if let Some((a, b)) = inner.split_once('/') {
                    if let (Ok(c1), Ok(c2)) =
                        (a.trim().parse::<Currency>(), b.trim().parse::<Currency>())
                    {
                        Self::Collateral(c1, c2)
                    } else {
                        Self::Other(other.to_string())
                    }
                } else {
                    Self::Other(other.to_string())
                }
            }
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
    pub fn details(&self) -> Result<Box<dyn MarketIndexDetails>> {
        match self {
            Self::SOFR
            | Self::SOFRCompounded
            | Self::TermSOFR1m
            | Self::TermSOFR3m
            | Self::TermSOFR6m
            | Self::TermSOFR12m => Ok(Box::new(SOFRIndex)),
            Self::ESTR => Ok(Box::new(ESTRIndex)),
            Self::EURIBOR1m | Self::EURIBOR3m | Self::EURIBOR6m | Self::EURIBOR12m => {
                Ok(Box::new(EuriborIndex))
            }
            Self::SONIA => Ok(Box::new(SONIAIndex)),
            Self::TONAR => Ok(Box::new(TONARIndex)),
            Self::TIBOR3m | Self::TIBOR6m => Ok(Box::new(TIBORIndex)),
            Self::SARON => Ok(Box::new(SARONIndex)),
            Self::CORRA => Ok(Box::new(CORRAIndex)),
            Self::AONIA => Ok(Box::new(AONIAIndex)),
            Self::NZONIA => Ok(Box::new(NZONIAIndex)),
            Self::NOWA => Ok(Box::new(NOWAIndex)),
            Self::SWESTR => Ok(Box::new(SWESTRIndex)),
            _ => Err(QSError::InvalidValueErr(
                "Index does not contain market details".into(),
            )),
        }
    }
    /// Rate index details.
    ///
    /// ## Errors
    /// Returns an error if the index is not a rate index.
    pub fn rate_index_details(&self) -> Result<Box<dyn RateIndexDetails>> {
        match self {
            Self::SOFR
            | Self::SOFRCompounded
            | Self::TermSOFR1m
            | Self::TermSOFR3m
            | Self::TermSOFR6m
            | Self::TermSOFR12m => Ok(Box::new(SOFRIndex)),
            Self::ESTR => Ok(Box::new(ESTRIndex)),
            Self::EURIBOR1m | Self::EURIBOR3m | Self::EURIBOR6m | Self::EURIBOR12m => {
                Ok(Box::new(EuriborIndex))
            }
            Self::SONIA => Ok(Box::new(SONIAIndex)),
            Self::TONAR => Ok(Box::new(TONARIndex)),
            Self::TIBOR3m | Self::TIBOR6m => Ok(Box::new(TIBORIndex)),
            Self::SARON => Ok(Box::new(SARONIndex)),
            Self::CORRA => Ok(Box::new(CORRAIndex)),
            Self::AONIA => Ok(Box::new(AONIAIndex)),
            Self::NZONIA => Ok(Box::new(NZONIAIndex)),
            Self::NOWA => Ok(Box::new(NOWAIndex)),
            Self::SWESTR => Ok(Box::new(SWESTRIndex)),
            _ => Err(QSError::InvalidValueErr("Index is not rate index".into())),
        }
    }
}
