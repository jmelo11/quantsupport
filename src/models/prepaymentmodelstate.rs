use crate::{ad::scalar::Scalar, time::date::Date};

/// Loan and market observations available to a conditional prepayment model.
#[derive(Clone, Copy, Debug)]
pub struct PrepaymentModelState<T: Scalar> {
    interval_start_date: Date,
    interval_end_date: Date,
    interval_year_fraction: f64,
    outstanding_principal: T,
    contractual_coupon_rate: T,
    refinancing_benchmark_rate: T,
}

impl<T: Scalar> PrepaymentModelState<T> {
    /// Creates a state after scheduled principal has been paid.
    ///
    /// `interval_start_date`/`interval_end_date` bound the remaining prepayment
    /// interval, which may begin at the evaluation date for a partially elapsed
    /// coupon. `interval_year_fraction` is its CPR conversion fraction.
    /// `outstanding_principal` includes prior projected
    /// prepayment reductions; both rates are annual decimal observations.
    /// This constructor stores inputs without validation; the model checks them.
    #[must_use]
    pub const fn new(
        interval_start_date: Date,
        interval_end_date: Date,
        interval_year_fraction: f64,
        outstanding_principal: T,
        contractual_coupon_rate: T,
        refinancing_benchmark_rate: T,
    ) -> Self {
        Self {
            interval_start_date,
            interval_end_date,
            interval_year_fraction,
            outstanding_principal,
            contractual_coupon_rate,
            refinancing_benchmark_rate,
        }
    }

    /// Interval start date.
    #[must_use]
    pub const fn interval_start_date(&self) -> Date {
        self.interval_start_date
    }

    /// Interval end date.
    #[must_use]
    pub const fn interval_end_date(&self) -> Date {
        self.interval_end_date
    }

    /// Interval length in years for annual CPR conversion.
    #[must_use]
    pub const fn interval_year_fraction(&self) -> f64 {
        self.interval_year_fraction
    }

    /// Balance remaining after scheduled principal.
    #[must_use]
    pub const fn outstanding_principal(&self) -> T {
        self.outstanding_principal
    }

    /// Applicable contractual coupon.
    #[must_use]
    pub const fn contractual_coupon_rate(&self) -> T {
        self.contractual_coupon_rate
    }

    /// Observed index benchmark used by the refinancing-incentive model.
    ///
    /// Excludes replacement-loan credit spreads, fees, and transaction costs.
    #[must_use]
    pub const fn refinancing_benchmark_rate(&self) -> T {
        self.refinancing_benchmark_rate
    }
}
