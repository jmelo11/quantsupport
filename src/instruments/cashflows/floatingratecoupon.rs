use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::Cashflow, coupons::LinearCoupon},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A [`FloatingRateCoupon`] represents a cash flow from a floating-rate bond or loan,
/// where the interest rate is determined by a market index plus a spread.
#[derive(Debug, Serialize, Deserialize)]
pub struct FloatingRateCoupon<T: Scalar> {
    notional: f64,
    spread: T,
    #[serde(skip)]
    fixing: RwLock<Option<T>>,
    index: MarketIndex,
    start_date: Date,
    end_date: Date,
    payment_date: Date,
    day_counter: DayCounter,
}

#[allow(clippy::unwrap_used)]
impl<T> Clone for FloatingRateCoupon<T>
where
    T: Scalar,
{
    fn clone(&self) -> Self {
        let coupon = Self::new(
            self.notional,
            self.spread,
            self.index.clone(),
            self.start_date,
            self.end_date,
            self.payment_date,
        );
        let value = *self.fixing.read().unwrap();
        if let Some(fixing) = value {
            coupon.set_fixing(fixing);
        }
        coupon
    }
}

impl<T> FloatingRateCoupon<T>
where
    T: Scalar,
{
    /// Creates a new [`FloatingRateCoupon`].
    pub fn new(
        notional: f64,
        spread: T,
        index: MarketIndex,
        start_date: Date,
        end_date: Date,
        payment_date: Date,
    ) -> Self {
        let day_counter = index
            .rate_index_details()
            .map_or(DayCounter::Actual360, |details| {
                details.rate_definition().day_counter()
            });

        Self {
            notional,
            spread,
            fixing: RwLock::new(None),
            index,
            start_date,
            end_date,
            payment_date,
            day_counter,
        }
    }

    /// Sets the fixing for the coupon.
    #[allow(clippy::unwrap_used)]
    pub fn set_fixing(&self, fixing: T) {
        *self.fixing.write().unwrap() = Some(fixing);
    }

    /// Returns the market index associated with this floating rate coupon.
    pub const fn market_index(&self) -> &MarketIndex {
        &self.index
    }

    /// Returns the spread applied to this coupon.
    pub const fn spread(&self) -> T {
        self.spread
    }

    /// Returns the fixing value if set.
    #[allow(clippy::unwrap_used)]
    pub fn fixing(&self) -> Option<T> {
        *self.fixing.read().unwrap()
    }

    /// Returns the day counter used for year fraction calculations.
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the accrual start date.
    pub const fn accrual_start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the accrual end date.
    pub const fn accrual_end_date(&self) -> Date {
        self.end_date
    }
}

#[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
impl Cashflow<f64> for FloatingRateCoupon<f64> {
    fn amount(&self) -> Result<f64> {
        let fixing = self
            .fixing
            .read()
            .unwrap()
            .ok_or_else(|| QSError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self
            .day_counter
            .year_fraction(self.start_date, self.end_date);
        Ok((fixing + self.spread) * (year_fraction * self.notional))
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

#[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
impl LinearCoupon<f64> for FloatingRateCoupon<f64> {
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        let fixing = self
            .fixing()
            .ok_or_else(|| QSError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self.day_counter.year_fraction(start_date, end_date);
        Ok((fixing + self.spread) * (year_fraction * self.notional))
    }

    fn accrual_start_date(&self) -> Date {
        self.start_date
    }

    fn accrual_end_date(&self) -> Date {
        self.end_date
    }

    fn notional(&self) -> f64 {
        self.notional
    }
}

#[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
impl Cashflow<DualFwd> for FloatingRateCoupon<DualFwd> {
    fn amount(&self) -> Result<DualFwd> {
        let fixing = self
            .fixing
            .read()
            .unwrap()
            .ok_or_else(|| QSError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self
            .day_counter
            .year_fraction(self.start_date, self.end_date);
        Ok(((fixing + self.spread) * DualFwd::new(year_fraction * self.notional)).into())
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

#[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
impl LinearCoupon<DualFwd> for FloatingRateCoupon<DualFwd> {
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<DualFwd> {
        let fixing = self
            .fixing
            .read()
            .unwrap()
            .ok_or_else(|| QSError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self.day_counter.year_fraction(start_date, end_date);
        Ok(((fixing + self.spread) * DualFwd::new(year_fraction * self.notional)).into())
    }

    fn accrual_start_date(&self) -> Date {
        self.start_date
    }

    fn accrual_end_date(&self) -> Date {
        self.end_date
    }

    fn notional(&self) -> f64 {
        self.notional
    }
}

impl From<FloatingRateCoupon<f64>> for FloatingRateCoupon<DualFwd> {
    fn from(value: FloatingRateCoupon<f64>) -> Self {
        let fixing = value.fixing();
        let coupon = Self::new(
            value.notional,
            DualFwd::new(value.spread.value()),
            value.index,
            value.start_date,
            value.end_date,
            value.payment_date,
        );
        if let Some(fixing) = fixing {
            coupon.set_fixing(DualFwd::new(fixing.value()));
        }
        coupon
    }
}

#[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
impl From<FloatingRateCoupon<DualFwd>> for FloatingRateCoupon<f64> {
    fn from(value: FloatingRateCoupon<DualFwd>) -> Self {
        let fixing = value.fixing();
        let coupon = Self::new(
            value.notional,
            value.spread.value(),
            value.index,
            value.start_date,
            value.end_date,
            value.payment_date,
        );
        if let Some(fixing) = fixing {
            coupon.set_fixing(fixing.value());
        }
        coupon
    }
}
