//! BKBM conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::enums::Frequency;

use crate::time::{calendar::Calendar, calendars::newzealand::NewZealand};

rate_index!(
    BKBM1mIndex,
    BKBM1m,
    NZD,
    Calendar::NewZealand(NewZealand::default()),
    Actual365,
    Simple,
    0,
    Monthly,
    false,
    (|_| Frequency::Semiannual, Actual365, Monthly, 0)
);
rate_index!(
    BKBM2mIndex,
    BKBM2m,
    NZD,
    Calendar::NewZealand(NewZealand::default()),
    Actual365,
    Simple,
    0,
    Bimonthly,
    false,
    (|_| Frequency::Semiannual, Actual365, Bimonthly, 0)
);
rate_index!(
    BKBM3mIndex,
    BKBM3m,
    NZD,
    Calendar::NewZealand(NewZealand::default()),
    Actual365,
    Simple,
    0,
    Quarterly,
    false,
    (|_| Frequency::Semiannual, Actual365, Quarterly, 0)
);
rate_index!(
    BKBM4mIndex,
    BKBM4m,
    NZD,
    Calendar::NewZealand(NewZealand::default()),
    Actual365,
    Simple,
    0,
    EveryFourthMonth,
    false,
    (|_| Frequency::Semiannual, Actual365, EveryFourthMonth, 0)
);
rate_index!(
    BKBM6mIndex,
    BKBM6m,
    NZD,
    Calendar::NewZealand(NewZealand::default()),
    Actual365,
    Simple,
    0,
    Semiannual,
    false,
    (|_| Frequency::Semiannual, Actual365, Semiannual, 0)
);
