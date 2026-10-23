//! SHIBOR conventions, checked against QuantLib 5f4811e7e03b.

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
        calendars::china::{China, Market},
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Conventions for the `SHIBOR1d` rate index.
    SHIBOR1dIndex {
        market_index: SHIBOR1d,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 0,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Daily,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Daily,
            settlement_days: 0,
        },
    }
}

rate_index! {
    /// Conventions for the `SHIBOR1w` rate index.
    SHIBOR1wIndex {
        market_index: SHIBOR1w,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Weekly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Weekly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `SHIBOR2w` rate index.
    SHIBOR2wIndex {
        market_index: SHIBOR2w,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Biweekly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Biweekly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `SHIBOR1m` rate index.
    SHIBOR1mIndex {
        market_index: SHIBOR1m,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Monthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Monthly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `SHIBOR3m` rate index.
    SHIBOR3mIndex {
        market_index: SHIBOR3m,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Quarterly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Quarterly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `SHIBOR6m` rate index.
    SHIBOR6mIndex {
        market_index: SHIBOR6m,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Semiannual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Semiannual,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `SHIBOR12m` rate index.
    SHIBOR12mIndex {
        market_index: SHIBOR12m,
        currency: CNY,
        calendar: Calendar::China(China::new(Market::IB)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Annual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: |_| Frequency::Quarterly,
            floating_frequency: Annual,
            settlement_days: 1,
        },
    }
}
