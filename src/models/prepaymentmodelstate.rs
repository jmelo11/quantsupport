use crate::{ad::scalar::Scalar, time::date::Date};

/// Loan and market observations available to a conditional prepayment model.
#[derive(Clone, Copy, Debug)]
pub struct PrepaymentModelState<T: Scalar> {
    start: Date,
    end: Date,
    year_fraction: f64,
    outstanding: T,
    client_rate: T,
    refinancing_rate: T,
}

impl<T: Scalar> PrepaymentModelState<T> {
    /// Creates a state after scheduled principal has been paid.
    #[must_use]
    pub const fn new(
        start: Date,
        end: Date,
        year_fraction: f64,
        outstanding: T,
        client_rate: T,
        refinancing_rate: T,
    ) -> Self {
        Self {
            start,
            end,
            year_fraction,
            outstanding,
            client_rate,
            refinancing_rate,
        }
    }

    /// Interval start date.
    #[must_use]
    pub const fn start(&self) -> Date {
        self.start
    }

    /// Interval end date.
    #[must_use]
    pub const fn end(&self) -> Date {
        self.end
    }

    /// Interval length in years for annual CPR conversion.
    #[must_use]
    pub const fn year_fraction(&self) -> f64 {
        self.year_fraction
    }

    /// Balance remaining after scheduled principal.
    #[must_use]
    pub const fn outstanding(&self) -> T {
        self.outstanding
    }

    /// Applicable contractual coupon.
    #[must_use]
    pub const fn client_rate(&self) -> T {
        self.client_rate
    }

    /// Market refinancing rate supplied by the projection engine.
    #[must_use]
    pub const fn refinancing_rate(&self) -> T {
        self.refinancing_rate
    }
}
