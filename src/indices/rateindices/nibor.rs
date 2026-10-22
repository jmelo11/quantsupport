//! NIBOR conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::enums::Frequency;

use crate::time::{calendar::Calendar, calendars::norway::Norway};

rate_index!(
    NIBOR1mIndex,
    NIBOR1m,
    NOK,
    Calendar::Norway(Norway::new()),
    Actual360,
    Simple,
    2,
    Monthly,
    false,
    (|_| Frequency::Annual, Thirty360, Monthly, 2)
);
rate_index!(
    NIBOR3mIndex,
    NIBOR3m,
    NOK,
    Calendar::Norway(Norway::new()),
    Actual360,
    Simple,
    2,
    Quarterly,
    false,
    (|_| Frequency::Annual, Thirty360, Quarterly, 2)
);
rate_index!(
    NIBOR6mIndex,
    NIBOR6m,
    NOK,
    Calendar::Norway(Norway::new()),
    Actual360,
    Simple,
    2,
    Semiannual,
    false,
    (|_| Frequency::Annual, Thirty360, Semiannual, 2)
);
rate_index!(
    NIBOR12mIndex,
    NIBOR12m,
    NOK,
    Calendar::Norway(Norway::new()),
    Actual360,
    Simple,
    2,
    Annual,
    false,
    (|_| Frequency::Annual, Thirty360, Annual, 2)
);
