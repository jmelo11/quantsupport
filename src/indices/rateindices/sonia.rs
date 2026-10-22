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
        calendar::Calendar, calendars::unitedkingdom::UnitedKingdom, daycounter::DayCounter,
        enums::Frequency,
    },
};

/// Details for SONIA (Sterling Overnight Index Average).
/// Overnight unsecured rate administered by the Bank of England.
#[derive(Copy, Clone, Serialize, Default, Deserialize)]
pub struct SONIAIndex;

impl MarketIndexDetails for SONIAIndex {
    fn quote_type(&self) -> QuoteType {
        QuoteType::Rate
    }

    fn name(&self) -> &'static str {
        "SONIA"
    }
}

impl RateIndexDetails for SONIAIndex {
    fn calendar(&self) -> Calendar {
        Calendar::UnitedKingdom(UnitedKingdom::new())
    }

    fn currency(&self) -> Currency {
        Currency::GBP
    }

    fn fixing_lag(&self) -> i64 {
        0
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
            0,
            crate::time::enums::BusinessDayConvention::ModifiedFollowing,
        )
    }

    fn market_index(&self) -> MarketIndex {
        MarketIndex::SONIA
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{calendars::traits::IsCalendar, date::Date};
    #[test]
    fn overnight_index_uses_local_bank_holidays() {
        assert!(SONIAIndex.calendar().is_holiday(&Date::new(2026, 4, 3)));
    }
}
