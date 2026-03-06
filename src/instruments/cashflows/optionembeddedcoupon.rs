use crate::{
    ad::adreal::{ADReal, IsReal},
    indices::{marketindex::MarketIndex, rateindex::RateIndexDetails},
    instruments::cashflows::coupons::{NonLinearCoupon, PayoffOps},
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// An [`OptionEmbeddedCoupon`] represents a cash flow that includes embedded options,
/// where the payoff is determined by a specified payoff function. For example, this could represent a
/// floored or capped coupon, where the payoff is determined by the underlying index and the specified floor or cap.
pub struct OptionEmbeddedCoupon<T: IsReal> {
    notional: f64,
    fixing: Option<T>,
    spread: ADReal,
    index: MarketIndex,
    accrual_start_date: Date,
    accrual_end_date: Date,
    payment_date: Date,
    payoff: PayoffOps,
}

impl<T: IsReal> OptionEmbeddedCoupon<T> {
    /// Creates a new [`OptionEmbeddedCoupon`].
    pub fn new(
        notional: f64,
        index: MarketIndex,
        spread: ADReal,
        accrual_start_date: Date,
        accrual_end_date: Date,
        payment_date: Date,
        payoff: PayoffOps,
    ) -> Self {
        Self {
            notional,
            fixing: None,
            spread: spread,
            index,
            accrual_start_date,
            accrual_end_date,
            payment_date,
            payoff,
        }
    }

    /// Sets the fixing for the coupon, which is used to determine the payoff.
    pub fn with_fixing(mut self, fixing: T) -> Self {
        self.fixing = Some(fixing);
        self
    }

    /// Returns the spread applied to this coupon.
    pub fn spread(&self) -> ADReal {
        self.spread
    }

    /// Returns the fixing value if set.
    pub fn fixing(&self) -> Option<T> {
        self.fixing
    }

    /// Returns the market index associated with this coupon.
    pub fn market_index(&self) -> &MarketIndex {
        &self.index
    }

    /// Returns the payoff operations.
    pub fn payoff_ops(&self) -> &PayoffOps {
        &self.payoff
    }
}

impl OptionEmbeddedCoupon<ADReal> {
    /// Returns the amount of the coupon (calculated from accrued amount). Only available for ADReal.
    pub fn amount(&self) -> Result<ADReal> {
        self.accrued_amount(self.accrual_start_date, self.accrual_end_date)
    }
}

impl NonLinearCoupon<ADReal> for OptionEmbeddedCoupon<ADReal> {
    fn accrual_end_date(&self) -> Date {
        self.accrual_end_date
    }

    fn accrual_start_date(&self) -> Date {
        self.accrual_start_date
    }

    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<ADReal> {
        let fixing = self
            .fixing
            .ok_or_else(|| QSError::NotFoundErr("Fixing not set".into()))?;

        let year_fraction = self
            .index
            .rate_index_details()?
            .rate_definition()
            .day_counter()
            .year_fraction(start_date, end_date);

        let resuling_rate = self.payoff.evaluate(fixing)?;
        Ok(((self.spread + resuling_rate) * year_fraction * self.notional).into())
    }

    fn notional(&self) -> f64 {
        self.notional
    }

    fn payoff_description(&self) -> PayoffOps {
        self.payoff.clone()
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}
