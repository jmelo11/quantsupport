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

#[xll_bindgen(
    name = "QS.CALENDAR.IS.BUSINESS.DAY",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Checks a date against a market or joint calendar"
)]
pub fn qs_calendar_is_business_day(date: &str, calendar: &str) -> Result<bool, XllError> {
    use quantsupport::prelude::{Calendar, IsCalendar};
    let calendar = Calendar::try_from(calendar.to_string()).map_err(value_error)?;
    Ok(calendar.is_business_day(&parse_date(date)?))
}

#[xll_bindgen(
    name = "QS.CALENDAR.ADJUST",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Adjusts an ISO date using a market or joint calendar"
)]
pub fn qs_calendar_adjust(
    date: &str,
    calendar: &str,
    convention: &str,
) -> Result<String, XllError> {
    use quantsupport::prelude::{BusinessDayConvention, Calendar, IsCalendar};
    let calendar = Calendar::try_from(calendar.to_string()).map_err(value_error)?;
    let convention =
        BusinessDayConvention::try_from(convention.to_string()).map_err(value_error)?;
    Ok(calendar
        .adjust(parse_date(date)?, Some(convention))
        .to_string())
}

#[xll_bindgen(
    name = "QS.SCHEDULE.ADJUSTED",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Spills a schedule adjusted with a market or joint calendar"
)]
pub fn qs_schedule_adjusted(
    start_date: &str,
    end_date: &str,
    frequency: &str,
    calendar: &str,
    convention: &str,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    use quantsupport::prelude::{BusinessDayConvention, Calendar};
    let calendar = Calendar::try_from(calendar.to_string()).map_err(value_error)?;
    let convention =
        BusinessDayConvention::try_from(convention.to_string()).map_err(value_error)?;
    let schedule = MakeSchedule::new(parse_date(start_date)?, parse_date(end_date)?)
        .with_frequency(parse_frequency(frequency)?)
        .with_calendar(calendar)
        .with_convention(convention)
        .build()
        .map_err(value_error)?;
    Ok(super::helpers::string_column(
        schedule.dates().iter().map(ToString::to_string).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worksheet_calendar_strings_resolve_joint_holidays() {
        let calendar = "JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)";
        assert_eq!(
            qs_calendar_adjust("2019-04-27", calendar, "Following").unwrap(),
            "2019-05-07"
        );
        assert!(!qs_calendar_is_business_day("2023-05-08", "UnitedKingdom(Settlement)").unwrap());
        assert!(qs_calendar_adjust("2019-04-27", "Unknown", "Following").is_err());
    }
}
