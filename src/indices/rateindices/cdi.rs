//! CDI conventions, checked against QuantLib 5f4811e7e03b.

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
        calendars::brazil::Brazil,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Conventions for the `CDI` rate index.
    CDIIndex {
        market_index: CDI,
        currency: BRL,
        calendar: Calendar::Brazil(Brazil::default()),
        fixing_lag: 0,
        day_counter: Business252,
        compounding: Compounded,
        compounding_frequency: Annual,
        frequency: Daily,
        is_in_arrears: true,
        swap_conventions: {
            fixed_day_counter: Business252,
            fixed_frequency: |_| Frequency::Once,
            floating_frequency: Once,
            settlement_days: 0,
        },
    }
}
