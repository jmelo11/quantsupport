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
        calendar::Calendar, calendars::weekendsonly::WeekendsOnly, daycounter::DayCounter,
        enums::Frequency,
    },
};

macro_rules! tibor_index {
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
                Calendar::WeekendsOnly(WeekendsOnly::new())
            }

            fn currency(&self) -> Currency {
                Currency::JPY
            }

            fn fixing_lag(&self) -> i64 {
                2
            }

            fn rate_definition(&self) -> RateDefinition {
                RateDefinition::new(
                    DayCounter::Actual365,
                    Compounding::Simple,
                    Frequency::Semiannual,
                )
            }

            fn market_index(&self) -> MarketIndex {
                MarketIndex::$variant
            }
        }
    };
}

tibor_index!(Tibor3mIndex, "Details for the TIBOR 3-month rate index.", "TIBOR3m", TIBOR3m);
tibor_index!(Tibor6mIndex, "Details for the TIBOR 6-month rate index.", "TIBOR6m", TIBOR6m);
