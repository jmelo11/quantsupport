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
    /// Details for the SOFR overnight rate index.
    SOFRIndex {
        market_index: SOFR,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::Sofr)),
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

rate_index! {
    /// Details for the SOFR Compounded rate index.
    SOFRCompoundedIndex {
        market_index: SOFRCompounded,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::Sofr)),
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

rate_index! {
    /// Details for the Term-SOFR 1-month rate index.
    TermSOFR1mIndex {
        market_index: TermSOFR1m,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::Sofr)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Monthly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual360,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Monthly,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the Term-SOFR 3-month rate index.
    TermSOFR3mIndex {
        market_index: TermSOFR3m,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::Sofr)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Quarterly,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual360,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Quarterly,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the Term-SOFR 6-month rate index.
    TermSOFR6mIndex {
        market_index: TermSOFR6m,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::Sofr)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Semiannual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual360,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Semiannual,
            settlement_days: 2,
        },
    }
}

rate_index! {
    /// Details for the Term-SOFR 12-month rate index.
    TermSOFR12mIndex {
        market_index: TermSOFR12m,
        currency: USD,
        calendar: Calendar::UnitedStates(UnitedStates::new(Market::Sofr)),
        fixing_lag: 1,
        day_counter: Actual360,
        compounding: Simple,
        compounding_frequency: Annual,
        frequency: Annual,
        is_in_arrears: false,
        swap_conventions: {
            fixed_day_counter: Actual360,
            fixed_frequency: |_| Frequency::Semiannual,
            floating_frequency: Annual,
            settlement_days: 2,
        },
    }
}
