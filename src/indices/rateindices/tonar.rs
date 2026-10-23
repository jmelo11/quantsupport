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
        calendars::japan::Japan,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Details for TONAR (Tokyo Overnight Average Rate).
    /// Overnight unsecured rate published by the Bank of Japan.
    TONARIndex {
        market_index: TONAR,
        currency: JPY,
        calendar: Calendar::Japan(Japan::new()),
        fixing_lag: 1,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Daily,
        is_in_arrears: true,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Annual,
            settlement_days: 2,
        },
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
