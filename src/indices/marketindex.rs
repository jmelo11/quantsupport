use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

use crate::currencies::currency::Currency;
use crate::indices::fxpair::FxPair;
use crate::indices::rateindex::RateIndexDetails;
use crate::indices::{
    quotetype::QuoteType,
    rateindices::{
        aonia::AONIAIndex,
        bbsw::{BBSW1mIndex, BBSW2mIndex, BBSW3mIndex, BBSW4mIndex, BBSW6mIndex},
        bkbm::{BKBM1mIndex, BKBM2mIndex, BKBM3mIndex, BKBM4mIndex, BKBM6mIndex},
        cdi::CDIIndex,
        corra::CORRAIndex,
        destr::DESTRIndex,
        estr::ESTRIndex,
        euribor::Euribor1wIndex,
        euribor::{Euribor12mIndex, Euribor1mIndex, Euribor3mIndex, Euribor6mIndex},
        fedfunds::FedFundsIndex,
        icp::ICPIndex,
        kofr::KOFRIndex,
        nibor::{NIBOR12mIndex, NIBOR1mIndex, NIBOR3mIndex, NIBOR6mIndex},
        nowa::NOWAIndex,
        nzonia::NZONIAIndex,
        saron::SARONIndex,
        shibor::{
            SHIBOR12mIndex, SHIBOR1dIndex, SHIBOR1mIndex, SHIBOR1wIndex, SHIBOR2wIndex,
            SHIBOR3mIndex, SHIBOR6mIndex,
        },
        sofr::{
            SOFRCompoundedIndex, SOFRIndex, TermSOFR12mIndex, TermSOFR1mIndex, TermSOFR3mIndex,
            TermSOFR6mIndex,
        },
        sonia::SONIAIndex,
        swestr::SWESTRIndex,
        tibor::{Tibor3mIndex, Tibor6mIndex},
        tonar::TONARIndex,
        zaronia::ZARONIAIndex,
    },
};
use crate::utils::errors::{QSError, Result};

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, Clone)]
/// Identifier for an observable market index or underlying.
pub enum MarketIndex {
    /// `FedFunds` rate index.
    FedFunds,
    /// `CDI` rate index.
    CDI,
    /// `DESTR` rate index.
    DESTR,
    /// `KOFR` rate index.
    KOFR,
    /// `ZARONIA` rate index.
    ZARONIA,
    /// `BBSW1m` rate index.
    BBSW1m,
    /// `BBSW2m` rate index.
    BBSW2m,
    /// `BBSW3m` rate index.
    BBSW3m,
    /// `BBSW4m` rate index.
    BBSW4m,
    /// `BBSW6m` rate index.
    BBSW6m,
    /// `BKBM1m` rate index.
    BKBM1m,
    /// `BKBM2m` rate index.
    BKBM2m,
    /// `BKBM3m` rate index.
    BKBM3m,
    /// `BKBM4m` rate index.
    BKBM4m,
    /// `BKBM6m` rate index.
    BKBM6m,
    /// `NIBOR1m` rate index.
    NIBOR1m,
    /// `NIBOR3m` rate index.
    NIBOR3m,
    /// `NIBOR6m` rate index.
    NIBOR6m,
    /// `NIBOR12m` rate index.
    NIBOR12m,
    /// `SHIBOR1d` rate index.
    SHIBOR1d,
    /// `SHIBOR1w` rate index.
    SHIBOR1w,
    /// `SHIBOR2w` rate index.
    SHIBOR2w,
    /// `SHIBOR1m` rate index.
    SHIBOR1m,
    /// `SHIBOR3m` rate index.
    SHIBOR3m,
    /// `SHIBOR6m` rate index.
    SHIBOR6m,
    /// `SHIBOR12m` rate index.
    SHIBOR12m,
    /// `EURIBOR1w` rate index.
    EURIBOR1w,

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
    /// FX spot pair. Encodes a directed foreign-exchange pair.
    ///
    /// For example `FxPair(FxPair::new(EUR, USD))` represents the EUR/USD spot.
    /// Its string form is `FxPair(EURUSD)`.
    FxPair(FxPair),
    /// Collateral discount curve for cashflows in `ccy` posted under `collateral_ccy`.
    ///
    /// For example `Collateral(CLP, USD)` represents the CLP discount curve
    /// under a USD-denominated CSA.
    Collateral(Currency, Currency),
    /// Credit (survival/hazard) curve of a reference entity or counterparty.
    ///
    /// For example `Credit("CORPORATE-A")` keys the survival curve bootstrapped from
    /// CORPORATE-A CDS quotes. Survival probabilities are exposed through the
    /// standard term-structure interface (a survival probability plays the
    /// same role as a discount factor).
    Credit(String),
    /// Other indices. Could represent a particular issuer, a custom index, or any other index not covered by the above.
    Other(String),
}

impl Display for MarketIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::FedFunds => write!(f, "FedFunds"),
            Self::CDI => write!(f, "CDI"),
            Self::DESTR => write!(f, "DESTR"),
            Self::KOFR => write!(f, "KOFR"),
            Self::ZARONIA => write!(f, "ZARONIA"),
            Self::BBSW1m => write!(f, "BBSW1m"),
            Self::BBSW2m => write!(f, "BBSW2m"),
            Self::BBSW3m => write!(f, "BBSW3m"),
            Self::BBSW4m => write!(f, "BBSW4m"),
            Self::BBSW6m => write!(f, "BBSW6m"),
            Self::BKBM1m => write!(f, "BKBM1m"),
            Self::BKBM2m => write!(f, "BKBM2m"),
            Self::BKBM3m => write!(f, "BKBM3m"),
            Self::BKBM4m => write!(f, "BKBM4m"),
            Self::BKBM6m => write!(f, "BKBM6m"),
            Self::NIBOR1m => write!(f, "NIBOR1m"),
            Self::NIBOR3m => write!(f, "NIBOR3m"),
            Self::NIBOR6m => write!(f, "NIBOR6m"),
            Self::NIBOR12m => write!(f, "NIBOR12m"),
            Self::SHIBOR1d => write!(f, "SHIBOR1d"),
            Self::SHIBOR1w => write!(f, "SHIBOR1w"),
            Self::SHIBOR2w => write!(f, "SHIBOR2w"),
            Self::SHIBOR1m => write!(f, "SHIBOR1m"),
            Self::SHIBOR3m => write!(f, "SHIBOR3m"),
            Self::SHIBOR6m => write!(f, "SHIBOR6m"),
            Self::SHIBOR12m => write!(f, "SHIBOR12m"),
            Self::EURIBOR1w => write!(f, "EURIBOR1w"),

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
            Self::Equity(name) => write!(f, "EQ:{name}"),
            Self::FxPair(pair) => write!(f, "FxPair({pair})"),
            Self::Collateral(ccy, coll) => write!(f, "COLL:{ccy}/{coll}"),
            Self::Credit(entity) => write!(f, "CREDIT:{entity}"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::str::FromStr for MarketIndex {
    type Err = std::convert::Infallible;

    /// Parses built-in names case insensitively, preserving custom names and payloads.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let result = match s.to_ascii_uppercase().as_str() {
            "FEDFUNDS" => Self::FedFunds,
            "CDI" => Self::CDI,
            "DESTR" => Self::DESTR,
            "KOFR" => Self::KOFR,
            "ZARONIA" => Self::ZARONIA,
            "BBSW1M" => Self::BBSW1m,
            "BBSW2M" => Self::BBSW2m,
            "BBSW3M" => Self::BBSW3m,
            "BBSW4M" => Self::BBSW4m,
            "BBSW6M" => Self::BBSW6m,
            "BKBM1M" => Self::BKBM1m,
            "BKBM2M" => Self::BKBM2m,
            "BKBM3M" => Self::BKBM3m,
            "BKBM4M" => Self::BKBM4m,
            "BKBM6M" => Self::BKBM6m,
            "NIBOR1M" => Self::NIBOR1m,
            "NIBOR3M" => Self::NIBOR3m,
            "NIBOR6M" => Self::NIBOR6m,
            "NIBOR12M" => Self::NIBOR12m,
            "SHIBOR1D" => Self::SHIBOR1d,
            "SHIBOR1W" => Self::SHIBOR1w,
            "SHIBOR2W" => Self::SHIBOR2w,
            "SHIBOR1M" => Self::SHIBOR1m,
            "SHIBOR3M" => Self::SHIBOR3m,
            "SHIBOR6M" => Self::SHIBOR6m,
            "SHIBOR12M" => Self::SHIBOR12m,
            "EURIBOR1W" => Self::EURIBOR1w,

            "SOFR" => Self::SOFR,
            "SOFRCOMPOUNDED" => Self::SOFRCompounded,
            "TERMSOFR1M" => Self::TermSOFR1m,
            "TERMSOFR3M" => Self::TermSOFR3m,
            "TERMSOFR6M" => Self::TermSOFR6m,
            "TERMSOFR12M" => Self::TermSOFR12m,
            "ESTR" | "€STR" => Self::ESTR,
            "EURIBOR1M" => Self::EURIBOR1m,
            "EURIBOR3M" => Self::EURIBOR3m,
            "EURIBOR6M" => Self::EURIBOR6m,
            "EURIBOR12M" => Self::EURIBOR12m,
            "SONIA" => Self::SONIA,
            "TONAR" => Self::TONAR,
            "TIBOR3M" => Self::TIBOR3m,
            "TIBOR6M" => Self::TIBOR6m,
            "SARON" => Self::SARON,
            "CORRA" => Self::CORRA,
            "AONIA" => Self::AONIA,
            "NZONIA" => Self::NZONIA,
            "NOWA" => Self::NOWA,
            "SWESTR" => Self::SWESTR,
            "ICP" => Self::ICP,
            "VIX" => Self::VIX,
            _ => match s {
                other if other.starts_with("FxPair(") && other.ends_with(')') => {
                    let pair_str = &other["FxPair(".len()..other.len() - 1];
                    pair_str
                        .parse::<FxPair>()
                        .map_or_else(|_| Self::Other(other.to_string()), Self::FxPair)
                }
                other if other.starts_with("Credit(") && other.ends_with(')') => {
                    let entity = &other["Credit(".len()..other.len() - 1];
                    Self::Credit(entity.trim().to_string())
                }
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
            },
        };
        Ok(result)
    }
}

impl Default for MarketIndex {
    fn default() -> Self {
        Self::Other("UNKNOWN".to_string())
    }
}

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
            Self::FedFunds => Ok(Box::new(FedFundsIndex)),
            Self::CDI => Ok(Box::new(CDIIndex)),
            Self::DESTR => Ok(Box::new(DESTRIndex)),
            Self::KOFR => Ok(Box::new(KOFRIndex)),
            Self::ZARONIA => Ok(Box::new(ZARONIAIndex)),
            Self::BBSW1m => Ok(Box::new(BBSW1mIndex)),
            Self::BBSW2m => Ok(Box::new(BBSW2mIndex)),
            Self::BBSW3m => Ok(Box::new(BBSW3mIndex)),
            Self::BBSW4m => Ok(Box::new(BBSW4mIndex)),
            Self::BBSW6m => Ok(Box::new(BBSW6mIndex)),
            Self::BKBM1m => Ok(Box::new(BKBM1mIndex)),
            Self::BKBM2m => Ok(Box::new(BKBM2mIndex)),
            Self::BKBM3m => Ok(Box::new(BKBM3mIndex)),
            Self::BKBM4m => Ok(Box::new(BKBM4mIndex)),
            Self::BKBM6m => Ok(Box::new(BKBM6mIndex)),
            Self::NIBOR1m => Ok(Box::new(NIBOR1mIndex)),
            Self::NIBOR3m => Ok(Box::new(NIBOR3mIndex)),
            Self::NIBOR6m => Ok(Box::new(NIBOR6mIndex)),
            Self::NIBOR12m => Ok(Box::new(NIBOR12mIndex)),
            Self::SHIBOR1d => Ok(Box::new(SHIBOR1dIndex)),
            Self::SHIBOR1w => Ok(Box::new(SHIBOR1wIndex)),
            Self::SHIBOR2w => Ok(Box::new(SHIBOR2wIndex)),
            Self::SHIBOR1m => Ok(Box::new(SHIBOR1mIndex)),
            Self::SHIBOR3m => Ok(Box::new(SHIBOR3mIndex)),
            Self::SHIBOR6m => Ok(Box::new(SHIBOR6mIndex)),
            Self::SHIBOR12m => Ok(Box::new(SHIBOR12mIndex)),
            Self::EURIBOR1w => Ok(Box::new(Euribor1wIndex)),

            Self::SOFR => Ok(Box::new(SOFRIndex)),
            Self::SOFRCompounded => Ok(Box::new(SOFRCompoundedIndex)),
            Self::TermSOFR1m => Ok(Box::new(TermSOFR1mIndex)),
            Self::TermSOFR3m => Ok(Box::new(TermSOFR3mIndex)),
            Self::TermSOFR6m => Ok(Box::new(TermSOFR6mIndex)),
            Self::TermSOFR12m => Ok(Box::new(TermSOFR12mIndex)),
            Self::ESTR => Ok(Box::new(ESTRIndex)),
            Self::EURIBOR1m => Ok(Box::new(Euribor1mIndex)),
            Self::EURIBOR3m => Ok(Box::new(Euribor3mIndex)),
            Self::EURIBOR6m => Ok(Box::new(Euribor6mIndex)),
            Self::EURIBOR12m => Ok(Box::new(Euribor12mIndex)),
            Self::SONIA => Ok(Box::new(SONIAIndex)),
            Self::TONAR => Ok(Box::new(TONARIndex)),
            Self::TIBOR3m => Ok(Box::new(Tibor3mIndex)),
            Self::TIBOR6m => Ok(Box::new(Tibor6mIndex)),
            Self::SARON => Ok(Box::new(SARONIndex)),
            Self::CORRA => Ok(Box::new(CORRAIndex)),
            Self::AONIA => Ok(Box::new(AONIAIndex)),
            Self::NZONIA => Ok(Box::new(NZONIAIndex)),
            Self::NOWA => Ok(Box::new(NOWAIndex)),
            Self::SWESTR => Ok(Box::new(SWESTRIndex)),
            Self::ICP => Ok(Box::new(ICPIndex)),
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
            Self::FedFunds => Ok(Box::new(FedFundsIndex)),
            Self::CDI => Ok(Box::new(CDIIndex)),
            Self::DESTR => Ok(Box::new(DESTRIndex)),
            Self::KOFR => Ok(Box::new(KOFRIndex)),
            Self::ZARONIA => Ok(Box::new(ZARONIAIndex)),
            Self::BBSW1m => Ok(Box::new(BBSW1mIndex)),
            Self::BBSW2m => Ok(Box::new(BBSW2mIndex)),
            Self::BBSW3m => Ok(Box::new(BBSW3mIndex)),
            Self::BBSW4m => Ok(Box::new(BBSW4mIndex)),
            Self::BBSW6m => Ok(Box::new(BBSW6mIndex)),
            Self::BKBM1m => Ok(Box::new(BKBM1mIndex)),
            Self::BKBM2m => Ok(Box::new(BKBM2mIndex)),
            Self::BKBM3m => Ok(Box::new(BKBM3mIndex)),
            Self::BKBM4m => Ok(Box::new(BKBM4mIndex)),
            Self::BKBM6m => Ok(Box::new(BKBM6mIndex)),
            Self::NIBOR1m => Ok(Box::new(NIBOR1mIndex)),
            Self::NIBOR3m => Ok(Box::new(NIBOR3mIndex)),
            Self::NIBOR6m => Ok(Box::new(NIBOR6mIndex)),
            Self::NIBOR12m => Ok(Box::new(NIBOR12mIndex)),
            Self::SHIBOR1d => Ok(Box::new(SHIBOR1dIndex)),
            Self::SHIBOR1w => Ok(Box::new(SHIBOR1wIndex)),
            Self::SHIBOR2w => Ok(Box::new(SHIBOR2wIndex)),
            Self::SHIBOR1m => Ok(Box::new(SHIBOR1mIndex)),
            Self::SHIBOR3m => Ok(Box::new(SHIBOR3mIndex)),
            Self::SHIBOR6m => Ok(Box::new(SHIBOR6mIndex)),
            Self::SHIBOR12m => Ok(Box::new(SHIBOR12mIndex)),
            Self::EURIBOR1w => Ok(Box::new(Euribor1wIndex)),

            Self::SOFR => Ok(Box::new(SOFRIndex)),
            Self::SOFRCompounded => Ok(Box::new(SOFRCompoundedIndex)),
            Self::TermSOFR1m => Ok(Box::new(TermSOFR1mIndex)),
            Self::TermSOFR3m => Ok(Box::new(TermSOFR3mIndex)),
            Self::TermSOFR6m => Ok(Box::new(TermSOFR6mIndex)),
            Self::TermSOFR12m => Ok(Box::new(TermSOFR12mIndex)),
            Self::ESTR => Ok(Box::new(ESTRIndex)),
            Self::EURIBOR1m => Ok(Box::new(Euribor1mIndex)),
            Self::EURIBOR3m => Ok(Box::new(Euribor3mIndex)),
            Self::EURIBOR6m => Ok(Box::new(Euribor6mIndex)),
            Self::EURIBOR12m => Ok(Box::new(Euribor12mIndex)),
            Self::SONIA => Ok(Box::new(SONIAIndex)),
            Self::TONAR => Ok(Box::new(TONARIndex)),
            Self::TIBOR3m => Ok(Box::new(Tibor3mIndex)),
            Self::TIBOR6m => Ok(Box::new(Tibor6mIndex)),
            Self::SARON => Ok(Box::new(SARONIndex)),
            Self::CORRA => Ok(Box::new(CORRAIndex)),
            Self::AONIA => Ok(Box::new(AONIAIndex)),
            Self::NZONIA => Ok(Box::new(NZONIAIndex)),
            Self::NOWA => Ok(Box::new(NOWAIndex)),
            Self::SWESTR => Ok(Box::new(SWESTRIndex)),
            Self::ICP => Ok(Box::new(ICPIndex)),
            _ => Err(QSError::InvalidValueErr("Index is not rate index".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_names_parse_in_any_case() {
        for index in [
            MarketIndex::EURIBOR1w,
            MarketIndex::EURIBOR1m,
            MarketIndex::EURIBOR3m,
            MarketIndex::EURIBOR6m,
            MarketIndex::EURIBOR12m,
            MarketIndex::SOFR,
            MarketIndex::SOFRCompounded,
            MarketIndex::TermSOFR3m,
            MarketIndex::FedFunds,
            MarketIndex::BBSW3m,
            MarketIndex::BKBM3m,
            MarketIndex::NIBOR3m,
            MarketIndex::SHIBOR1d,
            MarketIndex::TIBOR3m,
        ] {
            let name = index.to_string();
            for spelling in [
                name.clone(),
                name.to_ascii_lowercase(),
                name.to_ascii_uppercase(),
            ] {
                assert_eq!(
                    spelling.parse::<MarketIndex>().unwrap(),
                    index,
                    "{spelling}"
                );
            }
        }
        for spelling in ["ESTR", "estr", "eStR", "€STR", "€str"] {
            assert_eq!(spelling.parse::<MarketIndex>().unwrap(), MarketIndex::ESTR);
        }
        assert_eq!(
            "EuRiBoR3m".parse::<MarketIndex>().unwrap(),
            MarketIndex::EURIBOR3m
        );
    }

    #[test]
    fn custom_names_and_parameter_payloads_keep_their_case() {
        for name in [
            "MyCurve",
            "MYCURVE",
            "mycurve",
            "FxPair(BadPair)",
            "Collateral(Bad/Pair)",
        ] {
            assert_eq!(
                name.parse::<MarketIndex>().unwrap(),
                MarketIndex::Other(name.into())
            );
        }
        assert_eq!(
            "Credit(AcmeCorp)".parse::<MarketIndex>().unwrap(),
            MarketIndex::Credit("AcmeCorp".into())
        );
        assert_eq!(
            "Collateral(EUR/USD)".parse::<MarketIndex>().unwrap(),
            MarketIndex::Collateral(Currency::EUR, Currency::USD)
        );
    }

    #[test]
    fn fx_pair_indices_use_parentheses_and_preserve_direction() {
        for (label, base, quote) in [
            ("FxPair(EURUSD)", Currency::EUR, Currency::USD),
            ("FxPair(USDEUR)", Currency::USD, Currency::EUR),
        ] {
            let index = MarketIndex::FxPair(FxPair::new(base, quote).unwrap());
            assert_eq!(label.parse::<MarketIndex>().unwrap(), index);
            assert_eq!(index.to_string(), label);
            assert_eq!(index.to_string().parse::<MarketIndex>().unwrap(), index);
        }
    }

    #[test]
    fn malformed_fx_pair_indices_remain_custom_names() {
        for label in [
            "FxPair(EURUSD",
            "FxPairEURUSD)",
            "FxPair()",
            "FxPair(EUR)",
            "FxPair(EUREUR)",
            "FxPair(EURUSD)extra",
        ] {
            assert_eq!(
                label.parse::<MarketIndex>().unwrap(),
                MarketIndex::Other(label.into())
            );
        }
    }
}
