//! Numerically stable accumulation of finite scalar values.

use crate::utils::errors::{QSError, Result};

/// Neumaier compensated sum that preserves small terms after large offsets.
#[derive(Default)]
pub struct CompensatedSum {
    value: f64,
    correction: f64,
}

impl CompensatedSum {
    /// Adds a finite term while retaining the floating-point rounding correction.
    ///
    /// # Errors
    /// Rejects nonfinite inputs or overflow without changing the sum.
    pub fn add(&mut self, value: f64) -> Result<()> {
        let next = self.value + value;
        if !value.is_finite() || !next.is_finite() {
            return Err(QSError::InvalidValueErr(
                "Nonfinite aggregation result".into(),
            ));
        }
        let correction = self.correction
            + if self.value.abs() >= value.abs() {
                (self.value - next) + value
            } else {
                (value - next) + self.value
            };
        if !(next + correction).is_finite() {
            return Err(QSError::InvalidValueErr("Aggregation overflow".into()));
        }
        self.value = next;
        self.correction = correction;
        Ok(())
    }

    /// Returns the sum including its rounding correction.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.value + self.correction
    }
}
