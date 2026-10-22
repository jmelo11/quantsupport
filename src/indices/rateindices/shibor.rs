//! SHIBOR conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::enums::Frequency;

use crate::time::{
    calendar::Calendar,
    calendars::china::{China, Market},
};

rate_index!(
    SHIBOR1dIndex,
    SHIBOR1d,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    0,
    Daily,
    false,
    (|_| Frequency::Quarterly, Actual365, Daily, 0)
);
rate_index!(
    SHIBOR1wIndex,
    SHIBOR1w,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    1,
    Weekly,
    false,
    (|_| Frequency::Quarterly, Actual365, Weekly, 1)
);
rate_index!(
    SHIBOR2wIndex,
    SHIBOR2w,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    1,
    Biweekly,
    false,
    (|_| Frequency::Quarterly, Actual365, Biweekly, 1)
);
rate_index!(
    SHIBOR1mIndex,
    SHIBOR1m,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    1,
    Monthly,
    false,
    (|_| Frequency::Quarterly, Actual365, Monthly, 1)
);
rate_index!(
    SHIBOR3mIndex,
    SHIBOR3m,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    1,
    Quarterly,
    false,
    (|_| Frequency::Quarterly, Actual365, Quarterly, 1)
);
rate_index!(
    SHIBOR6mIndex,
    SHIBOR6m,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    1,
    Semiannual,
    false,
    (|_| Frequency::Quarterly, Actual365, Semiannual, 1)
);
rate_index!(
    SHIBOR12mIndex,
    SHIBOR12m,
    CNY,
    Calendar::China(China::new(Market::IB)),
    Actual360,
    Simple,
    1,
    Annual,
    false,
    (|_| Frequency::Quarterly, Actual365, Annual, 1)
);
