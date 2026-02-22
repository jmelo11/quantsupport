use crate::{time::date::Date, utils::errors::Result};

use super::cashflow::Side;

/// # `InterestAccrual`
/// A trait that defines the accrual period of an instrument.
pub trait InterestAccrual {
    /// Returns the start date of the accrual period.
    ///
    /// # Errors
    /// Returns an error if the accrual start date cannot be determined.
    fn accrual_start_date(&self) -> Result<Date>;
    /// Returns the end date of the accrual period.
    ///
    /// # Errors
    /// Returns an error if the accrual end date cannot be determined.
    fn accrual_end_date(&self) -> Result<Date>;
    /// Calculates the accrued amount between two dates.
    ///
    /// # Errors
    /// Returns an error if the accrued amount cannot be computed for the given dates.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64>;
    /// Returns the relevant accrual dates that intersect with the given date range.
    ///
    /// # Errors
    ///
    /// Returns an error if the relevant accrual dates cannot be determined.
    fn relevant_accrual_dates(&self, start_date: Date, end_date: Date) -> Result<(Date, Date)> {
        let accrual_start = self.accrual_start_date()?;
        let accrual_end = self.accrual_end_date()?;

        // Check if the ranges intersect
        if start_date <= accrual_end && end_date >= accrual_start {
            // The ranges intersect, so we find the relevant accrual dates
            let relevant_start = if accrual_start >= start_date {
                accrual_start
            } else {
                start_date
            };

            let relevant_end = if accrual_end <= end_date {
                accrual_end
            } else {
                end_date
            };

            Ok((relevant_start, relevant_end))
        } else {
            // The ranges do not intersect, so return Date::empty()
            Ok((Date::empty(), Date::empty()))
        }
    }
}

/// # `RequiresFixingRate`
/// A trait that defines if an instrument requires a fixing rate.
pub trait RequiresFixingRate: InterestAccrual {
    /// Sets the fixing rate for the instrument.
    fn set_fixing_rate(&mut self, fixing_rate: f64);
}

/// # `Payable`
/// A trait that defines the payment of an instrument.
pub trait Payable {
    /// Returns the payment amount.
    ///
    /// # Errors
    ///
    /// Returns an error if the payment amount cannot be determined.
    fn amount(&self) -> Result<f64>;
    /// Returns the side of the payment (pay or receive).
    fn side(&self) -> Side;
    /// Returns the payment date.
    fn payment_date(&self) -> Date;
}

/// # `Expires`
/// A trait that defines if an instrument expires.
pub trait Expires {
    /// Checks if the instrument has expired at the given date.
    fn is_expired(&self, date: Date) -> bool;
}

#[cfg(test)]
mod tests {
    use crate::{
        cashflows::fixedratecoupon::FixedRateCoupon,
        currencies::enums::Currency,
        rates::{enums::Compounding, interestrate::InterestRate},
        time::{daycounter::DayCounter, enums::Frequency},
    };

    use super::*;

    #[test]
    fn test_delta_accrued_amount_simple() -> Result<()> {
        let notional = 10000.0;
        let rate = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Thirty360,
        );
        let accrual_start_date = Date::new(2023, 1, 1);
        let accrual_end_date = Date::new(2023, 3, 31);
        let payment_date = Date::new(2023, 3, 31);
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

        let mut start_date = Date::new(2023, 1, 1);
        let mut end_date = Date::new(2023, 3, 31);
        let mut accrued_amount = coupon.accrued_amount(start_date, end_date)?;
        assert!((accrued_amount - 125.0).abs() < 0.00001);

        start_date = Date::new(2023, 1, 15);
        end_date = Date::new(2023, 1, 16);
        accrued_amount = coupon.accrued_amount(start_date, end_date)?;
        assert!((accrued_amount - 125.0 / 90.0).abs() < 0.00001);
        Ok(())
    }

    #[test]
    fn test_delta_accrued_amount_compounded() -> Result<()> {
        let notional = 10000.0;
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );
        let accrual_start_date = Date::new(2023, 1, 30);
        let accrual_end_date = Date::new(2023, 3, 31);
        let payment_date = Date::new(2023, 3, 31);
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

        let start_date = Date::new(2023, 1, 30);
        let end_date = Date::new(2023, 3, 31);
        let accrued_amount = coupon.clone().accrued_amount(start_date, end_date)?;

        assert!(accrued_amount - 122.72234429 < 0.00001);
        Ok(())
    }
}
