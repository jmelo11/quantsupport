//! BKBM conventions, checked against QuantLib 5f4811e7e03b.

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
        calendars::newzealand::NewZealand,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Conventions for the `BKBM1m` rate index.
    BKBM1mIndex {
        market_index: BKBM1m,
        currency: NZD,
        calendar: Calendar::NewZealand(NewZealand::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Monthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Monthly,
            settlement_days: 0,
        },
    }
}

rate_index! {
    /// Conventions for the `BKBM2m` rate index.
    BKBM2mIndex {
        market_index: BKBM2m,
        currency: NZD,
        calendar: Calendar::NewZealand(NewZealand::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Bimonthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Bimonthly,
            settlement_days: 0,
        },
    }
}

rate_index! {
    /// Conventions for the `BKBM3m` rate index.
    BKBM3mIndex {
        market_index: BKBM3m,
        currency: NZD,
        calendar: Calendar::NewZealand(NewZealand::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Quarterly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Quarterly,
            settlement_days: 0,
        },
    }
}

rate_index! {
    /// Conventions for the `BKBM4m` rate index.
    BKBM4mIndex {
        market_index: BKBM4m,
        currency: NZD,
        calendar: Calendar::NewZealand(NewZealand::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: EveryFourthMonth,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: EveryFourthMonth,
            settlement_days: 0,
        },
    }
}

rate_index! {
    /// Conventions for the `BKBM6m` rate index.
    BKBM6mIndex {
        market_index: BKBM6m,
        currency: NZD,
        calendar: Calendar::NewZealand(NewZealand::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Semiannual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Semiannual,
            settlement_days: 0,
        },
    }
}
