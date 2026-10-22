#[cfg(windows)]
use quantsupport::prelude::{MakeSchedule, Period};
#[cfg(windows)]
mod datetime;
#[cfg(windows)]
use quantsupport_xll_macros::xll_bindgen;
#[cfg(windows)]
use xll_rs::types::XllError;

#[cfg(windows)]
use crate::functions::helpers::{
    bool_value, parse_date, parse_day_counter, parse_frequency, parse_time_unit, value_error,
};

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.DATE.ADVANCE",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Advances an ISO date by a number of Days, Weeks, Months, or Years"
)]
pub fn qs_date_advance(date: &str, length: i32, unit: &str) -> Result<String, XllError> {
    Ok((parse_date(date)? + Period::new(length, parse_time_unit(unit)?)).to_string())
}

#[cfg(windows)]
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

#[cfg(windows)]
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
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    let schedule = MakeSchedule::new(parse_date(start_date)?, parse_date(end_date)?)
        .with_frequency(parse_frequency(frequency)?)
        .build()
        .map_err(value_error)?;
    Ok(crate::functions::helpers::string_column(
        schedule.dates().iter().map(ToString::to_string).collect(),
        transpose,
    ))
}

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.CALENDAR.IS.HOLIDAY",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Checks whether an ISO date is a holiday in a canonical calendar"
)]
pub fn qs_calendar_is_holiday(date: &str, calendar: &str) -> Result<bool, XllError> {
    calendar_operations::is_holiday(date, calendar).map_err(calendar_value_error)
}

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.CALENDAR.END.OF.MONTH",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Returns the last business day of the month containing an ISO date"
)]
pub fn qs_calendar_end_of_month(date: &str, calendar: &str) -> Result<String, XllError> {
    calendar_operations::end_of_month(date, calendar).map_err(calendar_value_error)
}

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.CALENDAR.IS.END.OF.MONTH",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Checks whether Following adjustment of the next day crosses a month boundary"
)]
pub fn qs_calendar_is_end_of_month(date: &str, calendar: &str) -> Result<bool, XllError> {
    calendar_operations::is_end_of_month(date, calendar).map_err(calendar_value_error)
}

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.CALENDAR.BUSINESS.DAYS.BETWEEN",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Counts signed business days between ISO dates with optional boundary inclusion"
)]
pub fn qs_calendar_business_days_between(
    from: &str,
    to: &str,
    calendar: &str,
    include_first: *const xll_rs::types::XLOPER12,
    include_last: *const xll_rs::types::XLOPER12,
) -> Result<i32, XllError> {
    calendar_operations::business_days_between(
        from,
        to,
        calendar,
        optional_calendar_flag(include_first, "include_first")?,
        optional_calendar_flag(include_last, "include_last")?,
    )
    .map_err(calendar_value_error)
}

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.CALENDAR.HOLIDAYS",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Spills native calendar holidays between inclusive ISO date bounds"
)]
pub fn qs_calendar_holidays(
    from: &str,
    to: &str,
    calendar: &str,
    include_weekends: *const xll_rs::types::XLOPER12,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    Ok(crate::functions::helpers::string_column(
        calendar_operations::holidays(
            from,
            to,
            calendar,
            optional_calendar_flag(include_weekends, "include_weekends")?,
        )
        .map_err(calendar_value_error)?,
        transpose,
    ))
}

#[cfg(windows)]
#[xll_bindgen(
    name = "QS.CALENDAR.BUSINESS.DAYS",
    threadsafe,
    category = "QuantSupport - Time",
    help = "Spills native calendar business days between inclusive ISO date bounds"
)]
pub fn qs_calendar_business_days(
    from: &str,
    to: &str,
    calendar: &str,
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
    Ok(crate::functions::helpers::string_column(
        calendar_operations::business_days(from, to, calendar).map_err(calendar_value_error)?,
        transpose,
    ))
}

#[cfg(windows)]
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
    transpose: *const xll_rs::types::XLOPER12,
) -> Result<*mut xll_rs::types::XLOPER12, XllError> {
    let transpose = crate::functions::helpers::optional_bool(transpose, "transpose")?;
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
    Ok(crate::functions::helpers::string_column(
        schedule.dates().iter().map(ToString::to_string).collect(),
        transpose,
    ))
}

#[cfg(windows)]
fn calendar_value_error<Error>(_: Error) -> XllError {
    XllError::new(xll_rs::types::XLLError::VALUE)
}

#[cfg(windows)]
fn optional_calendar_flag(
    value: *const xll_rs::types::XLOPER12,
    argument_name: &str,
) -> Result<Option<bool>, XllError> {
    use xll_rs::types::{XLTYPE_MISSING, XLTYPE_NIL};

    if value.is_null() {
        return Err(calendar_value_error(argument_name));
    }
    if matches!(unsafe { &*value }.base_type(), XLTYPE_MISSING | XLTYPE_NIL) {
        return Ok(None);
    }
    bool_value(value, argument_name)
        .map(Some)
        .map_err(calendar_value_error)
}

mod calendar_operations {
    use quantsupport::prelude::{Calendar, Date, ImplCalendar, IsCalendar};

    fn date(value: &str) -> Result<Date, String> {
        let value = value.trim();
        let date = Date::from_str(value, "%Y-%m-%d").map_err(|error| error.to_string())?;
        if date.to_string() != value || !(1902..=2198).contains(&date.year()) {
            return Err("calendar dates must use YYYY-MM-DD within 1902-01-01..2198-12-31".into());
        }
        Ok(date)
    }

    fn inputs(value: &str, calendar: &str) -> Result<(Date, Calendar), String> {
        Ok((
            date(value)?,
            Calendar::try_from(calendar.to_string()).map_err(|error| error.to_string())?,
        ))
    }

    fn list_inputs(from: &str, to: &str, calendar: &str) -> Result<(Date, Date, Calendar), String> {
        let (from, calendar) = inputs(from, calendar)?;
        let to = date(to)?;
        if from > to {
            return Err("calendar list queries require from <= to".into());
        }
        Ok((from, to, calendar))
    }

    pub fn is_holiday(value: &str, calendar: &str) -> Result<bool, String> {
        let (date, calendar) = inputs(value, calendar)?;
        Ok(calendar.is_holiday(&date))
    }

    pub fn end_of_month(value: &str, calendar: &str) -> Result<String, String> {
        let (date, calendar) = inputs(value, calendar)?;
        Ok(calendar.end_of_month(date).to_string())
    }

    pub fn is_end_of_month(value: &str, calendar: &str) -> Result<bool, String> {
        let (date, calendar) = inputs(value, calendar)?;
        Ok(calendar.is_end_of_month(&date))
    }

    pub fn business_days_between(
        from: &str,
        to: &str,
        calendar: &str,
        include_first: Option<bool>,
        include_last: Option<bool>,
    ) -> Result<i32, String> {
        let (from, calendar) = inputs(from, calendar)?;
        let to = date(to)?;
        i32::try_from(calendar.business_days_between(
            from,
            to,
            include_first.unwrap_or(true),
            include_last.unwrap_or(false),
        ))
        .map_err(|error| error.to_string())
    }

    pub fn holidays(
        from: &str,
        to: &str,
        calendar: &str,
        include_weekends: Option<bool>,
    ) -> Result<Vec<String>, String> {
        let (from, to, calendar) = list_inputs(from, to, calendar)?;
        Ok(calendar
            .holiday_list(from, to, include_weekends.unwrap_or(false))
            .iter()
            .map(ToString::to_string)
            .collect())
    }

    pub fn business_days(from: &str, to: &str, calendar: &str) -> Result<Vec<String>, String> {
        let (from, to, calendar) = list_inputs(from, to, calendar)?;
        Ok(calendar
            .business_day_list(from, to)
            .iter()
            .map(ToString::to_string)
            .collect())
    }
}

#[cfg(test)]
mod calendar_tests {
    use super::calendar_operations::*;

    #[test]
    fn accepts_all_nine_canonical_calendar_variants() {
        for calendar in [
            "NullCalendar",
            "WeekendsOnly",
            "TARGET",
            "UnitedStates(Sofr)",
            "Brazil(Settlement)",
            "Chile(SSE)",
            "UnitedKingdom(Settlement)",
            "Japan",
            "JointCalendar(JoinHolidays,TARGET,Japan)",
        ] {
            assert!(is_holiday("2024-03-29", calendar).is_ok(), "{calendar}");
            assert!(end_of_month("2024-03-15", calendar).is_ok(), "{calendar}");
            assert!(
                is_end_of_month("2024-03-29", calendar).is_ok(),
                "{calendar}"
            );
            assert!(
                business_days_between("2024-03-28", "2024-04-02", calendar, None, None).is_ok(),
                "{calendar}"
            );
            assert!(
                holidays("2024-03-28", "2024-04-02", calendar, None).is_ok(),
                "{calendar}"
            );
            assert!(
                business_days("2024-03-28", "2024-04-02", calendar).is_ok(),
                "{calendar}"
            );
        }
    }

    #[test]
    fn preserves_native_business_month_end_and_holiday_predicates() {
        assert_eq!(end_of_month("2024-03-15", "TARGET").unwrap(), "2024-03-28");
        assert!(is_end_of_month("2024-03-28", "TARGET").unwrap());
        assert!(is_end_of_month("2024-03-29", "TARGET").unwrap());
        assert!(is_end_of_month("2024-03-31", "TARGET").unwrap());
        assert!(!is_end_of_month("2024-03-27", "TARGET").unwrap());
        assert!(is_holiday("2024-03-29", "TARGET").unwrap());
        assert!(!is_holiday("2024-03-28", "TARGET").unwrap());
    }

    #[test]
    fn counting_preserves_signed_bounds_defaults_and_same_day_rules() {
        for (first, last, expected) in [
            (false, false, 0),
            (true, false, 1),
            (false, true, 1),
            (true, true, 2),
        ] {
            assert_eq!(
                business_days_between(
                    "2024-03-28",
                    "2024-04-02",
                    "TARGET",
                    Some(first),
                    Some(last)
                )
                .unwrap(),
                expected
            );
            assert_eq!(
                business_days_between(
                    "2024-04-02",
                    "2024-03-28",
                    "TARGET",
                    Some(last),
                    Some(first)
                )
                .unwrap(),
                -expected
            );
            assert_eq!(
                business_days_between(
                    "2024-03-28",
                    "2024-03-28",
                    "TARGET",
                    Some(first),
                    Some(last)
                )
                .unwrap(),
                i32::from(first && last)
            );
        }
        assert_eq!(
            business_days_between("2024-03-28", "2024-04-02", "TARGET", None, None).unwrap(),
            1
        );
        assert_eq!(
            business_days_between("2024-03-29", "2024-03-29", "TARGET", Some(true), Some(true))
                .unwrap(),
            0
        );
        assert_eq!(
            business_days_between(
                "2024-03-28",
                "2024-04-01",
                "TARGET",
                Some(true),
                Some(false)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            business_days_between(
                "2024-04-01",
                "2024-03-28",
                "TARGET",
                Some(true),
                Some(false)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            business_days_between(
                "2024-04-01",
                "2024-03-28",
                "TARGET",
                Some(false),
                Some(true)
            )
            .unwrap(),
            -1
        );
    }

    #[test]
    fn lists_include_bounds_and_preserve_native_empty_list_quirks() {
        assert_eq!(
            holidays("2024-03-29", "2024-04-01", "TARGET", None).unwrap(),
            ["2024-03-29", "2024-04-01"]
        );
        assert_eq!(
            holidays("2024-03-29", "2024-04-01", "TARGET", Some(true)).unwrap(),
            ["2024-03-29", "2024-03-30", "2024-03-31", "2024-04-01"]
        );
        assert_eq!(
            business_days("2024-03-28", "2024-04-02", "TARGET").unwrap(),
            ["2024-03-28", "2024-04-02"]
        );
        assert_eq!(
            business_days("2024-03-28", "2024-03-28", "TARGET").unwrap(),
            ["2024-03-28"]
        );
        for calendar in ["WeekendsOnly", "NullCalendar"] {
            assert!(holidays("2024-03-28", "2024-04-02", calendar, Some(true))
                .unwrap()
                .is_empty());
            assert!(business_days("2024-03-28", "2024-04-02", calendar)
                .unwrap()
                .is_empty());
        }
        assert!(is_holiday("2024-03-30", "WeekendsOnly").unwrap());
    }

    #[test]
    fn rejects_invalid_dates_calendars_and_reversed_list_ranges() {
        for invalid in [
            "1901-12-31",
            "2199-01-01",
            "0000-01-01",
            "2024-02-30",
            "2024-3-01",
            "not a date",
        ] {
            assert!(is_holiday(invalid, "TARGET").is_err());
            assert!(end_of_month(invalid, "TARGET").is_err());
            assert!(is_end_of_month(invalid, "TARGET").is_err());
            assert!(business_days_between(invalid, "2024-03-28", "TARGET", None, None).is_err());
            assert!(business_days_between("2024-03-28", invalid, "TARGET", None, None).is_err());
            assert!(holidays("2024-03-28", invalid, "TARGET", None).is_err());
            assert!(business_days(invalid, "2024-03-28", "TARGET").is_err());
        }
        for calendar in [
            "Unknown",
            "UnitedStates",
            "Brazil",
            "Chile",
            "UnitedKingdom",
        ] {
            assert!(is_holiday("2024-03-28", calendar).is_err());
        }
        assert!(holidays("2024-04-02", "2024-03-28", "TARGET", None).is_err());
        assert!(business_days("2024-04-02", "2024-03-28", "TARGET").is_err());
    }

    #[test]
    fn accepts_safe_year_edges_without_panicking_on_adjacent_native_dates() {
        for date in ["1902-01-01", "2198-12-31"] {
            assert!(is_holiday(date, "TARGET").is_ok());
            assert!(end_of_month(date, "TARGET").is_ok());
            assert!(is_end_of_month(date, "TARGET").is_ok());
            assert!(holidays(date, date, "TARGET", Some(true)).is_ok());
            assert!(business_days(date, date, "TARGET").is_ok());
        }
        assert!(
            business_days_between("1902-01-01", "2198-12-31", "NullCalendar", None, None).unwrap()
                > 100_000
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn new_calendar_operations_return_excel_error_codes_not_message_cells() {
        use xll_rs::types::{XLERR_VALUE, XLOPER12};

        for error in [
            qs_calendar_is_holiday("1901-12-31", "TARGET").unwrap_err(),
            qs_calendar_end_of_month("bad date", "TARGET").unwrap_err(),
            qs_calendar_is_end_of_month("2024-03-28", "Unknown").unwrap_err(),
            qs_calendar_business_days_between(
                "bad date",
                "2024-03-28",
                "TARGET",
                &XLOPER12::nil(),
                &XLOPER12::nil(),
            )
            .unwrap_err(),
            qs_calendar_holidays(
                "2024-04-02",
                "2024-03-28",
                "TARGET",
                &XLOPER12::nil(),
                &XLOPER12::missing(),
            )
            .unwrap_err(),
            qs_calendar_business_days("2024-04-02", "2024-03-28", "TARGET", &XLOPER12::missing())
                .unwrap_err(),
            optional_calendar_flag(std::ptr::null(), "flag").unwrap_err(),
            optional_calendar_flag(&XLOPER12::from_err(XLERR_VALUE), "flag").unwrap_err(),
        ] {
            assert_eq!(error.code(), XLERR_VALUE);
            assert!(error.message().is_none());
        }
    }

    #[test]
    fn optional_calendar_flags_accept_excel_boolean_numeric_and_blank_cells() {
        use xll_rs::types::{XLERR_VALUE, XLOPER12};

        for (cell, expected) in [
            (XLOPER12::nil(), None),
            (XLOPER12::missing(), None),
            (XLOPER12::from_bool(true), Some(true)),
            (XLOPER12::from_bool(false), Some(false)),
            (XLOPER12::from_int(0), Some(false)),
            (XLOPER12::from_int(-2), Some(true)),
            (XLOPER12::from_f64(0.0), Some(false)),
            (XLOPER12::from_f64(0.5), Some(true)),
        ] {
            assert_eq!(optional_calendar_flag(&cell, "flag").unwrap(), expected);
        }
        assert!(optional_calendar_flag(std::ptr::null(), "flag").is_err());
        assert!(optional_calendar_flag(&XLOPER12::from_err(XLERR_VALUE), "flag").is_err());
        assert_eq!(
            qs_calendar_business_days_between(
                "2024-03-28",
                "2024-04-02",
                "TARGET",
                &XLOPER12::missing(),
                &XLOPER12::nil(),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            qs_calendar_business_days_between(
                "2024-03-28",
                "2024-04-02",
                "TARGET",
                &XLOPER12::from_bool(false),
                &XLOPER12::from_f64(1.0),
            )
            .unwrap(),
            1
        );
    }

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
