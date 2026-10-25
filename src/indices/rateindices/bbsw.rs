//! BBSW conventions, checked against QuantLib 5f4811e7e03b.

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
        calendars::australia::Australia,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency, TimeUnit},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Conventions for the `BBSW1m` rate index.
    BBSW1mIndex {
        market_index: BBSW1m,
        currency: AUD,
        calendar: Calendar::Australia(Australia::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Monthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: bbsw_fixed_frequency,
            floating_frequency: Monthly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `BBSW2m` rate index.
    BBSW2mIndex {
        market_index: BBSW2m,
        currency: AUD,
        calendar: Calendar::Australia(Australia::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Bimonthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: bbsw_fixed_frequency,
            floating_frequency: Bimonthly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `BBSW3m` rate index.
    BBSW3mIndex {
        market_index: BBSW3m,
        currency: AUD,
        calendar: Calendar::Australia(Australia::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Quarterly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: bbsw_fixed_frequency,
            floating_frequency: Quarterly,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `BBSW4m` rate index.
    BBSW4mIndex {
        market_index: BBSW4m,
        currency: AUD,
        calendar: Calendar::Australia(Australia::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: EveryFourthMonth,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: bbsw_fixed_frequency,
            floating_frequency: EveryFourthMonth,
            settlement_days: 1,
        },
    }
}

rate_index! {
    /// Conventions for the `BBSW6m` rate index.
    BBSW6mIndex {
        market_index: BBSW6m,
        currency: AUD,
        calendar: Calendar::Australia(Australia::default()),
        fixing_lag: 0,
        day_counter: Actual365,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Semiannual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual365,
            fixed_frequency: bbsw_fixed_frequency,
            floating_frequency: Semiannual,
            settlement_days: 1,
        },
    }
}

// AFMA: quarterly through three years, semiannual from four years.
/// Selects quarterly fixed coupons below four years and semiannual coupons otherwise.
fn bbsw_fixed_frequency(tenor: Period) -> Frequency {
    if tenor < Period::new(4, TimeUnit::Years) {
        Frequency::Quarterly
    } else {
        Frequency::Semiannual
    }
}
