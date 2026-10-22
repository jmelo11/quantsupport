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
        calendar::Calendar, calendars::norway::Norway, daycounter::DayCounter, enums::Frequency,
    },
};

/// Details for NOWA (Norwegian Overnight Weighted Average).
/// Overnight unsecured rate published by Norges Bank.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct NOWAIndex;

impl MarketIndexDetails for NOWAIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "NOWA"
    }
}

impl RateIndexDetails for NOWAIndex {
    fn calendar(&self) -> Calendar {
        Calendar::Norway(Norway::default())
    }

    fn currency(&self) -> Currency {
        Currency::NOK
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

    fn swap_conventions(&self, _tenor: crate::time::period::Period) -> SwapConventions {
        SwapConventions::new(
            self.rate_definition(),
            Frequency::Annual,
            Frequency::Annual,
            self.rate_definition().day_counter(),
            2,
            crate::time::enums::BusinessDayConvention::ModifiedFollowing,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::NOWA
    }
}
