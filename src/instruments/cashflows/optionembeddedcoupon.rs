use crate::{
    ad::adreal::{ADReal, IsReal},
    indices::marketindex::MarketIndex,
    instruments::cashflows::coupons::{NonLinearCoupon, PayoffOps},
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// An [`OptionEmbeddedCoupon`] represents a cash flow that includes embedded options.
///
/// The payoff is determined by a specified payoff function. For example, this could
/// represent a floored or capped coupon, where the payoff is determined by the
/// underlying index and the specified floor or cap.
pub struct OptionEmbeddedCoupon<T: IsReal> {
    notional: f64,
    fixing: Option<T>,
    spread: T,
    index: MarketIndex,
    accrual_start_date: Date,
    accrual_end_date: Date,
    payment_date: Date,
    payoff: PayoffOps,
}

impl<T: IsReal> OptionEmbeddedCoupon<T> {
    /// Creates a new [`OptionEmbeddedCoupon`].
    #[must_use]
    pub const fn new(
        notional: f64,
        index: MarketIndex,
        spread: T,
        accrual_start_date: Date,
        accrual_end_date: Date,
        payment_date: Date,
        payoff: PayoffOps,
    ) -> Self {
        Self {
            notional,
            fixing: None,
            spread,
            index,
            accrual_start_date,
            accrual_end_date,
            payment_date,
            payoff,
        }
    }

    /// Sets the fixing for the coupon, which is used to determine the payoff.
    #[must_use]
    pub const fn with_fixing(mut self, fixing: T) -> Self {
        self.fixing = Some(fixing);
        self
    }

    /// Returns the spread applied to this coupon.
    pub const fn spread(&self) -> T {
        self.spread
    }

    /// Returns the fixing value if set.
    pub const fn fixing(&self) -> Option<T> {
        self.fixing
    }

    /// Returns the market index associated with this coupon.
    pub const fn market_index(&self) -> &MarketIndex {
        &self.index
    }

    /// Returns the payoff operations.
    pub const fn payoff_ops(&self) -> &PayoffOps {
        &self.payoff
    }
}

impl OptionEmbeddedCoupon<f64> {
    /// Returns the amount of the coupon (calculated from accrued amount).
    ///
    /// # Errors
    ///
    /// Returns an error if the accrued amount calculation fails.
    pub fn amount(&self) -> Result<f64> {
        self.accrued_amount(self.accrual_start_date, self.accrual_end_date)
    }
}

impl OptionEmbeddedCoupon<ADReal> {
    /// Returns the amount of the coupon (calculated from accrued amount).
    ///
    /// # Errors
    ///
    /// Returns an error if the accrued amount calculation fails.
    pub fn amount(&self) -> Result<ADReal> {
        self.accrued_amount(self.accrual_start_date, self.accrual_end_date)
    }
}

impl NonLinearCoupon<f64> for OptionEmbeddedCoupon<f64> {
    fn accrual_end_date(&self) -> Date {
        self.accrual_end_date
    }

    fn accrual_start_date(&self) -> Date {
        self.accrual_start_date
    }

    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        let fixing = self
            .fixing
            .ok_or_else(|| QSError::NotFoundErr("Fixing not set".into()))?;

        let year_fraction = self
            .index
            .rate_index_details()?
            .rate_definition()
            .day_counter()
            .year_fraction(start_date, end_date);

        let resuling_rate = self.payoff.evaluate_f64(fixing)?;
        let coupon_rate = self.spread + resuling_rate;
        Ok(coupon_rate * (year_fraction * self.notional))
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
        let coupon_rate: ADReal = (self.spread + resuling_rate).into();
        Ok((coupon_rate * ADReal::new(year_fraction * self.notional)).into())
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

impl From<OptionEmbeddedCoupon<f64>> for OptionEmbeddedCoupon<ADReal> {
    fn from(value: OptionEmbeddedCoupon<f64>) -> Self {
        Self {
            notional: value.notional,
            fixing: value.fixing.map(ADReal::new),
            spread: ADReal::new(value.spread.value()),
            index: value.index,
            accrual_start_date: value.accrual_start_date,
            accrual_end_date: value.accrual_end_date,
            payment_date: value.payment_date,
            payoff: value.payoff,
        }
    }
}

impl From<OptionEmbeddedCoupon<ADReal>> for OptionEmbeddedCoupon<f64> {
    fn from(value: OptionEmbeddedCoupon<ADReal>) -> Self {
        Self {
            notional: value.notional,
            fixing: value.fixing.map(|fixing| fixing.value()),
            spread: value.spread.value(),
            index: value.index,
            accrual_start_date: value.accrual_start_date,
            accrual_end_date: value.accrual_end_date,
            payment_date: value.payment_date,
            payoff: value.payoff,
        }
    }
}
