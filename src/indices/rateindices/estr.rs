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
    /// Details for the €STR (Euro Short-Term Rate) index.
    /// Overnight unsecured rate published by the ECB.
    ESTRIndex {
        market_index: ESTR,
        currency: EUR,
        calendar: Calendar::TARGET(TARGET::new()),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Daily,
        is_in_arrears: true,
        swap_conventions: {
            fixed_day_counter: Actual360,
            fixed_frequency: |_| Frequency::Annual,
            floating_frequency: Annual,
            settlement_days: 2,
        },
    }
}
