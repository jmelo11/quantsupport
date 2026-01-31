use serde::{Deserialize, Serialize};

use super::cashflow::Side;
use super::simplecashflow::SimpleCashflow;
use super::traits::{Expires, InterestAccrual, Payable};
use crate::core::traits::{HasCurrency, HasDiscountCurveId, HasForecastCurveId};
use crate::utils::errors::AtlasError;
use crate::{
    core::{meta::MarketRequest, traits::Registrable},
    currencies::enums::Currency,
    rates::interestrate::InterestRate,
    time::date::Date,
    utils::errors::Result,
};

/// # `FixedRateCoupon`
/// A fixed rate coupon is a cashflow that pays a fixed rate of interest on a notional amount.
///
/// ## Parameters
/// * `notional` - The notional amount of the coupon
/// * `rate` - The fixed rate of interest
/// * `accrual_start_date` - The date from which the coupon accrues interest
/// * `accrual_end_date` - The date until which the coupon accrues interest
/// * `payment_date` - The date on which the coupon is paid
/// * `currency` - The currency of the coupon
/// * `side` - The side of the coupon (Pay or Receive)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedRateCoupon {
    notional: f64,
    rate: InterestRate,
    accrual_start_date: Date,
    accrual_end_date: Date,
    cashflow: SimpleCashflow,
}

impl FixedRateCoupon {
    /// Creates a new fixed rate coupon.
    ///
    /// # Arguments
    ///
    /// * `notional` - The notional amount of the coupon
    /// * `rate` - The fixed interest rate
    /// * `accrual_start_date` - The date from which interest accrues
    /// * `accrual_end_date` - The date until which interest accrues
    /// * `payment_date` - The date on which the coupon is paid
    /// * `currency` - The currency of the coupon
    /// * `side` - Whether this is a Pay or Receive side
    #[must_use]
    pub fn new(
        notional: f64,
        rate: InterestRate,
        accrual_start_date: Date,
        accrual_end_date: Date,
        payment_date: Date,
        currency: Currency,
        side: Side,
    ) -> Self {
        let amount = notional * (rate.compound_factor(accrual_start_date, accrual_end_date) - 1.0);
        let cashflow = SimpleCashflow::new(payment_date, currency, side).with_amount(amount);
        Self {
            notional,
            rate,
            accrual_start_date,
            accrual_end_date,
            cashflow,
        }
    }

    /// Sets the discount curve ID and returns self for method chaining.
    #[must_use]
    pub const fn with_discount_curve_id(mut self, id: usize) -> Self {
        self.cashflow.set_discount_curve_id(id);
        self
    }

    /// Sets the discount curve ID.
    pub const fn set_discount_curve_id(&mut self, id: usize) {
        self.cashflow.set_discount_curve_id(id);
    }

    /// Sets the interest rate value.
    pub fn set_rate_value(&mut self, rate_value: f64) {
        let rate = InterestRate::from_rate_definition(rate_value, self.rate.rate_definition());
        self.set_rate(rate);
    }

    /// Sets the interest rate and updates the cashflow amount.
    pub fn set_rate(&mut self, rate: InterestRate) {
        self.rate = rate;
        // Update the cashflow amount
        self.cashflow.set_amount(
            self.notional
                * (rate.compound_factor(self.accrual_start_date, self.accrual_end_date) - 1.0),
        );
    }

    /// Sets the notional amount and updates the cashflow amount.
    pub fn set_notional(&mut self, notional: f64) {
        self.notional = notional;
        self.cashflow.set_amount(
            self.notional
                * (self
                    .rate
                    .compound_factor(self.accrual_start_date, self.accrual_end_date)
                    - 1.0),
        );
    }

    /// Returns the notional amount.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Returns the interest rate.
    #[must_use]
    pub const fn rate(&self) -> InterestRate {
        self.rate
    }
}

impl HasCurrency for FixedRateCoupon {
    fn currency(&self) -> Result<Currency> {
        self.cashflow.currency()
    }
}

impl HasDiscountCurveId for FixedRateCoupon {
    fn discount_curve_id(&self) -> Result<usize> {
        self.cashflow.discount_curve_id()
    }
}

impl HasForecastCurveId for FixedRateCoupon {
    fn forecast_curve_id(&self) -> Result<usize> {
        Err(AtlasError::InvalidValueErr(
            "No forecast curve id for fixed rate cashflow".to_string(),
        ))
    }
}

impl Registrable for FixedRateCoupon {
    fn id(&self) -> Result<usize> {
        self.cashflow.id()
    }

    fn set_id(&mut self, id: usize) {
        self.cashflow.set_id(id);
    }

    fn market_request(&self) -> Result<MarketRequest> {
        self.cashflow.market_request()
    }
}

impl InterestAccrual for FixedRateCoupon {
    fn accrual_start_date(&self) -> Result<Date> {
        Ok(self.accrual_start_date)
    }

    fn accrual_end_date(&self) -> Result<Date> {
        Ok(self.accrual_end_date)
    }

    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        let (d1, d2) = self.relevant_accrual_dates(self.accrual_start_date, end_date)?;
        let acc_1 = self.notional * (self.rate.compound_factor(d1, d2) - 1.0);

        let (d1, d2) = self.relevant_accrual_dates(self.accrual_start_date, start_date)?;
        let acc_2 = self.notional * (self.rate.compound_factor(d1, d2) - 1.0);

        Ok(acc_1 - acc_2)
    }
}

impl Payable for FixedRateCoupon {
    fn amount(&self) -> Result<f64> {
        self.cashflow.amount()
    }

    fn side(&self) -> Side {
        self.cashflow.side()
    }

    fn payment_date(&self) -> Date {
        self.cashflow.payment_date()
    }
}

impl Expires for FixedRateCoupon {
    fn is_expired(&self, date: Date) -> bool {
        self.cashflow.is_expired(date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflows::cashflow::Side;
    use crate::currencies::enums::Currency;
    use crate::rates::enums::Compounding;
    use crate::rates::interestrate::InterestRate;
    use crate::time::date::Date;
    use crate::time::daycounter::DayCounter;
    use crate::time::enums::Frequency;

    #[test]
    fn test_fixed_rate_coupon_creation() -> Result<()> {
        let notional = 1000.0;
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );
        let accrual_start_date = Date::new(2023, 1, 1);
        let accrual_end_date = Date::new(2023, 12, 31);
        let payment_date = Date::new(2024, 1, 1);
        let currency = Currency::JPY;

        let coupon = FixedRateCoupon::new(
            notional,
            rate,
            accrual_start_date,
            accrual_end_date,
            payment_date,
            currency,
            Side::Pay,
        );

        assert_eq!(coupon.accrual_start_date()?, accrual_start_date);
        assert_eq!(coupon.accrual_end_date()?, accrual_end_date);

        Ok(())
    }

    #[test]
    fn test_amount_calculation() -> Result<()> {
        let notional = 1000.0;
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let accrual_start_date = Date::new(2023, 1, 1);
        let accrual_end_date = Date::new(2023, 12, 31);
        let payment_date = Date::new(2024, 1, 1);
        let id = 1;
        let currency = Currency::USD;

        let mut coupon = FixedRateCoupon::new(
            notional,
            rate,
            accrual_start_date,
            accrual_end_date,
            payment_date,
            currency,
            Side::Pay,
        );

        coupon.set_discount_curve_id(id);

        let expected_amount =
            notional * (rate.compound_factor(accrual_start_date, accrual_end_date) - 1.0);
        let accrued = coupon.accrued_amount(accrual_start_date, accrual_end_date)?;
        assert!((accrued - expected_amount).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn test_accrual() -> Result<()> {
        let notional = 1000.0;
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );
        let accrual_start_date = Date::new(2023, 12, 10);
        let accrual_end_date = Date::new(2024, 3, 30);
        let payment_date = Date::new(2024, 1, 10);
        let id = 1;
        let currency = Currency::USD;

        let mut coupon = FixedRateCoupon::new(
            notional,
            rate,
            accrual_start_date,
            accrual_end_date,
            payment_date,
            currency,
            Side::Receive,
        );

        coupon.set_discount_curve_id(id);

        let star_date = Date::new(2024, 2, 28);
        let end_date = Date::new(2024, 3, 1);
        let accrued_amount = coupon.accrued_amount(star_date, end_date)?;

        print!("Accrued amount between {star_date} and {end_date} is {accrued_amount}");
        Ok(())
    }
}
