use crate::{
    ad::scalar::Scalar,
    instruments::cashflows::{cashflow::Cashflow, payoffops::PayoffOps},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A [`LinearCoupon`] is a type of [`Cashflow`] that represents a periodic payment.
///
/// In addition to the amount and payment date, a coupon also has an accrual period defined by a start and end date,
/// and can calculate the accrued amount for a given period.
pub trait LinearCoupon<T>: Cashflow<T>
where
    T: Scalar,
{
    /// Returns earned interest in the overlap of `[start_date, end_date)` and
    /// the contractual coupon period. Empty overlaps return zero.
    ///
    /// Accrual is the difference of cumulative amounts measured from the coupon
    /// start, so adjacent windows reconcile even for non-additive day counts.
    /// Floating observations must be resolved for nonzero accrual.
    ///
    /// ## Errors
    /// Returns an error if the accrued amount cannot be calculated.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<T>;

    /// Returns the accrual start date.
    fn accrual_start_date(&self) -> Date;

    /// Returns the accrual end date.
    fn accrual_end_date(&self) -> Date;

    /// Returns the notional amount associated with this coupon.
    fn notional(&self) -> f64;
}

/// A [`NonLinearCoupon`] is a coupon that contains some optionality.
pub trait NonLinearCoupon<T> {
    /// Returns the payoff computation tree.
    fn payoff_description(&self) -> PayoffOps;

    /// Returns earned interest in the overlap of `[start_date, end_date)` and
    /// the contractual coupon period. Empty overlaps return zero.
    ///
    /// Accrual is the difference of cumulative amounts measured from the coupon
    /// start, so adjacent windows reconcile even for non-additive day counts.
    /// Floating observations must be resolved for nonzero accrual.
    ///
    /// # Errors
    /// Returns an error if the accrued amount cannot be calculated.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<T>;

    /// Returns the accrual start date.
    fn accrual_start_date(&self) -> Date;

    /// Returns the accrual end date.
    fn accrual_end_date(&self) -> Date;

    /// Returns the notional amount associated with this coupon.
    fn notional(&self) -> f64;

    /// Returns the payment date of the coupon.
    fn payment_date(&self) -> Date;
}

/// Validates a range and returns its cumulative contractual accrual increment.
///
/// # Errors
/// Rejects reversed request/contract dates and invalid day-count increments.
pub(crate) fn accrual_fraction(
    day_counter: DayCounter,
    coupon_start: Date,
    coupon_end: Date,
    start: Date,
    end: Date,
) -> Result<f64> {
    if end < start || coupon_end < coupon_start {
        return Err(QSError::InvalidValueErr(
            "Accrual end must not precede start".into(),
        ));
    }
    let left = start.max(coupon_start);
    let right = end.min(coupon_end);
    if right <= left {
        return Ok(0.0);
    }
    let fraction = day_counter.year_fraction(coupon_start, right)
        - day_counter.year_fraction(coupon_start, left);
    if !fraction.is_finite() || fraction < 0.0 {
        return Err(QSError::InvalidValueErr(
            "Invalid contractual accrual fraction".into(),
        ));
    }
    Ok(fraction)
}

#[cfg(test)]
mod accrual_tests {
    use super::*;
    use crate::instruments::cashflows::{
        optionembeddedcoupon::OptionEmbeddedCoupon, payoffops::PayoffOps,
    };
    use crate::prelude::*;

    fn fixed(basis: DayCounter) -> FixedRateCoupon<f64> {
        FixedRateCoupon::new(
            1_000.0,
            Box::new(InterestRate::new(
                0.06,
                Compounding::Simple,
                Frequency::Annual,
                basis,
            )),
            Date::new(2026, 1, 31),
            Date::new(2026, 3, 31),
            Date::new(2026, 4, 5),
        )
    }

    #[test]
    fn clips_range_and_rejects_reversed_dates() -> Result<()> {
        let coupon = fixed(DayCounter::Actual365);
        assert!(
            (coupon.accrued_amount(Date::new(2025, 1, 1), Date::new(2027, 1, 1))?
                - coupon.amount()?)
            .abs()
                < 1e-12
        );
        assert_eq!(
            coupon.accrued_amount(Date::new(2026, 1, 1), Date::new(2026, 1, 31))?,
            0.0
        );
        assert_eq!(
            coupon.accrued_amount(Date::new(2026, 3, 31), Date::new(2026, 5, 1))?,
            0.0
        );
        assert_eq!(
            coupon.accrued_amount(Date::new(2026, 2, 10), Date::new(2026, 2, 10))?,
            0.0
        );
        assert!(coupon
            .accrued_amount(Date::new(2026, 3, 1), Date::new(2026, 2, 1))
            .is_err());
        Ok(())
    }

    #[test]
    fn monthly_windows_reconcile_for_thirty_360_and_ad_values() -> Result<()> {
        let coupon = fixed(DayCounter::Thirty360);
        let split = [
            Date::new(2026, 1, 31),
            Date::new(2026, 2, 1),
            Date::new(2026, 3, 1),
            Date::new(2026, 3, 31),
        ];
        let mut sum = 0.0;
        let dual: FixedRateCoupon<DualFwd> = coupon.clone().into();
        for dates in split.windows(2) {
            let value = coupon.accrued_amount(dates[0], dates[1])?;
            assert!((value - dual.accrued_amount(dates[0], dates[1])?.value()).abs() < 1e-12);
            sum += value;
        }
        assert!((sum - 10.0).abs() < 1e-12);
        assert!((sum - coupon.amount()?).abs() < 1e-12);
        // Direct day-count fractions for these subperiods would total 61/360, not 60/360.
        let direct: f64 = split
            .windows(2)
            .map(|d| DayCounter::Thirty360.year_fraction(d[0], d[1]))
            .sum();
        assert!((direct - 61.0 / 360.0).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn missing_fixing_is_needed_only_for_nonempty_floating_overlap() -> Result<()> {
        let coupon = FloatingRateCoupon::<f64>::new(
            1_000.0,
            0.01,
            MarketIndex::EURIBOR3m,
            Date::new(2026, 1, 1),
            Date::new(2026, 4, 1),
            Date::new(2026, 4, 1),
        );
        assert_eq!(
            coupon.accrued_amount(Date::new(2025, 1, 1), Date::new(2026, 1, 1))?,
            0.0
        );
        assert!(coupon
            .accrued_amount(Date::new(2026, 2, 1), Date::new(2026, 3, 1))
            .is_err());
        coupon.set_fixing(0.04);
        assert!(
            (coupon.accrued_amount(Date::new(2026, 2, 1), Date::new(2026, 3, 1))?
                - 1_000.0 * 0.05 * 28.0 / 360.0)
                .abs()
                < 1e-12
        );
        Ok(())
    }

    #[test]
    fn resolved_option_coupon_accrues_its_effective_payoff() -> Result<()> {
        let mut coupon = OptionEmbeddedCoupon::new(
            1_000.0,
            MarketIndex::EURIBOR3m,
            0.01,
            Date::new(2026, 1, 1),
            Date::new(2026, 4, 1),
            Date::new(2026, 4, 1),
            PayoffOps::Min(Box::new(PayoffOps::Index), Box::new(PayoffOps::Const(0.04))),
        );
        assert_eq!(
            coupon.accrued_amount(Date::new(2027, 1, 1), Date::new(2027, 2, 1))?,
            0.0
        );
        coupon.set_fixing(0.08);
        assert!(
            (coupon.accrued_amount(Date::new(2026, 2, 1), Date::new(2026, 3, 1))?
                - 1_000.0 * 0.05 * 28.0 / 360.0)
                .abs()
                < 1e-12
        );
        Ok(())
    }

    #[test]
    fn long_actual_actual_coupon_includes_every_calendar_year() -> Result<()> {
        let coupon = FixedRateCoupon::new(
            1_000.0,
            Box::new(InterestRate::new(
                0.06,
                Compounding::Simple,
                Frequency::Annual,
                DayCounter::ActualActual,
            )),
            Date::new(2023, 1, 1),
            Date::new(2026, 1, 1),
            Date::new(2026, 1, 1),
        );
        assert!((coupon.amount()? - 180.0).abs() < 1e-12);
        assert!(
            (coupon.accrued_amount(Date::new(2024, 1, 1), Date::new(2025, 1, 1))? - 60.0).abs()
                < 1e-12
        );
        assert!(
            (DayCounter::ActualActual.year_fraction(Date::new(2026, 1, 1), Date::new(2023, 1, 1))
                + 3.0)
                .abs()
                < 1e-12
        );
        Ok(())
    }
    #[test]
    fn partial_overnight_accrual_is_rejected_instead_of_linearized() -> Result<()> {
        let start = Date::new(2026, 1, 1);
        let end = Date::new(2026, 4, 1);
        let mut coupon =
            FloatingRateCoupon::<f64>::new(1_000.0, 0.0, MarketIndex::SOFR, start, end, end);
        coupon.set_overnight_conventions(OvernightCouponConventions::new())?;
        coupon.set_fixing(0.04);
        assert!(coupon.accrued_amount(start, Date::new(2026, 2, 1)).is_err());
        assert!((coupon.accrued_amount(start, end)? - coupon.amount()?).abs() < 1e-12);
        assert_eq!(coupon.accrued_amount(end, end)?, 0.0);
        Ok(())
    }
}
