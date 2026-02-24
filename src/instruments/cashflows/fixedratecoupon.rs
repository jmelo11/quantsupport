use crate::{
    ad::adreal::{ADReal, IsReal},
    instruments::cashflows::{cashflow::Cashflow, coupon::Coupon},
    rates::interestrate::InterestRate,
    time::date::Date,
    utils::errors::Result,
};

pub struct FixedRateCoupon<'a, T: IsReal> {
    notional: f64,
    rate: &'a InterestRate<T>,
    accrual_start_date: Date,
    accrual_end_date: Date,
    payment_date: Date,
}

impl<'a, T: IsReal> FixedRateCoupon<'a, T> {
    pub fn new(
        notional: f64,
        rate: &'a InterestRate<T>,
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
}

impl Cashflow<ADReal> for FixedRateCoupon<'_, ADReal> {
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

impl Coupon<ADReal> for FixedRateCoupon<'_, ADReal> {
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
