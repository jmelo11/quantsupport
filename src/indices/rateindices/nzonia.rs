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
    /// Details for NZONIA (New Zealand Overnight Index Average).
    /// Overnight unsecured rate based on the RBNZ Official Cash Rate.
    NZONIAIndex {
        market_index: NZONIA,
        currency: NZD,
        calendar: Calendar::NewZealand(NewZealand::default()),
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
            settlement_days: 2,
        },
    }
}
