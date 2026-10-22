/// AONIA (AUD) index implementation.
pub mod aonia;
/// CORRA (CAD) index implementation.
pub mod corra;
/// €STR (EUR) index implementation.
pub mod estr;
/// EURIBOR (EUR) index implementation.
pub mod euribor;
/// ICP (CLP) index implementation.
pub mod icp;
/// NOWA (NOK) index implementation.
pub mod nowa;
/// NZONIA (NZD) index implementation.
pub mod nzonia;
/// SARON (CHF) index implementation.
pub mod saron;
/// SOFR (USD) index implementation.
pub mod sofr;
/// SONIA (GBP) index implementation.
pub mod sonia;
/// SWESTR (SEK) index implementation.
pub mod swestr;
/// TIBOR (JPY) index implementation.
pub mod tibor;
/// TONAR (JPY) index implementation.
pub mod tonar;

// Convention-only rate definitions share the same metadata implementation.
macro_rules! rate_index {
    ($name:ident, $variant:ident, $ccy:ident, $calendar:expr, $dc:ident, $comp:ident, $lag:expr, $frequency:ident, $arrears:expr, ($fixed_frequency:expr, $fixed_dc:ident, $floating_frequency:ident, $settlement:expr)) => {
        #[doc = concat!("Conventions for the ", stringify!($variant), " rate index.")]
        #[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
        pub struct $name;
        impl crate::indices::marketindex::MarketIndexDetails for $name {
            fn name(&self) -> &'static str {
                stringify!($variant)
            }
            fn quote_type(&self) -> crate::indices::quotetype::QuoteType {
                crate::indices::quotetype::QuoteType::Rate
            }
        }
        impl crate::indices::rateindex::RateIndexDetails for $name {
            fn currency(&self) -> crate::currencies::currency::Currency {
                crate::currencies::currency::Currency::$ccy
            }
            fn calendar(&self) -> crate::time::calendar::Calendar {
                $calendar
            }
            fn fixing_lag(&self) -> i64 {
                $lag
            }
            fn rate_definition(&self) -> crate::rates::interestrate::RateDefinition {
                crate::rates::interestrate::RateDefinition::new(
                    crate::time::daycounter::DayCounter::$dc,
                    crate::rates::compounding::Compounding::$comp,
                    crate::time::enums::Frequency::Annual,
                )
            }
            fn swap_conventions(
                &self,
                tenor: crate::time::period::Period,
            ) -> crate::indices::rateindex::SwapConventions {
                crate::indices::rateindex::SwapConventions::new(
                    crate::rates::interestrate::RateDefinition::new(
                        crate::time::daycounter::DayCounter::$fixed_dc,
                        crate::rates::compounding::Compounding::$comp,
                        crate::time::enums::Frequency::Annual,
                    ),
                    ($fixed_frequency)(tenor),
                    crate::time::enums::Frequency::$floating_frequency,
                    crate::time::daycounter::DayCounter::$dc,
                    $settlement,
                    crate::time::enums::BusinessDayConvention::ModifiedFollowing,
                )
            }
            fn market_index(&self) -> crate::indices::marketindex::MarketIndex {
                crate::indices::marketindex::MarketIndex::$variant
            }
            fn frequency(&self) -> crate::time::enums::Frequency {
                crate::time::enums::Frequency::$frequency
            }
            fn is_in_arrears(&self) -> bool {
                $arrears
            }
        }
    };
}

/// FEDFUNDS rate-index conventions.
pub mod fedfunds;

/// CDI rate-index conventions.
pub mod cdi;

/// DESTR rate-index conventions.
pub mod destr;

/// KOFR rate-index conventions.
pub mod kofr;

/// ZARONIA rate-index conventions.
pub mod zaronia;

/// BBSW rate-index conventions.
pub mod bbsw;

/// BKBM rate-index conventions.
pub mod bkbm;

/// NIBOR rate-index conventions.
pub mod nibor;

/// SHIBOR rate-index conventions.
pub mod shibor;
