use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{cashflow::Cashflow, coupons::LinearCoupon},
        rates::oisconventions::{OvernightAveraging, OvernightCouponConventions},
    },
    time::{
        calendars::traits::IsCalendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, TimeUnit},
        period::Period,
    },
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
    #[serde(default)]
    overnight_conventions: Option<OvernightCouponConventions>,
}

#[allow(clippy::unwrap_used)]
impl<T> Clone for FloatingRateCoupon<T>
where
    T: Scalar,
{
    fn clone(&self) -> Self {
        let mut coupon = Self::new(
            self.notional,
            self.spread,
            self.index.clone(),
            self.start_date,
            self.end_date,
            self.payment_date,
        );
        coupon
            .overnight_conventions
            .clone_from(&self.overnight_conventions);
        coupon.day_counter = self.day_counter;
        let value = *self.fixing.read().unwrap();
        if let Some(fixing) = value {
            coupon.set_fixing(fixing);
        }
        coupon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coupon(start: Date, end: Date, spread: f64) -> FloatingRateCoupon<f64> {
        let mut coupon =
            FloatingRateCoupon::new(1.0, spread, MarketIndex::SOFR, start, end, end + 2);
        coupon
            .set_overnight_conventions(OvernightCouponConventions::default())
            .unwrap();
        coupon
    }

    #[test]
    fn overnight_weekend_and_partial_realization() {
        let start = Date::new(2026, 1, 2);
        let monday = Date::new(2026, 1, 5);
        let end = Date::new(2026, 1, 6);
        let coupon = coupon(start, end, 0.001);
        let rate = coupon
            .overnight_rate(
                monday,
                |date| {
                    Ok(if date == monday {
                        1.0
                    } else if date == end {
                        0.9998
                    } else {
                        panic!("unexpected forecast date")
                    })
                },
                |date| Ok((date == start).then_some(0.04)),
            )
            .unwrap();
        coupon.set_fixing(rate);
        let expected = (1.0 + 0.04 * 3.0 / 360.0) / 0.9998 - 1.0 + 0.001 * 4.0 / 360.0;
        assert!((coupon.amount().unwrap() - expected).abs() < 1e-14);
    }

    #[test]
    fn overnight_holiday_uses_previous_business_day_fixing() {
        let start = Date::new(2026, 10, 9);
        let end = Date::new(2026, 10, 13);
        let coupon = coupon(start, end, 0.0);
        let rate = coupon
            .overnight_rate(
                end,
                |_| panic!("fully realized coupon"),
                |date| {
                    assert_eq!(date, start);
                    Ok(Some(0.04))
                },
            )
            .unwrap();
        assert!((rate - 0.04).abs() < 1e-12);
    }

    #[test]
    fn overnight_requires_all_historical_fixings() {
        let coupon = coupon(Date::new(2026, 1, 2), Date::new(2026, 1, 6), 0.0);
        assert!(coupon
            .overnight_rate(Date::new(2026, 1, 5), |_| Ok(1.0), |_| Ok(None))
            .is_err());
    }

    #[test]
    fn overnight_future_daily_product_matches_telescoping() {
        let start = Date::new(2026, 1, 2);
        let end = Date::new(2026, 2, 2);
        let mut coupon = coupon(start, end, 0.0);
        let discount = |date| Ok((-0.04 * DayCounter::Actual365.year_fraction(start, date)).exp());
        let rate = coupon
            .overnight_rate(start, discount, |_| Ok(None))
            .unwrap();
        coupon
            .set_overnight_conventions(
                OvernightCouponConventions::new().with_compound_spread_daily(true),
            )
            .unwrap();
        let daily = coupon
            .overnight_rate(start, discount, |_| Ok(None))
            .unwrap();
        assert!((rate - daily).abs() < 1e-12);
    }

    #[test]
    fn overnight_today_fixing_is_used_when_present() {
        let start = Date::new(2026, 1, 2);
        let end = Date::new(2026, 1, 5);
        let coupon = coupon(start, end, 0.0);
        let realized = coupon
            .overnight_rate(
                start,
                |_| panic!("today fixing is known"),
                |_| Ok(Some(0.03)),
            )
            .unwrap();
        assert!((realized - 0.03).abs() < 1e-12);
    }

    #[test]
    fn overnight_lookback_shift_and_lockout() {
        let start = Date::new(2026, 1, 5);
        let end = Date::new(2026, 1, 8);
        let mut coupon = coupon(start, end, 0.0);
        let fixings = |date| {
            Ok(Some(if date == Date::new(2026, 1, 2) {
                0.03
            } else {
                0.05
            }))
        };
        coupon
            .set_overnight_conventions(OvernightCouponConventions::new().with_lookback_days(1))
            .unwrap();
        let unshifted = coupon
            .overnight_rate(end, |_| panic!("realized"), fixings)
            .unwrap();
        let expected =
            ((1.0 + 0.03 / 360.0) * (1.0 + 0.05_f64 / 360.0).powi(2) - 1.0) / (3.0 / 360.0);
        assert!((unshifted - expected).abs() < 1e-12);
        coupon
            .set_overnight_conventions(
                OvernightCouponConventions::new()
                    .with_lookback_days(1)
                    .with_observation_shift(true),
            )
            .unwrap();
        let shifted = coupon
            .overnight_rate(end, |_| panic!("realized"), fixings)
            .unwrap();
        let expected_shifted =
            ((1.0 + 0.03 * 3.0 / 360.0) * (1.0 + 0.05_f64 / 360.0).powi(2) - 1.0) / (5.0 / 360.0);
        assert!((shifted - expected_shifted).abs() < 1e-12);
        coupon
            .set_overnight_conventions(
                OvernightCouponConventions::new()
                    .with_lookback_days(1)
                    .with_lockout_days(2),
            )
            .unwrap();
        let locked = coupon
            .overnight_rate(end, |_| panic!("realized"), fixings)
            .unwrap();
        assert!((locked - ((1.0 + 0.03_f64 / 360.0).powi(3) - 1.0) / (3.0 / 360.0)).abs() < 1e-12);
    }

    #[test]
    fn overnight_arithmetic_and_compounded_spread() {
        let start = Date::new(2026, 1, 5);
        let end = Date::new(2026, 1, 7);
        let mut coupon = coupon(start, end, 0.01);
        coupon
            .set_overnight_conventions(
                OvernightCouponConventions::new().with_averaging(OvernightAveraging::Simple),
            )
            .unwrap();
        let average = coupon
            .overnight_rate(
                end,
                |_| panic!("realized"),
                |date| Ok(Some(if date == start { 0.03 } else { 0.05 })),
            )
            .unwrap();
        assert!((average - 0.04).abs() < 1e-14);
        coupon
            .set_overnight_conventions(
                OvernightCouponConventions::new().with_compound_spread_daily(true),
            )
            .unwrap();
        let compounded = coupon
            .overnight_rate(end, |_| panic!("realized"), |_| Ok(Some(0.03)))
            .unwrap();
        coupon.set_fixing(compounded);
        assert!(
            (coupon.amount().unwrap() - ((1.0 + 0.04_f64 / 360.0).powi(2) - 1.0)).abs() < 1e-14
        );
        let converted: FloatingRateCoupon<DualFwd> = coupon.clone().into();
        assert_eq!(
            converted.overnight_conventions(),
            coupon.overnight_conventions()
        );
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
            overnight_conventions: None,
        }
    }

    /// Uses a contractual accrual day counter independently of the index quote.
    #[must_use]
    pub const fn with_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.day_counter = day_counter;
        self
    }

    /// Installs explicit overnight observation conventions.
    ///
    /// # Errors
    /// Rejects negative observation lookbacks and term-rate indices.
    pub fn set_overnight_conventions(
        &mut self,
        conventions: OvernightCouponConventions,
    ) -> Result<()> {
        if conventions.lookback_days() < 0 || !self.index.rate_index_details()?.is_in_arrears() {
            return Err(QSError::InvalidValueErr(
                "Invalid overnight coupon conventions or index".into(),
            ));
        }
        self.overnight_conventions = Some(conventions);
        Ok(())
    }

    /// Returns overnight observation conventions, when this is an OIS coupon.
    pub const fn overnight_conventions(&self) -> Option<&OvernightCouponConventions> {
        self.overnight_conventions.as_ref()
    }

    /// Business-day observations required through the valuation date, including today.
    ///
    /// # Errors
    /// Rejects invalid overnight conventions or lockout lengths.
    pub fn overnight_fixing_dates(&self, evaluation_date: Date) -> Result<Vec<Date>> {
        let conventions = self
            .overnight_conventions
            .as_ref()
            .ok_or_else(|| QSError::InvalidValueErr("Not an overnight coupon".into()))?;
        if self.start_date > evaluation_date && conventions.lookback_days() == 0 {
            return Ok(Vec::new());
        }
        let periods = self.overnight_periods(conventions)?;
        let mut dates = Vec::new();
        for position in 0..periods.len() {
            let observation_position = if conventions.lockout_days() > 0
                && position >= periods.len() - conventions.lockout_days()
            {
                periods.len() - conventions.lockout_days() - 1
            } else {
                position
            };
            let date = periods[observation_position].0;
            if date <= evaluation_date && dates.last() != Some(&date) {
                dates.push(date);
            }
        }
        Ok(dates)
    }

    fn overnight_periods(
        &self,
        conventions: &OvernightCouponConventions,
    ) -> Result<Vec<(Date, Date, f64)>> {
        let calendar = self.index.rate_index_details()?.calendar();
        let mut periods = Vec::new();
        let mut value_date =
            calendar.adjust(self.start_date, Some(BusinessDayConvention::Preceding));
        while value_date < self.end_date {
            let next = calendar.advance(value_date, Period::new(1, TimeUnit::Days), None, false);
            let observation = calendar.advance(
                value_date,
                Period::new(-conventions.lookback_days(), TimeUnit::Days),
                None,
                false,
            );
            let observation_end = calendar.advance(
                next,
                Period::new(-conventions.lookback_days(), TimeUnit::Days),
                None,
                false,
            );
            let weight = if conventions.observation_shift() {
                self.day_counter.year_fraction(observation, observation_end)
            } else {
                self.day_counter
                    .year_fraction(value_date.max(self.start_date), next.min(self.end_date))
            };
            periods.push((observation, observation_end, weight));
            value_date = next;
        }
        if periods.is_empty()
            || (conventions.lockout_days() >= periods.len() && conventions.lockout_days() != 0)
        {
            return Err(QSError::InvalidValueErr(
                "Overnight lockout must be shorter than the coupon".into(),
            ));
        }
        Ok(periods)
    }

    /// Resolves a daily overnight coupon to the equivalent fixing used by `amount`.
    /// Known fixings use business-day intervals; future standard coupons telescope.
    /// The fixing callback returns `None` only when a fixing is unavailable.
    ///
    /// # Errors
    /// Rejects missing historical observations and invalid accrual or lockout periods.
    pub fn overnight_rate(
        &self,
        evaluation_date: Date,
        mut discount_factor: impl FnMut(Date) -> Result<T>,
        mut fixing: impl FnMut(Date) -> Result<Option<f64>>,
    ) -> Result<T> {
        let conventions = self
            .overnight_conventions
            .as_ref()
            .ok_or_else(|| QSError::InvalidValueErr("Not an overnight coupon".into()))?;
        let calendar = self.index.rate_index_details()?.calendar();
        let tau = self
            .day_counter
            .year_fraction(self.start_date, self.end_date);
        if tau <= 0.0 || conventions.lookback_days() < 0 {
            return Err(QSError::InvalidValueErr(
                "Invalid overnight accrual period or lookback".into(),
            ));
        }
        let today_known = self.start_date == evaluation_date && fixing(self.start_date)?.is_some();
        if self.start_date >= evaluation_date
            && !today_known
            && conventions.lookback_days() == 0
            && conventions.lockout_days() == 0
            && conventions.averaging() == OvernightAveraging::Compound
            && !conventions.compound_spread_daily()
            && calendar.is_business_day(&self.start_date)
            && calendar.is_business_day(&self.end_date)
        {
            return Ok(discount_factor(self.start_date)?
                .div_val(discount_factor(self.end_date)?)
                .sub_val(T::one())
                .div_val(T::scalar(tau)));
        }
        let periods = self.overnight_periods(conventions)?;
        let rate_tau = if conventions.observation_shift() {
            periods.iter().map(|period| period.2).sum()
        } else {
            tau
        };
        let mut compound = T::scalar(1.0);
        let mut accumulated = T::scalar(0.0);
        for (position, &(_, _, weight)) in periods.iter().enumerate() {
            let observation_position = if conventions.lockout_days() > 0
                && position >= periods.len() - conventions.lockout_days()
            {
                periods.len() - conventions.lockout_days() - 1
            } else {
                position
            };
            let (observation, observation_end, _) = periods[observation_position];
            let known = if observation <= evaluation_date {
                fixing(observation)?
            } else {
                None
            };
            let rate = if let Some(rate) = known {
                T::scalar(rate)
            } else if observation < evaluation_date {
                return Err(QSError::NotFoundErr(format!(
                    "Missing {} overnight fixing for {observation}",
                    self.index
                )));
            } else {
                discount_factor(observation)?
                    .div_val(discount_factor(observation_end)?)
                    .sub_val(T::one())
                    .div_val(T::scalar(
                        self.day_counter.year_fraction(observation, observation_end),
                    ))
            };
            accumulated = accumulated.add_val(rate.mul_val(T::scalar(weight)));
            let daily_rate = if conventions.compound_spread_daily() {
                rate.add_val(self.spread)
            } else {
                rate
            };
            compound = compound.mul_val(T::one().add_val(daily_rate.mul_val(T::scalar(weight))));
        }
        Ok(match conventions.averaging() {
            OvernightAveraging::Simple => accumulated.div_val(T::scalar(rate_tau)),
            OvernightAveraging::Compound => {
                let rate = compound.sub_val(T::one()).div_val(T::scalar(rate_tau));
                if conventions.compound_spread_daily() {
                    rate.sub_val(self.spread)
                } else {
                    rate
                }
            }
        })
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

    /// Contractual principal; it is not an AD input.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Contractual payment date, independent of the rate scalar type.
    #[must_use]
    pub const fn payment_date(&self) -> Date {
        self.payment_date
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
        let mut coupon = Self::new(
            value.notional,
            DualFwd::new(value.spread.value()),
            value.index,
            value.start_date,
            value.end_date,
            value.payment_date,
        );
        coupon.overnight_conventions = value.overnight_conventions;
        coupon.day_counter = value.day_counter;
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
        let mut coupon = Self::new(
            value.notional,
            value.spread.value(),
            value.index,
            value.start_date,
            value.end_date,
            value.payment_date,
        );
        coupon.overnight_conventions = value.overnight_conventions;
        coupon.day_counter = value.day_counter;
        if let Some(fixing) = fixing {
            coupon.set_fixing(fixing.value());
        }
        coupon
    }
}
