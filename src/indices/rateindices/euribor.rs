use serde::{Deserialize, Serialize};

use crate::{
    currencies::currency::Currency,
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::{RateIndexDetails, SwapConventions},
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{
        calendar::Calendar, calendars::target::TARGET, daycounter::DayCounter, enums::Frequency,
    },
};

macro_rules! euribor_index {
    ($name:ident, $doc:expr, $display:expr, $variant:ident, $frequency:expr) => {
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
            /// Returns the EURIBOR observation tenor, independently of compounding.
            fn frequency(&self) -> Frequency {
                $frequency
            }
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

            fn swap_conventions(&self, _tenor: crate::time::period::Period) -> SwapConventions {
                SwapConventions::new(
                    RateDefinition::new(
                        DayCounter::Thirty360,
                        Compounding::Simple,
                        Frequency::Annual,
                    ),
                    Frequency::Annual,
                    $frequency,
                    self.rate_definition().day_counter(),
                    2,
                    crate::time::enums::BusinessDayConvention::ModifiedFollowing,
                )
            }

            fn market_index(&self) -> MarketIndex {
                MarketIndex::$variant
            }

            fn is_in_arrears(&self) -> bool {
                false
            }
        }
    };
}

euribor_index!(
    Euribor1mIndex,
    "Details for the EURIBOR 1-month rate index.",
    "EURIBOR1m",
    EURIBOR1m,
    Frequency::Monthly
);
euribor_index!(
    Euribor3mIndex,
    "Details for the EURIBOR 3-month rate index.",
    "EURIBOR3m",
    EURIBOR3m,
    Frequency::Quarterly
);
euribor_index!(
    Euribor6mIndex,
    "Details for the EURIBOR 6-month rate index.",
    "EURIBOR6m",
    EURIBOR6m,
    Frequency::Semiannual
);
euribor_index!(
    Euribor12mIndex,
    "Details for the EURIBOR 12-month rate index.",
    "EURIBOR12m",
    EURIBOR12m,
    Frequency::Annual
);

euribor_index!(
    Euribor1wIndex,
    "Details for the EURIBOR 1-week rate index.",
    "EURIBOR1w",
    EURIBOR1w,
    Frequency::Weekly
);
