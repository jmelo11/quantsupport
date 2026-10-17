//! KOFR conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::{calendar::Calendar, calendars::southkorea::SouthKorea};

rate_index!(
    KOFRIndex,
    KOFR,
    KRW,
    Calendar::SouthKorea(SouthKorea::default()),
    Actual365,
    Simple,
    0,
    Daily,
    true
);
