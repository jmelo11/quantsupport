use crate::{
    ad::adreal::{ADReal, IsReal},
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::Cashflow, coupon::Coupon},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{AtlasError, Result},
};

pub struct FloatingRateCoupon<'a, T: IsReal> {
    notional: f64,
    spread: T,
    fixing: Option<T>,
    index: &'a MarketIndex,
    start_date: Date,
    end_date: Date,
    payment_date: Date,
    day_counter: DayCounter,
}

impl<'a, T> FloatingRateCoupon<'a, T>
where
    T: IsReal,
{
    pub fn new(
        notional: f64,
        spread: T,
        index: &'a MarketIndex,
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

    pub fn set_fixing(&mut self, fixing: T) {
        self.fixing = Some(fixing);
    }

    pub fn index(&self) -> &MarketIndex {
        self.index
    }
}

impl Cashflow<ADReal> for FloatingRateCoupon<'_, ADReal> {
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

impl Coupon<ADReal> for FloatingRateCoupon<'_, ADReal> {
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
}
