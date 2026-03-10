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
        calendar::Calendar, calendars::target::TARGET, daycounter::DayCounter, enums::Frequency,
    },
};

macro_rules! euribor_index {
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
                Calendar::TARGET(TARGET::new())
            }

            fn currency(&self) -> Currency {
                Currency::EUR
            }

            fn fixing_lag(&self) -> i64 {
                2
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

euribor_index!(
    Euribor1mIndex,
    "Details for the EURIBOR 1-month rate index.",
    "EURIBOR1m",
    EURIBOR1m
);
euribor_index!(
    Euribor3mIndex,
    "Details for the EURIBOR 3-month rate index.",
    "EURIBOR3m",
    EURIBOR3m
);
euribor_index!(
    Euribor6mIndex,
    "Details for the EURIBOR 6-month rate index.",
    "EURIBOR6m",
    EURIBOR6m
);
euribor_index!(
    Euribor12mIndex,
    "Details for the EURIBOR 12-month rate index.",
    "EURIBOR12m",
    EURIBOR12m
);
