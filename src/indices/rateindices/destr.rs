//! DESTR conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::{calendar::Calendar, calendars::denmark::Denmark};

rate_index!(
    DESTRIndex,
    DESTR,
    DKK,
    Calendar::Denmark(Denmark::new()),
    Actual360,
    Simple,
    0,
    Daily,
    true
);
