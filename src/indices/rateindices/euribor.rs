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
        calendars::target::TARGET,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Details for the EURIBOR 1-month rate index.
    Euribor1mIndex {
        market_index: EURIBOR1m,
        currency: EUR,
        calendar: Calendar::TARGET(TARGET::new()),
        fixing_lag: 2,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Monthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Thirty360,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Monthly,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the EURIBOR 3-month rate index.
    Euribor3mIndex {
        market_index: EURIBOR3m,
        currency: EUR,
        calendar: Calendar::TARGET(TARGET::new()),
        fixing_lag: 2,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Quarterly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Thirty360,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Quarterly,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the EURIBOR 6-month rate index.
    Euribor6mIndex {
        market_index: EURIBOR6m,
        currency: EUR,
        calendar: Calendar::TARGET(TARGET::new()),
        fixing_lag: 2,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Semiannual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Thirty360,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Semiannual,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the EURIBOR 12-month rate index.
    Euribor12mIndex {
        market_index: EURIBOR12m,
        currency: EUR,
        calendar: Calendar::TARGET(TARGET::new()),
        fixing_lag: 2,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Annual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Thirty360,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Annual,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the EURIBOR 1-week rate index.
    Euribor1wIndex {
        market_index: EURIBOR1w,
        currency: EUR,
        calendar: Calendar::TARGET(TARGET::new()),
        fixing_lag: 2,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Weekly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Thirty360,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Weekly,
            settlement_days: 2,
        },
    }
}
