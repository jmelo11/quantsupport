//! CDI conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::enums::Frequency;

use crate::time::{calendar::Calendar, calendars::brazil::Brazil};

rate_index!(
    CDIIndex,
    CDI,
    BRL,
    Calendar::Brazil(Brazil::default()),
    Business252,
    Compounded,
    0,
    Daily,
    true,
    (|_| Frequency::Once, Business252, Once, 0)
);
