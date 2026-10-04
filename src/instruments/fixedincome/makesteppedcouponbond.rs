//! Builder for callable stepped-coupon bonds.

use std::{collections::HashSet, marker::PhantomData};

use crate::{
    ad::scalar::Scalar,
    core::{instrument::AssetClass, trade::Side},
    currencies::currency::Currency,
    instruments::{
        cashflows::{cashflow::SimpleCashflow, cashflowtype::CashflowType, leg::Leg},
        fixedincome::{callschedule::CallScheduleEntry, steppedcouponbond::SteppedCouponBond},
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{
        calendar::Calendar,
        calendars::nullcalendar::NullCalendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
        schedule::MakeSchedule,
    },
    utils::errors::{QSError, Result},
};

/// Fluent builder for a bullet or amortizing [`SteppedCouponBond`].
///
/// `coupons` and `notional_percentages` are positional schedules. If either
/// list is shorter than the generated coupon schedule, its final value is
/// repeated through maturity.
#[derive(Default)]
pub struct MakeSteppedCouponBond<T: Scalar> {
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    units: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    currency: Option<Currency>,
    side: Option<Side>,
    payment_frequency: Option<Frequency>,
    coupon_day_counter: Option<DayCounter>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    first_coupon_date: Option<Date>,
    coupons: Vec<CashflowType<T>>,
    notional_percentages: Vec<f64>,
    call_schedule: Vec<CallScheduleEntry>,
    _marker: PhantomData<T>,
}

impl<T: Scalar> MakeSteppedCouponBond<T> {
    /// Sets the bond effective date.
    #[must_use]
    pub const fn with_start_date(mut self, date: Date) -> Self {
        self.start_date = Some(date);
        self
    }

    /// Sets the contractual maturity date.
    #[must_use]
    pub const fn with_maturity_date(mut self, date: Date) -> Self {
        self.maturity_date = Some(date);
        self
    }

    /// Sets the original principal amount.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the quote units, normally 100.
    #[must_use]
    pub const fn with_units(mut self, units: f64) -> Self {
        self.units = Some(units);
        self
    }

    /// Sets the instrument identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the payment currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the contractual leg side.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the regular coupon frequency.
    #[must_use]
    pub const fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the day counter used by fixed coupon definitions.
    #[must_use]
    pub const fn with_coupon_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.coupon_day_counter = Some(day_counter);
        self
    }

    /// Sets the business calendar.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Sets the business-day adjustment convention.
    #[must_use]
    pub const fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Sets the date-generation rule.
    #[must_use]
    pub const fn with_date_generation_rule(mut self, rule: DateGenerationRule) -> Self {
        self.date_generation_rule = Some(rule);
        self
    }

    /// Enables or disables end-of-month schedule generation.
    #[must_use]
    pub const fn with_end_of_month(mut self, enabled: bool) -> Self {
        self.end_of_month = Some(enabled);
        self
    }

    /// Sets a first coupon date for a stub period.
    #[must_use]
    pub const fn with_first_coupon_date(mut self, date: Date) -> Self {
        self.first_coupon_date = Some(date);
        self
    }

    /// Sets positional coupon definitions.
    #[must_use]
    pub fn with_coupons(mut self, coupons: Vec<CashflowType<T>>) -> Self {
        self.coupons = coupons;
        self
    }

    /// Sets positional outstanding-notional percentages.
    #[must_use]
    pub fn with_notional_percentages(mut self, percentages: Vec<f64>) -> Self {
        self.notional_percentages = percentages;
        self
    }

    /// Sets explicit issuer call dates and strikes.
    #[must_use]
    pub fn with_call_schedule(mut self, calls: Vec<CallScheduleEntry>) -> Self {
        self.call_schedule = calls;
        self
    }

    /// Builds and validates the stepped-coupon bond.
    ///
    /// # Errors
    /// Returns an error for missing fields, invalid schedule lists, or an
    /// invalid call schedule.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<SteppedCouponBond<T>> {
        let start_date = required(self.start_date, "Start date")?;
        let maturity_date = required(self.maturity_date, "Maturity date")?;
        let notional = required(self.notional, "Notional")?;
        let identifier = required(self.identifier, "Identifier")?;
        let currency = required(self.currency, "Currency")?;
        let units = self.units.unwrap_or(100.0);
        if !notional.is_finite() || notional <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Stepped-coupon bond notional must be finite and positive".into(),
            ));
        }

        let mut schedule_builder = MakeSchedule::new(start_date, maturity_date)
            .with_frequency(self.payment_frequency.unwrap_or(Frequency::Semiannual))
            .with_calendar(
                self.calendar
                    .unwrap_or_else(|| Calendar::NullCalendar(NullCalendar::new())),
            )
            .with_convention(
                self.business_day_convention
                    .unwrap_or(BusinessDayConvention::Unadjusted),
            )
            .with_rule(
                self.date_generation_rule
                    .unwrap_or(DateGenerationRule::Backward),
            )
            .end_of_month(self.end_of_month.unwrap_or(false));
        if let Some(first_coupon_date) = self.first_coupon_date {
            schedule_builder = schedule_builder.with_first_date(first_coupon_date);
        }
        let schedule = schedule_builder.build()?;
        let dates = schedule.dates();
        let period_count = dates.len().saturating_sub(1);
        if period_count == 0 {
            return Err(QSError::InvalidValueErr(
                "Stepped-coupon bond requires at least one coupon period".into(),
            ));
        }
        if self.coupons.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Stepped-coupon bond requires at least one coupon definition".into(),
            ));
        }
        if self.notional_percentages.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Stepped-coupon bond requires at least one notional percentage".into(),
            ));
        }
        if self.coupons.len() > period_count {
            return Err(QSError::InvalidValueErr(format!(
                "Coupon list has {} entries but the bond has only {period_count} payment periods",
                self.coupons.len()
            )));
        }
        if self.notional_percentages.len() > period_count {
            return Err(QSError::InvalidValueErr(format!(
                "Notional-percentage list has {} entries but the bond has only {period_count} payment periods",
                self.notional_percentages.len()
            )));
        }
        if (self.notional_percentages[0] - 1.0).abs() > 1.0e-12 {
            return Err(QSError::InvalidValueErr(
                "The first outstanding-notional percentage must be 1.0".into(),
            ));
        }

        let expanded_coupons = repeat_last(&self.coupons, period_count)?;
        let expanded_percentages = repeat_last(&self.notional_percentages, period_count)?;
        for values in expanded_percentages.windows(2) {
            if values[1] > values[0] {
                return Err(QSError::InvalidValueErr(
                    "Outstanding-notional percentages must be non-increasing".into(),
                ));
            }
        }

        let floating_indices = self
            .coupons
            .iter()
            .filter_map(|coupon| match coupon {
                CashflowType::FloatingRateCoupon(coupon) => Some(coupon.market_index().clone()),
                CashflowType::OptionEmbeddedCoupon(coupon) => Some(coupon.market_index().clone()),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let forward_index = (floating_indices.len() == 1)
            .then(|| floating_indices.into_iter().next())
            .flatten();

        let fixed_definition = self.coupon_day_counter.map(|day_counter| {
            RateDefinition::new(day_counter, Compounding::Simple, Frequency::Annual)
        });
        let mut cashflows = vec![CashflowType::Disbursement(SimpleCashflow::new(
            notional, start_date,
        ))];
        let mut contains_optional_coupon = false;
        for (period_index, period_dates) in dates.windows(2).enumerate() {
            let accrual_start = period_dates[0];
            let accrual_end = period_dates[1];
            let outstanding = notional * expanded_percentages[period_index];
            let coupon = expanded_coupons[period_index].for_period(
                outstanding,
                accrual_start,
                accrual_end,
                accrual_end,
                fixed_definition,
            )?;
            contains_optional_coupon |= matches!(coupon, CashflowType::OptionEmbeddedCoupon(_));
            cashflows.push(coupon);

            let next_percentage = expanded_percentages
                .get(period_index + 1)
                .copied()
                .unwrap_or(0.0);
            let redemption = notional * (expanded_percentages[period_index] - next_percentage);
            if redemption > 0.0 {
                cashflows.push(CashflowType::Redemption(SimpleCashflow::new(
                    redemption,
                    accrual_end,
                )));
            }
        }

        let leg = Leg::new(
            0,
            cashflows,
            currency,
            None,
            forward_index,
            None,
            None,
            self.side.unwrap_or(Side::LongReceive),
            !contains_optional_coupon,
            AssetClass::FixedIncome,
            dates[1],
            maturity_date,
        );
        SteppedCouponBond::new(
            identifier,
            units,
            leg,
            currency,
            self.coupons,
            self.notional_percentages,
            self.call_schedule,
        )
    }
}

/// Extracts a required builder field with a domain-specific error label.
fn required<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| QSError::ValueNotSetErr(label.into()))
}

/// Expands a positional schedule and carries its final entry through `count`.
fn repeat_last<T: Clone>(values: &[T], count: usize) -> Result<Vec<T>> {
    let last = values
        .last()
        .ok_or_else(|| QSError::InvalidValueErr("Schedule list cannot be empty".into()))?;
    Ok((0..count)
        .map(|index| values.get(index).unwrap_or(last).clone())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indices::marketindex::MarketIndex;
    use crate::{ad::dual::DualFwd, instruments::cashflows::cashflow::Cashflow};

    #[test]
    fn repeats_last_coupon_and_notional_percentage() {
        let start = Date::new(2025, 1, 1);
        let bond = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("STEP-CALL".into())
            .with_start_date(start)
            .with_maturity_date(Date::new(2028, 1, 1))
            .with_notional(100.0)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_coupons(vec![
                CashflowType::fixed(0.04),
                CashflowType::floating(MarketIndex::SOFR, 0.01, Some(0.0), Some(0.05)),
            ])
            .with_notional_percentages(vec![1.0, 0.8])
            .with_call_schedule(vec![
                CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0),
                CallScheduleEntry::new(Date::new(2026, 1, 1), 101.0),
            ])
            .build()
            .unwrap_or_else(|error| panic!("valid stepped bond: {error}"));

        assert_eq!(bond.coupons().len(), 2);
        assert_eq!(bond.notional_percentages(), &[1.0, 0.8]);
        assert_eq!(bond.call_schedule().len(), 2);
        assert_eq!(bond.call_schedule()[0].date(), Date::new(2026, 1, 1));
        assert_eq!(bond.call_schedule()[1].date(), Date::new(2027, 1, 1));
        assert_eq!(
            bond.leg()
                .cashflows()
                .iter()
                .filter(|cashflow| matches!(cashflow, CashflowType::OptionEmbeddedCoupon(_)))
                .count(),
            5
        );
        let redemptions = bond
            .leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::Redemption(redemption) => Some(redemption.amount()),
                _ => None,
            })
            .collect::<Result<Vec<_>>>()
            .unwrap_or_else(|error| panic!("redemptions: {error}"));
        assert_eq!(redemptions.len(), 2);
        assert!((redemptions[0] - 20.0).abs() < 1.0e-12);
        assert!((redemptions[1] - 80.0).abs() < 1.0e-12);
    }

    #[test]
    fn rejects_increasing_outstanding_notional() {
        let result = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("BAD-NOTIONAL".into())
            .with_start_date(Date::new(2025, 1, 1))
            .with_maturity_date(Date::new(2027, 1, 1))
            .with_notional(100.0)
            .with_currency(Currency::USD)
            .with_coupons(vec![CashflowType::fixed(0.04)])
            .with_notional_percentages(vec![1.0, 0.8, 0.9])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn rejects_call_dates_outside_the_bond_life() {
        for call_date in [Date::new(2025, 1, 1), Date::new(2027, 1, 1)] {
            let result = MakeSteppedCouponBond::<DualFwd>::default()
                .with_identifier("BAD-CALL-DATE".into())
                .with_start_date(Date::new(2025, 1, 1))
                .with_maturity_date(Date::new(2027, 1, 1))
                .with_notional(100.0)
                .with_currency(Currency::USD)
                .with_coupons(vec![CashflowType::fixed(0.04)])
                .with_notional_percentages(vec![1.0])
                .with_call_schedule(vec![CallScheduleEntry::new(call_date, 100.0)])
                .build();
            assert!(result.is_err());
        }
    }

    #[test]
    fn validates_call_windows() {
        let build = |calls| {
            MakeSteppedCouponBond::<DualFwd>::default()
                .with_identifier("CALL-WINDOW-VALIDATION".into())
                .with_start_date(Date::new(2025, 1, 1))
                .with_maturity_date(Date::new(2027, 1, 1))
                .with_notional(100.0)
                .with_currency(Currency::USD)
                .with_coupons(vec![CashflowType::fixed(0.04)])
                .with_notional_percentages(vec![1.0])
                .with_call_schedule(calls)
                .build()
        };
        let first = Date::new(2026, 1, 1);
        let middle = Date::new(2026, 6, 1);
        let last = Date::new(2026, 12, 1);
        for end in [
            Date::new(2027, 1, 1),
            Date::new(2027, 2, 1),
            Date::new(2025, 12, 1),
        ] {
            assert!(build(vec![CallScheduleEntry::window(first, end, 100.0)]).is_err());
        }
        for second_start in [first, Date::new(2026, 5, 1), middle] {
            assert!(build(vec![
                CallScheduleEntry::window(second_start, last, 100.0),
                CallScheduleEntry::window(first, middle, 101.0),
            ])
            .is_err());
        }
        assert!(build(vec![
            CallScheduleEntry::new(last, 100.0),
            CallScheduleEntry::window(first, middle, 101.0),
        ])
        .is_ok());
    }

    #[test]
    fn floating_floor_and_cap_bound_the_all_in_rate() -> Result<()> {
        let bond = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("COLLARED-FLOAT".into())
            .with_start_date(Date::new(2025, 1, 1))
            .with_maturity_date(Date::new(2026, 1, 1))
            .with_notional(100.0)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Annual)
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::SOFR,
                0.01,
                Some(0.02),
                Some(0.05),
            )])
            .with_notional_percentages(vec![1.0])
            .build()?;
        let coupon = bond
            .leg()
            .cashflows()
            .iter()
            .find_map(|cashflow| match cashflow {
                CashflowType::OptionEmbeddedCoupon(coupon) => Some(coupon),
                _ => None,
            })
            .ok_or_else(|| QSError::NotFoundErr("collared coupon".into()))?;
        let capped = coupon
            .payoff_ops()
            .eval(DualFwd::from(0.10))?
            .add_val(coupon.spread())
            .value();
        let floored = coupon
            .payoff_ops()
            .eval(DualFwd::from(-0.10))?
            .add_val(coupon.spread())
            .value();
        assert!((capped - 0.05).abs() < 1.0e-12);
        assert!((floored - 0.02).abs() < 1.0e-12);
        Ok(())
    }
}
