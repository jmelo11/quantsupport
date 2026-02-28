use crate::{
    ad::adreal::{ADReal, IsReal},
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::Cashflow, coupons::LinearCoupon},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{AtlasError, Result},
};

/// A [`FloatingRateCoupon`] represents a cash flow from a floating-rate bond or loan,
/// where the interest rate is determined by a market index plus a spread.
pub struct FloatingRateCoupon<T: IsReal> {
    notional: f64,
    spread: T,
    fixing: Option<T>,
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
        Self {
            notional,
            spread,
            fixing: None,
            index,
            start_date,
            end_date,
            payment_date,
            day_counter: DayCounter::Actual360,
        }
    }

    /// Sets the fixing for the coupon. This should be called before calculating the amount.
    pub fn set_fixing(&mut self, fixing: T) {
        self.fixing = Some(fixing);
    }

    /// Returns the market index associated with this floating rate coupon.
    pub fn market_index(&self) -> &MarketIndex {
        &self.index
    }

    /// Returns the spread applied to this coupon.
    pub fn spread(&self) -> T {
        self.spread
    }

    /// Returns the fixing value if set.
    pub fn fixing(&self) -> Option<T> {
        self.fixing
    }

    /// Returns the day counter used for year fraction calculations.
    pub fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the accrual start date.
    pub fn accrual_start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the accrual end date.
    pub fn accrual_end_date(&self) -> Date {
        self.end_date
    }
}

impl Cashflow<ADReal> for FloatingRateCoupon<ADReal> {
    fn amount(&self) -> Result<ADReal> {
        let fixing = self
            .fixing
            .ok_or_else(|| AtlasError::InvalidValueErr("Fixing not set".into()))?;
        let year_fraction = self
            .day_counter
            .year_fraction(self.start_date, self.end_date);
        Ok(((fixing + self.spread) * year_fraction * self.notional).into())
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

impl LinearCoupon<ADReal> for FloatingRateCoupon<ADReal> {
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<ADReal> {
        let fixing = self
            .fixing
            .ok_or_else(|| AtlasError::InvalidValueErr("Fixing not set".into()))?;
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

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}
