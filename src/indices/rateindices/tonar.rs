use serde::{Deserialize, Serialize};

use crate::{
    currencies::currency::Currency,
    indices::{
        marketindex::{MarketIndex, MarketIndexDetails},
        quotetype::QuoteType,
        rateindex::{RateIndexDetails, SwapConventions},
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{calendar::Calendar, calendars::japan::Japan, daycounter::DayCounter, enums::Frequency},
};

/// Details for TONAR (Tokyo Overnight Average Rate).
/// Overnight unsecured rate published by the Bank of Japan.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct TONARIndex;

impl MarketIndexDetails for TONARIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "TONAR"
    }
}

impl RateIndexDetails for TONARIndex {
    fn calendar(&self) -> Calendar {
        Calendar::Japan(Japan::new())
    }

    fn currency(&self) -> Currency {
        Currency::JPY
    }

    fn fixing_lag(&self) -> i64 {
        1
    }

    fn rate_definition(&self) -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual365,
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
        MarketIndex::TONAR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{calendars::traits::IsCalendar, date::Date};
    #[test]
    fn overnight_index_uses_local_bank_holidays() {
        assert!(TONARIndex.calendar().is_holiday(&Date::new(2026, 5, 6)));
    }
}
