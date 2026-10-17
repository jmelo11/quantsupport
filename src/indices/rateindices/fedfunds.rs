//! FEDFUNDS conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::{
    calendar::Calendar,
    calendars::unitedstates::{Market, UnitedStates},
};

rate_index!(
    FedFundsIndex,
    FedFunds,
    USD,
    Calendar::UnitedStates(UnitedStates::new(Market::FederalReserve)),
    Actual360,
    Simple,
    0,
    Daily,
    true
);
