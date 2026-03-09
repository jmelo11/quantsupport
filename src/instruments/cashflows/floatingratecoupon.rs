use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::{
    ad::adreal::{ADReal, IsReal},
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::Cashflow, coupons::LinearCoupon},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A [`FloatingRateCoupon`] represents a cash flow from a floating-rate bond or loan,
/// where the interest rate is determined by a market index plus a spread.
///
/// The fixing is stored in an [`RwLock`] so that it can be set by the pricer
/// through a shared (`&self`) reference — the trade itself remains immutable
/// while still satisfying `Send + Sync`.
#[derive(Debug, Serialize, Deserialize)]
pub struct FloatingRateCoupon<T: IsReal> {
    notional: f64,
    spread: T,
    fixing: RwLock<Option<T>>,
    index: MarketIndex,
    start_date: Date,
    end_date: Date,
    payment_date: Date,
    day_counter: DayCounter,
}

impl<T> FloatingRateCoupon<T>
where
    T: IsReal,
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
            .map_or(DayCounter::Actual360, |details| details.rate_definition().day_counter());

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
    ///
    /// Uses interior mutability ([`RwLock`]) so that the pricer can resolve
    /// forward rates without requiring a mutable reference to the trade.
    #[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
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
    #[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
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
impl Cashflow<ADReal> for FloatingRateCoupon<ADReal> {
    fn amount(&self) -> Result<ADReal> {
        let fixing = self
            .fixing
            .read()
            .unwrap()
            .ok_or_else(|| QSError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self
            .day_counter
            .year_fraction(self.start_date, self.end_date);
        Ok(((fixing + self.spread) * year_fraction * self.notional).into())
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

#[allow(clippy::unwrap_used)] // RwLock poisoning is unrecoverable
impl LinearCoupon<ADReal> for FloatingRateCoupon<ADReal> {
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<ADReal> {
        let fixing = self
            .fixing
            .read()
            .unwrap()
            .ok_or_else(|| QSError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self.day_counter.year_fraction(start_date, end_date);
        Ok(((fixing + self.spread) * year_fraction * self.notional).into())
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
