//! Date, period and calendar functionality, wrapping the library's
//! `time` module.

pub(crate) mod datetime;
pub(crate) mod imm;
pub(crate) mod schedule;
pub(crate) use datetime::DateTime;

use pyo3::prelude::*;
use pyo3::types::PyDate;
use pyo3::IntoPyObjectExt;
use quantsupport::prelude::{
    Calendar as QsCalendar, Date as QsDate, ImplCalendar, IsCalendar, Period as QsPeriod,
};

use quantsupport::time::calendars::{
    brazil::Brazil, chile::Chile, japan::Japan, jointcalendar::JointCalendar,
    nullcalendar::NullCalendar, target::TARGET, unitedkingdom::UnitedKingdom,
    unitedstates::UnitedStates, weekendsonly::WeekendsOnly,
};

use crate::common::conversion::{
    extract_business_day_convention, extract_date, extract_period, extract_time_unit, qs_err,
};
use crate::common::enums::{
    BrazilMarket, Frequency, JointCalendarRule, TimeUnit, UnitedStatesMarket,
};
use crate::QuantSupportError;

/// A calendar date.
///
/// Wherever the bindings expect a date, a [`Date`], an ISO string
/// (`"2025-11-11"`) or a `datetime.date` may be passed.
#[pyclass(
    module = "quantsupport",
    name = "Date",
    eq,
    ord,
    frozen,
    hash,
    from_py_object
)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Date {
    pub(crate) inner: QsDate,
}

#[pymethods]
impl Date {
    #[new]
    fn new(year: i32, month: u32, day: u32) -> PyResult<Self> {
        // Goes through the string parser to avoid the library's panic on
        // invalid dates.
        let inner = QsDate::from_str(&format!("{year:04}-{month:02}-{day:02}"), "%Y-%m-%d")
            .map_err(|_| {
                QuantSupportError::new_err(format!("invalid date: {year}-{month}-{day}"))
            })?;
        Ok(Self { inner })
    }

    /// Parses a date string; `fmt` follows chrono's `strftime` syntax.
    #[staticmethod]
    #[pyo3(signature = (s, fmt = "%Y-%m-%d"))]
    fn parse(s: &str, fmt: &str) -> PyResult<Self> {
        Ok(Self {
            inner: QsDate::from_str(s, fmt).map_err(qs_err)?,
        })
    }

    #[getter]
    fn year(&self) -> i32 {
        self.inner.year()
    }

    #[getter]
    fn month(&self) -> u32 {
        self.inner.month()
    }

    #[getter]
    fn day(&self) -> u32 {
        self.inner.day()
    }

    /// Day of the week (e.g. `"Monday"`).
    fn weekday(&self) -> String {
        format!("{:?}", self.inner.weekday())
    }

    /// Number of days in this date's month.
    fn days_in_month(&self) -> i32 {
        self.inner.days_in_month()
    }

    /// Day of the year (1-based).
    fn day_of_year(&self) -> i32 {
        self.inner.day_of_year()
    }

    /// Whether the date falls in a leap year.
    fn is_leap_year(&self) -> bool {
        self.inner.date_has_leap_year()
    }

    /// Last day of this date's month.
    fn end_of_month(&self) -> Self {
        Self {
            inner: QsDate::end_of_month(self.inner),
        }
    }

    /// Advances the date by `n` units (`TimeUnit` or string).
    fn advance(&self, n: i32, units: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.advance(n, extract_time_unit(units)?),
        })
    }

    /// Adds a [`Period`] (or period string like `"6M"`).
    fn add_period(&self, period: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.add_period(extract_period(period)?),
        })
    }

    /// Formats the date; `fmt` follows chrono's `strftime` syntax.
    #[pyo3(signature = (fmt = "%Y-%m-%d"))]
    #[allow(clippy::wrong_self_convention)]
    fn to_str(&self, fmt: &str) -> String {
        self.inner.to_str(fmt)
    }

    /// Converts to a Python `datetime.date`.
    #[allow(clippy::wrong_self_convention)]
    fn to_datetime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDate>> {
        PyDate::new(
            py,
            self.inner.year(),
            u8::try_from(self.inner.month()).unwrap_or(1),
            u8::try_from(self.inner.day()).unwrap_or(1),
        )
    }

    /// `date + n` adds days; `date + Period` (or `"6M"`) adds a period.
    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(days) = other.extract::<i64>() {
            return Ok(Self {
                inner: self.inner + days,
            });
        }
        Ok(Self {
            inner: self.inner + extract_period(other)?,
        })
    }

    /// `date - date` returns days; `date - n` / `date - Period` returns a date.
    fn __sub__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(d) = other.extract::<Self>() {
            return (self.inner - d.inner).into_py_any(py);
        }
        if let Ok(days) = other.extract::<i64>() {
            return Self {
                inner: self.inner - days,
            }
            .into_py_any(py);
        }
        Self {
            inner: self.inner - extract_period(other)?,
        }
        .into_py_any(py)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Date('{}')", self.inner)
    }
}

/// A financial period such as `5Y`, `6M` or `1Y6M`.
///
/// Wherever the bindings expect a period, a [`Period`] or its string form
/// may be passed.
#[pyclass(module = "quantsupport", name = "Period", eq, frozen, from_py_object)]
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Period {
    pub(crate) inner: QsPeriod,
}

#[pymethods]
impl Period {
    #[new]
    fn new(length: i32, units: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: QsPeriod::new(length, extract_time_unit(units)?),
        })
    }

    /// Parses a period string such as `"5Y"`, `"6M"` or `"1Y6M"`.
    #[staticmethod]
    fn parse(s: &str) -> PyResult<Self> {
        crate::common::conversion::parse_str_enum::<QsPeriod>(s, "period")
            .map(|inner| Self { inner })
    }

    /// Builds the period equivalent to a [`Frequency`].
    #[staticmethod]
    fn from_frequency(freq: Frequency) -> PyResult<Self> {
        QsPeriod::from_frequency(freq.into())
            .map(|inner| Self { inner })
            .ok_or_else(|| {
                QuantSupportError::new_err("frequency has no period equivalent".to_string())
            })
    }

    #[getter]
    fn length(&self) -> i32 {
        self.inner.length()
    }

    #[getter]
    fn units(&self) -> TimeUnit {
        self.inner.units().into()
    }

    /// The [`Frequency`] equivalent of this period.
    fn frequency(&self) -> Frequency {
        self.inner.frequency().into()
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Period('{}')", self.inner)
    }
}

/// A business-day calendar.
///
/// Native calendar variants, for example ``Calendar.TARGET``.
///
/// Fixed calendars are typed class attributes. UnitedStates and Brazil accept
/// typed market enums; JointCalendar combines typed calendars and a rule.
/// The string constructor remains available for loading serialized names.
#[pyclass(module = "quantsupport", name = "Calendar", from_py_object)]
#[derive(Clone)]
pub(crate) struct Calendar {
    pub(crate) inner: QsCalendar,
}

#[pymethods]
#[allow(non_snake_case)]
impl Calendar {
    #[classattr]
    fn NullCalendar() -> Self {
        Self {
            inner: QsCalendar::NullCalendar(NullCalendar::new()),
        }
    }
    #[classattr]
    fn WeekendsOnly() -> Self {
        Self {
            inner: QsCalendar::WeekendsOnly(WeekendsOnly::new()),
        }
    }
    #[classattr]
    fn TARGET() -> Self {
        Self {
            inner: QsCalendar::TARGET(TARGET::new()),
        }
    }
    #[classattr]
    fn UnitedKingdom() -> Self {
        Self {
            inner: QsCalendar::UnitedKingdom(UnitedKingdom::new()),
        }
    }
    #[classattr]
    fn Japan() -> Self {
        Self {
            inner: QsCalendar::Japan(Japan::new()),
        }
    }
    #[classattr]
    fn Chile() -> Self {
        Self {
            inner: QsCalendar::Chile(Chile::default()),
        }
    }

    /// United States calendar for the selected market.
    ///
    /// Parameters
    /// ----------
    /// market : UnitedStatesMarket, optional
    ///     Defaults to Sofr.
    #[staticmethod]
    #[pyo3(signature = (*, market = None))]
    fn UnitedStates(market: Option<UnitedStatesMarket>) -> Self {
        Self {
            inner: QsCalendar::UnitedStates(UnitedStates::new(
                market.unwrap_or(UnitedStatesMarket::Sofr).into(),
            )),
        }
    }
    /// Brazil calendar for the selected market.
    ///
    /// Parameters
    /// ----------
    /// market : BrazilMarket, optional
    ///     Defaults to Settlement.
    #[staticmethod]
    #[pyo3(signature = (*, market = None))]
    fn Brazil(market: Option<BrazilMarket>) -> Self {
        Self {
            inner: QsCalendar::Brazil(Brazil::new(
                market.unwrap_or(BrazilMarket::Settlement).into(),
            )),
        }
    }
    /// Combine native calendars.
    ///
    /// Parameters
    /// ----------
    /// calendars : sequence of Calendar
    ///     At least two constituent calendars.
    /// rule : JointCalendarRule, optional
    ///     Defaults to JoinHolidays: every constituent must be open.
    #[staticmethod]
    #[pyo3(signature = (*, calendars, rule = None))]
    fn JointCalendar(calendars: Vec<Self>, rule: Option<JointCalendarRule>) -> PyResult<Self> {
        Ok(Self {
            inner: QsCalendar::JointCalendar(
                JointCalendar::new(
                    calendars
                        .into_iter()
                        .map(|calendar| calendar.inner)
                        .collect(),
                    rule.unwrap_or(JointCalendarRule::JoinHolidays).into(),
                )
                .map_err(qs_err)?,
            ),
        })
    }

    #[new]
    fn new(name: &str) -> PyResult<Self> {
        Ok(Self {
            inner: QsCalendar::try_from(name.to_string()).map_err(qs_err)?,
        })
    }

    /// Calendar name.
    fn name(&self) -> String {
        self.inner.name()
    }

    /// Whether the date is a business day.
    fn is_business_day(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_business_day(&extract_date(date)?))
    }

    /// Whether the date is a holiday (including weekends).
    fn is_holiday(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_holiday(&extract_date(date)?))
    }

    /// Returns a separate calendar with `date` marked as a holiday.
    fn with_added_holiday(&self, date: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut calendar = self.clone();
        calendar.inner.add_holiday(calendar_date(date)?);
        Ok(calendar)
    }

    /// Returns a separate calendar with a native removed-holiday override.
    /// Added holidays take precedence if both override sets contain the date.
    fn with_removed_holiday(&self, date: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut calendar = self.clone();
        calendar.inner.remove_holiday(calendar_date(date)?);
        Ok(calendar)
    }

    /// A sorted copy of explicitly added holidays.
    #[getter]
    fn added_holidays(&self) -> Vec<Date> {
        let mut dates: Vec<_> = self.inner.added_holidays().into_iter().collect();
        dates.sort_unstable();
        dates.into_iter().map(|inner| Date { inner }).collect()
    }

    /// A sorted copy of explicitly removed holidays.
    #[getter]
    fn removed_holidays(&self) -> Vec<Date> {
        let mut dates: Vec<_> = self.inner.removed_holidays().into_iter().collect();
        dates.sort_unstable();
        dates.into_iter().map(|inner| Date { inner }).collect()
    }

    /// Last business day in the month containing `date`, using Preceding.
    fn end_of_month(&self, date: &Bound<'_, PyAny>) -> PyResult<Date> {
        Ok(Date {
            inner: self.inner.end_of_month(calendar_date(date)?),
        })
    }

    /// Whether following adjustment of the next day crosses a month boundary.
    fn is_end_of_month(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_end_of_month(&calendar_date(date)?))
    }

    /// Whether the weekday of `date` is a weekend under this calendar's rule.
    fn is_weekend(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_weekend(&extract_date(date)?.weekday()))
    }

    /// Adjusts a date to a business day under the given convention
    /// (default `Following`).
    #[pyo3(signature = (date, convention = None))]
    fn adjust(
        &self,
        date: &Bound<'_, PyAny>,
        convention: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Date> {
        let conv = convention
            .map(extract_business_day_convention)
            .transpose()?;
        Ok(Date {
            inner: self.inner.adjust(extract_date(date)?, conv),
        })
    }

    /// Advances a date by a period, adjusting for business days.
    #[pyo3(signature = (date, period, convention = None, end_of_month = false))]
    fn advance(
        &self,
        date: &Bound<'_, PyAny>,
        period: &Bound<'_, PyAny>,
        convention: Option<&Bound<'_, PyAny>>,
        end_of_month: bool,
    ) -> PyResult<Date> {
        let conv = convention
            .map(extract_business_day_convention)
            .transpose()?;
        Ok(Date {
            inner: self.inner.advance(
                extract_date(date)?,
                extract_period(period)?,
                conv,
                end_of_month,
            ),
        })
    }

    /// Number of business days between two dates.
    #[pyo3(signature = (from, to, include_first = true, include_last = false))]
    fn business_days_between(
        &self,
        from: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
        include_first: bool,
        include_last: bool,
    ) -> PyResult<i64> {
        Ok(self.inner.business_days_between(
            extract_date(from)?,
            extract_date(to)?,
            include_first,
            include_last,
        ))
    }

    /// Holidays between two dates.
    #[pyo3(signature = (from, to, include_weekends = false))]
    fn holiday_list(
        &self,
        from: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
        include_weekends: bool,
    ) -> PyResult<Vec<Date>> {
        Ok(self
            .inner
            .holiday_list(extract_date(from)?, extract_date(to)?, include_weekends)
            .into_iter()
            .map(|inner| Date { inner })
            .collect())
    }

    /// Business days between two dates.
    fn business_day_list(
        &self,
        from: &Bound<'_, PyAny>,
        to: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<Date>> {
        Ok(self
            .inner
            .business_day_list(extract_date(from)?, extract_date(to)?)
            .into_iter()
            .map(|inner| Date { inner })
            .collect())
    }

    fn __repr__(&self) -> String {
        format!("Calendar('{}')", self.inner.name())
    }
}

fn calendar_date(value: &Bound<'_, PyAny>) -> PyResult<QsDate> {
    let date = extract_date(value)?;
    if !(1902..=2198).contains(&date.year()) {
        return Err(QuantSupportError::new_err(
            "calendar dates must be in 1902..2198",
        ));
    }
    Ok(date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::PyDict;
    use quantsupport::time::calendars::{
        brazil::Market as BrMarket, unitedstates::Market as UsMarket,
    };

    #[test]
    fn native_calendar_variant_parity() -> PyResult<()> {
        Python::initialize();
        Python::attach(|py| {
            let class = py.get_type::<Calendar>();
            let calendars = [
                QsCalendar::NullCalendar(NullCalendar::new()),
                QsCalendar::WeekendsOnly(WeekendsOnly::new()),
                QsCalendar::TARGET(TARGET::new()),
                QsCalendar::UnitedKingdom(UnitedKingdom::new()),
                QsCalendar::Japan(Japan::new()),
                QsCalendar::Chile(Chile::default()),
                QsCalendar::UnitedStates(UnitedStates::default()),
                QsCalendar::Brazil(Brazil::default()),
                QsCalendar::JointCalendar(JointCalendar::new(
                    vec![Calendar::TARGET().inner, Calendar::Japan().inner],
                    quantsupport::time::calendars::jointcalendar::JointCalendarRule::JoinHolidays,
                ).map_err(qs_err)?),
            ];
            for native in calendars {
                let wrapped = match &native {
                    QsCalendar::NullCalendar(_) => class.getattr("NullCalendar")?,
                    QsCalendar::WeekendsOnly(_) => class.getattr("WeekendsOnly")?,
                    QsCalendar::TARGET(_) => class.getattr("TARGET")?,
                    QsCalendar::UnitedKingdom(_) => class.getattr("UnitedKingdom")?,
                    QsCalendar::Japan(_) => class.getattr("Japan")?,
                    QsCalendar::Chile(_) => class.getattr("Chile")?,
                    QsCalendar::UnitedStates(_) => class.call_method0("UnitedStates")?,
                    QsCalendar::Brazil(_) => class.call_method0("Brazil")?,
                    QsCalendar::JointCalendar(_) => {
                        let kwargs = PyDict::new(py);
                        kwargs
                            .set_item("calendars", vec![Calendar::TARGET(), Calendar::Japan()])?;
                        class.call_method("JointCalendar", (), Some(&kwargs))?
                    }
                }
                .extract::<Calendar>()?;
                assert_eq!(wrapped.inner, native);
            }
            for market in [
                UsMarket::Settlement,
                UsMarket::LiborImpact,
                UsMarket::Nyse,
                UsMarket::GovernmentBond,
                UsMarket::Nerc,
                UsMarket::FederalReserve,
                UsMarket::Sofr,
            ] {
                let selector = py
                    .get_type::<UnitedStatesMarket>()
                    .getattr(format!("{market:?}"))?;
                let kwargs = PyDict::new(py);
                kwargs.set_item("market", selector)?;
                let wrapped = class
                    .call_method("UnitedStates", (), Some(&kwargs))?
                    .extract::<Calendar>()?;
                assert_eq!(
                    wrapped.inner,
                    QsCalendar::UnitedStates(UnitedStates::new(market))
                );
            }
            for market in [BrMarket::Settlement, BrMarket::Exchange] {
                let selector = py
                    .get_type::<BrazilMarket>()
                    .getattr(format!("{market:?}"))?;
                let kwargs = PyDict::new(py);
                kwargs.set_item("market", selector)?;
                let wrapped = class
                    .call_method("Brazil", (), Some(&kwargs))?
                    .extract::<Calendar>()?;
                assert_eq!(wrapped.inner, QsCalendar::Brazil(Brazil::new(market)));
            }
            Ok(())
        })
    }
}
