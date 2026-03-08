use serde::{Deserialize, Serialize};

use crate::{
    ad::adreal::{ADReal, IsReal},
    instruments::cashflows::{cashflow::Cashflow, coupons::LinearCoupon},
    rates::interestrate::InterestRate,
    time::date::Date,
    utils::errors::Result,
};

/// A [`FixedRateCoupon`] represents a cash flow from a fixed-rate bond or loan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedRateCoupon<T: IsReal> {
    notional: f64,
    rate: Box<InterestRate<T>>,
    accrual_start_date: Date,
    accrual_end_date: Date,
    payment_date: Date,
}

impl<T: IsReal> FixedRateCoupon<T> {
    /// Creates a new [`FixedRateCoupon`].
    pub fn new(
        notional: f64,
        rate: Box<InterestRate<T>>,
        accrual_start_date: Date,
        accrual_end_date: Date,
        payment_date: Date,
    ) -> Self {
        Self {
            notional,
            rate,
            accrual_start_date,
            accrual_end_date,
            payment_date,
        }
    }

    /// Returns the interest rate associated with this coupon.
    pub fn rate(&self) -> &InterestRate<T> {
        &self.rate
    }

    /// Returns the accrual start date.
    pub fn accrual_start_date(&self) -> Date {
        self.accrual_start_date
    }

    /// Returns the accrual end date.
    pub fn accrual_end_date(&self) -> Date {
        self.accrual_end_date
    }

    /// Returns the notional amount.
    pub fn notional(&self) -> f64 {
        self.notional
    }
}

impl Cashflow<ADReal> for FixedRateCoupon<ADReal> {
    fn amount(&self) -> Result<ADReal> {
        let year_fraction = self
            .rate
            .day_counter()
            .year_fraction(self.accrual_start_date, self.accrual_end_date);
        Ok((self.rate.rate() * year_fraction * self.notional).into())
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

impl LinearCoupon<ADReal> for FixedRateCoupon<ADReal> {
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<ADReal> {
        let year_fraction = self.rate.day_counter().year_fraction(start_date, end_date);
        Ok((self.rate.rate() * year_fraction * self.notional).into())
    }

    fn accrual_start_date(&self) -> Date {
        self.accrual_start_date
    }

    fn accrual_end_date(&self) -> Date {
        self.accrual_end_date
    }

    fn notional(&self) -> f64 {
        self.notional
    }
}
