use quantsupport::prelude::{MakeSchedule, Period};
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use super::helpers::{
    parse_date, parse_day_counter, parse_frequency, parse_time_unit, value_error,
};

#[xll_bindgen(
    name = "QS.DATE.ADVANCE",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Advances an ISO date by a number of Days, Weeks, Months, or Years"
)]
pub fn qs_date_advance(date: &str, length: i32, unit: &str) -> Result<String, XllError> {
    Ok((parse_date(date)? + Period::new(length, parse_time_unit(unit)?)).to_string())
}

#[xll_bindgen(
    name = "QS.YEAR.FRACTION",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Returns the year fraction between two ISO dates"
)]
pub fn qs_year_fraction(
    start_date: &str,
    end_date: &str,
    day_counter: &str,
) -> Result<f64, XllError> {
    Ok(parse_day_counter(day_counter)?
        .year_fraction(parse_date(start_date)?, parse_date(end_date)?))
}

#[xll_bindgen(
    name = "QS.SCHEDULE",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Spills an unadjusted schedule between two ISO dates"
)]
pub fn qs_schedule(
    start_date: &str,
    end_date: &str,
    frequency: &str,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let schedule = MakeSchedule::new(parse_date(start_date)?, parse_date(end_date)?)
        .with_frequency(parse_frequency(frequency)?)
        .build()
        .map_err(value_error)?;
    Ok(super::helpers::string_column(
        schedule.dates().iter().map(ToString::to_string).collect(),
    ))
}
