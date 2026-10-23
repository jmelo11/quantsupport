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
    /// Details for the TIBOR 3-month rate index.
    Tibor3mIndex {
        market_index: TIBOR3m,
        currency: JPY,
        calendar: Calendar::Japan(Japan::default()),
        fixing_lag: 2,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Semiannual,
        frequency: Quarterly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Quarterly,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the TIBOR 6-month rate index.
    Tibor6mIndex {
        market_index: TIBOR6m,
        currency: JPY,
        calendar: Calendar::Japan(Japan::default()),
        fixing_lag: 2,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Semiannual,
        frequency: Semiannual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Semiannual,
            settlement_days: 2,
        },
    }
}
