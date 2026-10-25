//! Validated access to native International Monetary Market date utilities.

use pyo3::prelude::*;
use quantsupport::time::{date::Date as QsDate, imm::IMM as QsIMM};

use super::Date;
use crate::common::conversion::extract_date;

/// Static IMM date/code utilities. Main-cycle months are March, June, September and December.
#[pyclass(module = "quantsupport", name = "IMM")]
pub struct IMM;

fn valid_code(code: &str, main_cycle: bool) -> bool {
    code.is_ascii() && QsIMM::is_imm_code(code, main_cycle)
}

fn checked_code(code: &str) -> PyResult<()> {
    if !valid_code(code, false) {
        return Err(crate::common::errors::invalid_value(
            "invalid IMM code: expected a month letter and one ASCII year digit",
        ));
    }
    Ok(())
}

fn reference(value: &Bound<'_, PyAny>, resolve_code: bool) -> PyResult<QsDate> {
    let date = extract_date(value)?;
    let maximum = if resolve_code { 9989 } else { 9998 };
    if date == QsDate::empty() || !(1..=maximum).contains(&date.year()) {
        return Err(crate::common::errors::invalid_value(format!(
            "reference_date must be nonempty and have a year in 1..{maximum}"
        )));
    }
    Ok(date)
}

fn checked_next_with_code(
    imm_code: &str,
    reference_date: &Bound<'_, PyAny>,
    main_cycle: bool,
) -> PyResult<QsDate> {
    checked_code(imm_code)?;
    let date = QsIMM::next_date_with_code(imm_code, main_cycle, reference(reference_date, true)?);
    if date.year() > 9999 {
        return Err(crate::common::errors::invalid_value(
            "next IMM date exceeds year 9999",
        ));
    }
    Ok(date)
}

#[pymethods]
impl IMM {
    /// Whether `date` is a third Wednesday; main_cycle restricts to quarterly months.
    #[staticmethod]
    #[pyo3(signature = (date, main_cycle = true))]
    fn is_imm_date(date: &Bound<'_, PyAny>, main_cycle: bool) -> PyResult<bool> {
        Ok(QsIMM::is_imm_date(extract_date(date)?, main_cycle))
    }

    /// Whether `code` is a case-insensitive two-character ASCII IMM code.
    #[staticmethod]
    #[pyo3(signature = (code, main_cycle = true))]
    fn is_imm_code(code: &str, main_cycle: bool) -> bool {
        valid_code(code, main_cycle)
    }

    /// Encodes any monthly IMM date, rejecting non-IMM dates and years outside 1..9999.
    #[staticmethod]
    fn code(imm_date: &Bound<'_, PyAny>) -> PyResult<String> {
        let date = extract_date(imm_date)?;
        if !(1..=9999).contains(&date.year()) || !QsIMM::is_imm_date(date, false) {
            return Err(crate::common::errors::invalid_value(
                "expected an IMM date with year in 1..9999",
            ));
        }
        Ok(QsIMM::code(date))
    }

    /// Resolves a code to the first matching IMM date on or after reference_date.
    #[staticmethod]
    fn date(imm_code: &str, reference_date: &Bound<'_, PyAny>) -> PyResult<Date> {
        checked_code(imm_code)?;
        Ok(Date {
            inner: QsIMM::date(imm_code, reference(reference_date, true)?),
        })
    }

    /// Returns the first IMM date strictly after reference_date.
    #[staticmethod]
    #[pyo3(signature = (reference_date, main_cycle = true))]
    fn next_date(reference_date: &Bound<'_, PyAny>, main_cycle: bool) -> PyResult<Date> {
        Ok(Date {
            inner: QsIMM::next_date(reference(reference_date, false)?, main_cycle),
        })
    }

    /// Returns the next IMM date after resolving imm_code against reference_date.
    #[staticmethod]
    #[pyo3(signature = (imm_code, reference_date, main_cycle = true))]
    fn next_date_with_code(
        imm_code: &str,
        reference_date: &Bound<'_, PyAny>,
        main_cycle: bool,
    ) -> PyResult<Date> {
        Ok(Date {
            inner: checked_next_with_code(imm_code, reference_date, main_cycle)?,
        })
    }

    /// Encodes the first IMM date strictly after reference_date.
    #[staticmethod]
    #[pyo3(signature = (reference_date, main_cycle = true))]
    fn next_code(reference_date: &Bound<'_, PyAny>, main_cycle: bool) -> PyResult<String> {
        Ok(QsIMM::next_code(
            reference(reference_date, false)?,
            main_cycle,
        ))
    }

    /// Encodes the next IMM date after resolving imm_code against reference_date.
    #[staticmethod]
    #[pyo3(signature = (imm_code, reference_date, main_cycle = true))]
    fn next_code_with_code(
        imm_code: &str,
        reference_date: &Bound<'_, PyAny>,
        main_cycle: bool,
    ) -> PyResult<String> {
        let date = checked_next_with_code(imm_code, reference_date, main_cycle)?;
        Ok(QsIMM::code(date))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imm_code_validation_guards_native_utf8_slices() {
        for code in [
            "",
            "H",
            "H26",
            "H\u{e9}",
            "\u{e9}",
            "\u{e9}6",
            "\u{1f600}",
            "A6",
            "H-",
            "H\u{ff16}",
        ] {
            assert!(!valid_code(code, false));
        }
        assert!(valid_code("h6", true));
        assert!(valid_code("F6", false));
        assert!(!valid_code("F6", true));
    }
}

crate::common::representation::impl_class_repr!(IMM);
