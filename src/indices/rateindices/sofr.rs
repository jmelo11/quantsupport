use serde::{Deserialize, Serialize};

use crate::{
    currencies::currency::Currency,
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::RateIndexDetails,
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{
        calendar::Calendar,
        calendars::unitedstates::{Market, UnitedStates},
        daycounter::DayCounter,
        enums::Frequency,
    },
};

macro_rules! sofr_index {
    ($name:ident, $doc:expr, $display:expr, $variant:ident) => {
        #[doc = $doc]
        #[derive(Copy, Clone, Serialize, Default, Deserialize)]
        pub struct $name;

        impl MarketIndexDetails for $name {
            fn quote_type(&self) -> QuoteType {
                QuoteType::Rate
            }

            fn name(&self) -> &'static str {
                $display
            }
        }

        impl RateIndexDetails for $name {
            fn calendar(&self) -> Calendar {
                Calendar::UnitedStates(UnitedStates::new(Market::Sofr))
            }

            fn currency(&self) -> Currency {
                Currency::USD
            }

            fn fixing_lag(&self) -> i64 {
                1
            }

            fn rate_definition(&self) -> RateDefinition {
                RateDefinition::new(
                    DayCounter::Actual360,
                    Compounding::Simple,
                    Frequency::Annual,
                )
            }

            fn market_index(&self) -> MarketIndex {
                MarketIndex::$variant
            }
        }
    };
}

sofr_index!(SOFRIndex, "Details for the SOFR overnight rate index.", "SOFR", SOFR);
sofr_index!(SOFRCompoundedIndex, "Details for the SOFR Compounded rate index.", "SOFRCompounded", SOFRCompounded);
sofr_index!(TermSOFR1mIndex, "Details for the Term-SOFR 1-month rate index.", "TermSOFR1m", TermSOFR1m);
sofr_index!(TermSOFR3mIndex, "Details for the Term-SOFR 3-month rate index.", "TermSOFR3m", TermSOFR3m);
sofr_index!(TermSOFR6mIndex, "Details for the Term-SOFR 6-month rate index.", "TermSOFR6m", TermSOFR6m);
sofr_index!(TermSOFR12mIndex, "Details for the Term-SOFR 12-month rate index.", "TermSOFR12m", TermSOFR12m);
