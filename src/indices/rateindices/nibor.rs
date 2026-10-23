//! NIBOR conventions, checked against QuantLib 5f4811e7e03b.

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
        calendars::norway::Norway,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Conventions for the `NIBOR1m` rate index.
    NIBOR1mIndex {
        market_index: NIBOR1m,
        currency: NOK,
        calendar: Calendar::Norway(Norway::new()),
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
    /// Conventions for the `NIBOR3m` rate index.
    NIBOR3mIndex {
        market_index: NIBOR3m,
        currency: NOK,
        calendar: Calendar::Norway(Norway::new()),
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
    /// Conventions for the `NIBOR6m` rate index.
    NIBOR6mIndex {
        market_index: NIBOR6m,
        currency: NOK,
        calendar: Calendar::Norway(Norway::new()),
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
    /// Conventions for the `NIBOR12m` rate index.
    NIBOR12mIndex {
        market_index: NIBOR12m,
        currency: NOK,
        calendar: Calendar::Norway(Norway::new()),
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
