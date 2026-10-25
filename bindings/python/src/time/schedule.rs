//! Immutable wrappers for native schedule generation.

use pyo3::{exceptions::PyIndexError, prelude::*, types::PyList};
use quantsupport::time::{
    date::Date as QsDate,
    enums::TimeUnit,
    period::Period as QsPeriod,
    schedule::{MakeSchedule, Schedule as QsSchedule},
};

use super::{calendar_date, Calendar, Date, Period};
use crate::common::{
    conversion::{extract_period, qs_err},
    enums::{BusinessDayConvention, DateGenerationRule, Frequency},
};

/// A native date schedule with immutable parameters and copied sequence views.
#[pyclass(module = "quantsupport", name = "Schedule", frozen, from_py_object)]
#[derive(Clone)]
pub struct Schedule {
    inner: QsSchedule,
}

#[pymethods]
impl Schedule {
    /// Generates dates with the native MakeSchedule builder.
    ///
    /// Supply at most one of tenor and frequency. Defaults are a zero tenor,
    /// NullCalendar, Unadjusted conventions, Backward generation and no EOM.
    /// Dates must be ordered and within 1902..2198; the tenor's trial dates
    /// must remain within the native calendar range 1901..2199.
    #[new]
    #[pyo3(signature = (effective_date, termination_date, tenor = None, frequency = None, calendar = None, convention = None, termination_date_convention = None, rule = None, end_of_month = false, first_date = None, next_to_last_date = None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        effective_date: &Bound<'_, PyAny>,
        termination_date: &Bound<'_, PyAny>,
        tenor: Option<&Bound<'_, PyAny>>,
        frequency: Option<Frequency>,
        calendar: Option<Calendar>,
        convention: Option<BusinessDayConvention>,
        termination_date_convention: Option<BusinessDayConvention>,
        rule: Option<DateGenerationRule>,
        end_of_month: bool,
        first_date: Option<&Bound<'_, PyAny>>,
        next_to_last_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let effective = calendar_date(effective_date)?;
        let termination = calendar_date(termination_date)?;
        if effective >= termination {
            return Err(crate::common::errors::invalid_value(
                "effective_date must precede termination_date",
            ));
        }
        if tenor.is_some() && frequency.is_some() {
            return Err(crate::common::errors::invalid_value(
                "supply tenor or frequency, not both",
            ));
        }
        let tenor = if let Some(value) = tenor {
            extract_period(value)?
        } else if let Some(value) = frequency {
            QsPeriod::from_frequency(value.into()).ok_or_else(|| {
                crate::common::errors::invalid_value("frequency has no period equivalent")
            })?
        } else {
            QsPeriod::empty()
        };
        validate_tenor(effective, termination, tenor)?;
        let rule = rule.unwrap_or(DateGenerationRule::Backward);
        if end_of_month && (tenor.length() == 0 || rule == DateGenerationRule::Zero) {
            return Err(crate::common::errors::invalid_value(
                "end_of_month requires a nonzero, non-Zero schedule",
            ));
        }
        let first = first_date.map(calendar_date).transpose()?;
        let penultimate = next_to_last_date.map(calendar_date).transpose()?;
        for date in [first, penultimate].into_iter().flatten() {
            if date <= effective || date >= termination {
                return Err(crate::common::errors::invalid_value(
                    "stub dates must be strictly inside the schedule range",
                ));
            }
        }
        if first
            .zip(penultimate)
            .is_some_and(|(first, last)| first >= last)
        {
            return Err(crate::common::errors::invalid_value(
                "first_date must precede next_to_last_date",
            ));
        }
        let mut builder = MakeSchedule::new(effective, termination)
            .with_tenor(tenor)
            .with_rule(rule.into())
            .end_of_month(end_of_month);
        if let Some(calendar) = calendar {
            builder = builder.with_calendar(calendar.inner);
        }
        if let Some(value) = convention {
            builder = builder.with_convention(value.into());
        }
        if let Some(value) = termination_date_convention {
            builder = builder.with_termination_date_convention(value.into());
        }
        if let Some(date) = first {
            builder = builder.with_first_date(date);
        }
        if let Some(date) = penultimate {
            builder = builder.with_next_to_last_date(date);
        }
        let inner = std::panic::catch_unwind(|| builder.build())
            .map_err(|_| {
                crate::common::errors::invalid_value(
                    "native schedule generation failed; dates may collapse under adjustment",
                )
            })?
            .map_err(qs_err)?;
        if inner.dates().len() < 2 || inner.dates().windows(2).any(|dates| dates[0] >= dates[1]) {
            return Err(crate::common::errors::invalid_value(
                "schedule must contain at least two increasing dates after adjustment",
            ));
        }
        Ok(Self { inner })
    }

    /// A fresh list of generated, business-day-adjusted dates.
    #[getter]
    fn dates(&self) -> Vec<Date> {
        self.inner
            .dates()
            .iter()
            .map(|&inner| Date { inner })
            .collect()
    }

    /// A fresh list of native regular-period flags, one per date interval.
    #[getter]
    fn is_regular(&self) -> Vec<bool> {
        self.inner.is_regular().clone()
    }

    /// Effective tenor; zero schedules normalize to zero years.
    #[getter]
    fn tenor(&self) -> Period {
        Period {
            inner: self.inner.tenor(),
        }
    }

    /// An independent copy of the adjustment calendar, including overrides.
    #[getter]
    fn calendar(&self) -> Calendar {
        Calendar {
            inner: self.inner.calendar().clone(),
        }
    }

    /// Convention used for the effective and intermediate dates.
    #[getter]
    fn convention(&self) -> BusinessDayConvention {
        self.inner.convention().into()
    }

    /// Separately configured convention for the termination date.
    #[getter]
    fn termination_date_convention(&self) -> BusinessDayConvention {
        self.inner.termination_date_convention().into()
    }

    /// Effective generation rule; a zero tenor forces Zero.
    #[getter]
    fn rule(&self) -> DateGenerationRule {
        self.inner.rule().into()
    }

    /// Whether end-of-month generation was requested.
    #[getter]
    fn end_of_month(&self) -> bool {
        self.inner.end_of_month()
    }

    /// Explicit first stub anchor, or None when unspecified.
    #[getter]
    fn first_date(&self) -> Option<Date> {
        (self.inner.first_date() != QsDate::empty()).then(|| Date {
            inner: self.inner.first_date(),
        })
    }

    /// Explicit final stub anchor, or None when unspecified.
    #[getter]
    fn next_to_last_date(&self) -> Option<Date> {
        (self.inner.next_to_last_date() != QsDate::empty()).then(|| Date {
            inner: self.inner.next_to_last_date(),
        })
    }

    /// Number of generated dates, including both endpoints.
    fn __len__(&self) -> usize {
        self.inner.dates().len()
    }

    /// Returns a date by integer index; negative indices count from the end.
    fn __getitem__(&self, index: isize) -> PyResult<Date> {
        let length = isize::try_from(self.inner.dates().len()).map_err(qs_err)?;
        let index = if index < 0 { index + length } else { index };
        usize::try_from(index)
            .ok()
            .and_then(|index| self.inner.dates().get(index))
            .map(|&inner| Date { inner })
            .ok_or_else(|| PyIndexError::new_err("schedule index out of range"))
    }

    /// Iterates over an independent list of dates.
    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyList::new(py, self.dates())?
            .call_method0("__iter__")?
            .unbind())
    }
}

fn validate_tenor(effective: QsDate, termination: QsDate, tenor: QsPeriod) -> PyResult<()> {
    if tenor.length() < 0 {
        return Err(crate::common::errors::invalid_value(
            "tenor must be nonnegative",
        ));
    }
    let length = i64::from(tenor.length());
    let years = match tenor.units() {
        TimeUnit::Days => (length + 364) / 365,
        TimeUnit::Weeks => (length * 7 + 364) / 365,
        TimeUnit::Months => (length + 11) / 12,
        TimeUnit::Years => length,
    };
    if i64::from(effective.year()) - years < 1901 || i64::from(termination.year()) + years > 2199 {
        return Err(crate::common::errors::invalid_value(
            "tenor trial dates exceed the native calendar range 1901..2199",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_tenor_bounds() {
        let effective = QsDate::new(2026, 1, 1);
        let termination = QsDate::new(2027, 1, 1);
        assert!(validate_tenor(effective, termination, QsPeriod::new(3, TimeUnit::Months)).is_ok());
        assert!(validate_tenor(
            effective,
            termination,
            QsPeriod::new(i32::MAX, TimeUnit::Years)
        )
        .is_err());
        assert!(validate_tenor(effective, termination, QsPeriod::new(-1, TimeUnit::Days)).is_err());
    }
}

crate::common::representation::impl_class_repr!(Schedule);
