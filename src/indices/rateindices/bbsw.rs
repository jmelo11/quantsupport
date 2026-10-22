//! BBSW conventions, checked against QuantLib 5f4811e7e03b.

use crate::time::enums::Frequency;

use crate::time::{calendar::Calendar, calendars::australia::Australia};

rate_index!(
    BBSW1mIndex,
    BBSW1m,
    AUD,
    Calendar::Australia(Australia::default()),
    Actual365,
    Simple,
    0,
    Monthly,
    false,
    (bbsw_fixed_frequency, Actual365, Monthly, 1)
);
rate_index!(
    BBSW2mIndex,
    BBSW2m,
    AUD,
    Calendar::Australia(Australia::default()),
    Actual365,
    Simple,
    0,
    Bimonthly,
    false,
    (bbsw_fixed_frequency, Actual365, Bimonthly, 1)
);
rate_index!(
    BBSW3mIndex,
    BBSW3m,
    AUD,
    Calendar::Australia(Australia::default()),
    Actual365,
    Simple,
    0,
    Quarterly,
    false,
    (bbsw_fixed_frequency, Actual365, Quarterly, 1)
);
rate_index!(
    BBSW4mIndex,
    BBSW4m,
    AUD,
    Calendar::Australia(Australia::default()),
    Actual365,
    Simple,
    0,
    EveryFourthMonth,
    false,
    (bbsw_fixed_frequency, Actual365, EveryFourthMonth, 1)
);
rate_index!(
    BBSW6mIndex,
    BBSW6m,
    AUD,
    Calendar::Australia(Australia::default()),
    Actual365,
    Simple,
    0,
    Semiannual,
    false,
    (bbsw_fixed_frequency, Actual365, Semiannual, 1)
);

// AFMA: quarterly through three years, semiannual from four years.
fn bbsw_fixed_frequency(tenor: crate::time::period::Period) -> Frequency {
    if tenor < crate::time::period::Period::new(4, crate::time::enums::TimeUnit::Years) {
        Frequency::Quarterly
    } else {
        Frequency::Semiannual
    }
}
