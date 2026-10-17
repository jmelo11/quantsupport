//! ZARONIA conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::{calendar::Calendar, calendars::southafrica::SouthAfrica};

rate_index!(
    ZARONIAIndex,
    ZARONIA,
    ZAR,
    Calendar::SouthAfrica(SouthAfrica::new()),
    Actual365,
    Simple,
    0,
    Daily,
    true
);
