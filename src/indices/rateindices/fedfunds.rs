//! FEDFUNDS conventions, checked against QuantLib 5f4811e7e03b.

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
        calendars::unitedstates::{Market, UnitedStates},
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

use super::macros::rate_index;

rate_index! {
    /// Conventions for the `FedFunds` rate index.
    FedFundsIndex {
        market_index: FedFunds,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::FederalReserve)),
        fixing_lag: 0,
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
