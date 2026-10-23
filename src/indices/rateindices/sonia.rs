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
        calendar::Calendar,
        calendars::unitedkingdom::UnitedKingdom,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Details for SONIA (Sterling Overnight Index Average).
    /// Overnight unsecured rate administered by the Bank of England.
    SONIAIndex {
        market_index: SONIA,
        currency: GBP,
        calendar: Calendar::UnitedKingdom(UnitedKingdom::new()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Daily,
        is_in_arrears: true,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Annual,
            settlement_days: 0,
        },
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
