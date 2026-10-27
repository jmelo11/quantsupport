use crate::{
    ad::scalar::Scalar,
    core::{instrument::AssetClass, makeinstrument::MakeInstrument, trade::Side},
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{cashflowtype::CashflowType, leg::Leg},
        rates::{oisconventions::OisConventions, overnightindexswap::OvernightIndexSwap},
    },
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
    },
    time::{
        calendars::traits::IsCalendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency, TimeUnit},
        period::Period,
        schedule::MakeSchedule,
    },
    utils::errors::{QSError, Result},
};

/// Builder for fixed versus overnight swaps without principal exchanges.
///
/// Overnight conventions select daily compounding or simple averaging.
#[derive(Clone)]
pub struct MakeOvernightIndexSwap<T: Scalar> {
    index: MarketIndex,
    reference_date: Option<Date>,
    start_tenor: Option<Period>,
    first_coupon_tenor: Option<Period>,
    next_to_last_coupon_tenor: Option<Period>,

    tenor: Period,
    fixed_rate: f64,
    notional: f64,
    spread: f64,
    identifier: String,
    side: Side,
    effective_date: Option<Date>,
    termination_date: Option<Date>,
    conventions: OisConventions,
    marker: std::marker::PhantomData<T>,
}

impl<T: Scalar> MakeOvernightIndexSwap<T> {
    /// Creates an OIS builder using the common convention preset for the index.
    #[must_use]
    pub fn new(index: MarketIndex, reference_date: Date, tenor: Period, fixed_rate: f64) -> Self {
        Self::from_tenor(index, tenor, fixed_rate).with_reference_date(reference_date)
    }

    /// Defines an OIS whose reference date is supplied when it is resolved.
    /// Settlement lag and forward-start conventions are applied during resolution.
    #[must_use]
    pub fn from_tenor(index: MarketIndex, tenor: Period, fixed_rate: f64) -> Self {
        let conventions = OisConventions::for_index(&index);
        Self {
            index,
            reference_date: None,
            start_tenor: None,
            first_coupon_tenor: None,
            next_to_last_coupon_tenor: None,

            tenor,
            fixed_rate,
            notional: 1.0,
            spread: 0.0,
            identifier: "OIS".into(),
            side: Side::LongReceive,
            effective_date: None,
            termination_date: None,
            conventions,
            marker: std::marker::PhantomData,
        }
    }

    /// Anchors settlement rules to an explicit reference date.
    #[must_use]
    pub const fn with_reference_date(mut self, date: Date) -> Self {
        self.reference_date = Some(date);
        self
    }

    /// Sets the effective date as a tenor from the context evaluation date.
    /// This replaces the settlement-lag and forward-start calculation.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_tenor = Some(tenor);
        self.effective_date = None;
        self
    }

    /// Sets maturity as a tenor from the effective date.
    #[must_use]
    pub const fn with_tenor(mut self, tenor: Period) -> Self {
        self.tenor = tenor;
        self.termination_date = None;
        self
    }

    /// Sets maturity as a tenor from the effective date.
    #[must_use]
    pub const fn with_maturity_tenor(self, tenor: Period) -> Self {
        self.with_tenor(tenor)
    }

    /// Sets the overnight coupon accrual day count independently of native index quotes.
    #[must_use]
    pub fn with_floating_day_counter(mut self, dc: DayCounter) -> Self {
        self.conventions = self.conventions.with_floating_day_counter(dc);
        self
    }

    /// Replaces the convention preset.
    #[must_use]
    pub fn with_conventions(mut self, conventions: OisConventions) -> Self {
        self.conventions = conventions;
        self.first_coupon_tenor = None;
        self.next_to_last_coupon_tenor = None;

        self
    }

    /// Sets the trade identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = identifier;
        self
    }

    /// Sets the notional.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = notional;
        self
    }

    /// Sets the overnight-leg spread.
    #[must_use]
    pub const fn with_spread(mut self, spread: f64) -> Self {
        self.spread = spread;
        self
    }

    /// Sets the fixed-leg side.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Sets an explicit effective date instead of applying settlement and forward-start rules.
    #[must_use]
    pub const fn with_start_date(mut self, date: Date) -> Self {
        self.effective_date = Some(date);
        self.start_tenor = None;
        self
    }

    /// Sets an explicit termination date instead of applying the tenor.
    #[must_use]
    pub const fn with_maturity_date(mut self, date: Date) -> Self {
        self.termination_date = Some(date);
        self
    }

    /// Sets first coupon date as a tenor from the effective date.
    #[must_use]
    pub const fn with_first_coupon_tenor(mut self, tenor: Period) -> Self {
        self.first_coupon_tenor = Some(tenor);
        self
    }

    /// Sets an explicit first coupon date.
    #[must_use]
    pub fn with_first_coupon_date(mut self, date: Date) -> Self {
        self.conventions = self.conventions.with_first_coupon_date(date);
        self.first_coupon_tenor = None;
        self
    }

    /// Sets next to last coupon date as a tenor from maturity; negative tenors move backwards.
    #[must_use]
    pub const fn with_next_to_last_coupon_tenor(mut self, tenor: Period) -> Self {
        self.next_to_last_coupon_tenor = Some(tenor);
        self
    }

    /// Sets an explicit next to last coupon date.
    #[must_use]
    pub fn with_next_to_last_coupon_date(mut self, date: Date) -> Self {
        self.conventions = self.conventions.with_next_to_last_coupon_date(date);
        self.next_to_last_coupon_tenor = None;
        self
    }

    fn schedule_dates(&self, reference_date: Date) -> Result<(Date, Date, bool)> {
        let details = self.index.rate_index_details()?;
        let conventions = &self.conventions;
        let index_calendar = details.calendar();
        let overnight_calendar = conventions.overnight_calendar().unwrap_or(&index_calendar);
        let settlement_calendar = conventions.settlement_calendar().unwrap_or(&index_calendar);
        let spot = settlement_calendar.advance(
            reference_date,
            Period::new(conventions.settlement_days(), TimeUnit::Days),
            None,
            false,
        )?;
        let forward_start = conventions
            .forward_start()
            .unwrap_or(Period::new(0, TimeUnit::Days));
        let start = self.effective_date.unwrap_or_else(|| {
            overnight_calendar.adjust(
                spot + forward_start,
                Some(if forward_start.length() < 0 {
                    BusinessDayConvention::Preceding
                } else {
                    BusinessDayConvention::Following
                }),
            )
        });
        let eom = conventions
            .end_of_month()
            .unwrap_or_else(|| overnight_calendar.is_end_of_month(&start));
        let end = self.termination_date.unwrap_or_else(|| {
            let maturity = start + self.tenor;
            if eom && matches!(self.tenor.units(), TimeUnit::Months | TimeUnit::Years) {
                overnight_calendar.end_of_month(maturity)
            } else {
                maturity
            }
        });
        Ok((start, end, eom))
    }
}

impl<T: Scalar> MakeInstrument for MakeOvernightIndexSwap<T> {
    type Instrument = OvernightIndexSwap<T>;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none()
            && self.first_coupon_tenor.is_none()
            && self.next_to_last_coupon_tenor.is_none()
            && self
                .reference_date
                .or(self.effective_date)
                .is_some_and(|date| date != Date::empty())
    }

    /// Builds the OIS and its independent fixed and overnight schedules.
    ///
    /// # Errors
    /// Rejects invalid notionals, dates, lags, and non-overnight indices.
    #[allow(clippy::too_many_lines)]
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeOvernightIndexSwap has unresolved date inputs. Supply a reference or effective date and absolute coupon dates, or call maker.build_using(&context)".into(),
            ));
        }
        if [
            self.reference_date,
            self.effective_date,
            self.termination_date,
        ]
        .contains(&Some(Date::empty()))
        {
            return Err(QSError::InvalidValueErr(
                "MakeOvernightIndexSwap dates must not be empty".into(),
            ));
        }
        let reference_date = self.reference_date.or(self.effective_date).ok_or_else(|| {
            QSError::ValueNotSetErr("OIS reference date requires resolution with a context".into())
        })?;
        let details = self.index.rate_index_details()?;
        let conventions = &self.conventions;
        if !details.is_in_arrears() || details.frequency() != Frequency::Daily {
            return Err(QSError::InvalidValueErr(
                "OIS requires a daily overnight rate index".into(),
            ));
        }
        if !self.notional.is_finite()
            || self.notional <= 0.0
            || !self.fixed_rate.is_finite()
            || !self.spread.is_finite()
            || conventions.settlement_days() < 0
            || conventions.payment_lag() < 0
            || conventions.overnight().lookback_days() < 0
        {
            return Err(QSError::InvalidValueErr(
                "Invalid OIS notional, rate, or lag".into(),
            ));
        }
        let index_calendar = details.calendar();
        let overnight_calendar = conventions.overnight_calendar().unwrap_or(&index_calendar);
        let (start, end, eom) = self.schedule_dates(reference_date)?;
        if end <= start {
            return Err(QSError::InvalidValueErr(
                "OIS maturity must follow its effective date".into(),
            ));
        }
        let payment_calendar = conventions.payment_calendar().unwrap_or(&index_calendar);
        let fixed_definition = RateDefinition::new(
            conventions
                .fixed_day_counter()
                .unwrap_or_else(|| details.rate_definition().day_counter()),
            Compounding::Simple,
            conventions.fixed_frequency(),
        );
        let fixed_rate =
            InterestRate::from_rate_definition(T::scalar(self.fixed_rate), fixed_definition);
        let build_leg = |fixed: bool, frequency: Frequency, side: Side| -> Result<Leg<T>> {
            let calendar = if fixed {
                conventions.fixed_calendar().unwrap_or(&index_calendar)
            } else {
                overnight_calendar
            };
            let mut schedule = MakeSchedule::new(start, end)
                .with_frequency(frequency)
                .with_calendar(calendar.clone())
                .with_convention(conventions.convention())
                .with_termination_date_convention(
                    conventions
                        .termination_convention()
                        .unwrap_or_else(|| conventions.convention()),
                )
                .with_rule(conventions.date_generation_rule())
                .end_of_month(eom);
            if let Some(first) = conventions.first_coupon_date() {
                schedule = schedule.with_first_date(first);
            }
            if let Some(last) = conventions.next_to_last_coupon_date() {
                schedule = schedule.with_next_to_last_date(last);
            }
            let schedule = schedule.build()?;
            let template = if fixed {
                CashflowType::<T>::fixed(self.fixed_rate)
            } else {
                CashflowType::<T>::floating(self.index.clone(), self.spread, None, None)
            };
            let mut cashflows = Vec::new();
            let mut payment_dates = Vec::new();
            for dates in schedule.dates().windows(2) {
                let payment = payment_calendar.adjust(
                    payment_calendar.advance(
                        dates[1],
                        Period::new(conventions.payment_lag(), TimeUnit::Days),
                        Some(conventions.payment_convention()),
                        false,
                    )?,
                    Some(conventions.payment_convention()),
                );
                let mut coupon = template.for_period(
                    self.notional,
                    dates[0],
                    dates[1],
                    payment,
                    Some(fixed_definition),
                )?;
                if let CashflowType::FloatingRateCoupon(floating) = &mut coupon {
                    if let Some(dc) = conventions.floating_day_counter() {
                        floating.set_day_counter(dc);
                    }
                    floating.set_overnight_conventions(conventions.overnight().clone())?;
                }
                cashflows.push(coupon);
                payment_dates.push(payment);
            }
            let first_payment = payment_dates
                .first()
                .copied()
                .ok_or_else(|| QSError::InvalidValueErr("Empty OIS schedule".into()))?;
            let last_payment = payment_dates
                .last()
                .copied()
                .ok_or_else(|| QSError::InvalidValueErr("Empty OIS schedule".into()))?;
            Ok(Leg::new(
                usize::from(!fixed),
                cashflows,
                details.currency(),
                Some(self.index.clone()),
                if fixed {
                    None
                } else {
                    Some(self.index.clone())
                },
                if fixed {
                    None
                } else {
                    Some(T::scalar(self.spread))
                },
                if fixed { Some(fixed_rate) } else { None },
                side,
                true,
                AssetClass::InterestRate,
                first_payment,
                last_payment,
            ))
        };
        let opposite_side = match self.side {
            Side::LongReceive => Side::PayShort,
            Side::PayShort => Side::LongReceive,
        };
        let fixed_leg = build_leg(true, conventions.fixed_frequency(), self.side)?;
        let floating_leg = build_leg(false, conventions.overnight_frequency(), opposite_side)?;
        OvernightIndexSwap::new(
            self.identifier.clone(),
            fixed_leg,
            floating_leg,
            self.index.clone(),
            details.currency(),
        )
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let reference = self
            .reference_date
            .or(self.effective_date)
            .unwrap_or(reference_date);
        resolved.reference_date = Some(reference);
        if let Some(tenor) = self.start_tenor {
            let details = self.index.rate_index_details()?;
            let index_calendar = details.calendar();
            let calendar = self
                .conventions
                .overnight_calendar()
                .unwrap_or(&index_calendar);
            let date_calendar = calendar;
            let date_convention = BusinessDayConvention::Following;
            let date_end_of_month = self.conventions.end_of_month().unwrap_or(false);
            resolved.effective_date = Some(date_calendar.advance(
                reference,
                tenor,
                Some(date_convention),
                date_end_of_month,
            )?);
            resolved.start_tenor = None;
        }
        if self.first_coupon_tenor.is_some() || self.next_to_last_coupon_tenor.is_some() {
            let (start, end, eom) = resolved.schedule_dates(reference)?;
            let details = self.index.rate_index_details()?;
            let index_calendar = details.calendar();
            let calendar = self
                .conventions
                .overnight_calendar()
                .unwrap_or(&index_calendar);
            let date_calendar = calendar;
            let date_convention = self.conventions.convention();
            let date_end_of_month = eom;
            if let Some(tenor) = self.first_coupon_tenor {
                resolved.conventions =
                    resolved
                        .conventions
                        .with_first_coupon_date(date_calendar.advance(
                            start,
                            tenor,
                            Some(date_convention),
                            date_end_of_month,
                        )?);
                resolved.first_coupon_tenor = None;
            }
            if let Some(tenor) = self.next_to_last_coupon_tenor {
                resolved.conventions =
                    resolved
                        .conventions
                        .with_next_to_last_coupon_date(date_calendar.advance(
                            end,
                            tenor,
                            Some(date_convention),
                            date_end_of_month,
                        )?);
                resolved.next_to_last_coupon_tenor = None;
            }
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sofr_ois_settlement_and_payment_lag() {
        let swap = MakeOvernightIndexSwap::<f64>::new(
            MarketIndex::SOFR,
            Date::new(2026, 10, 8),
            Period::new(1, TimeUnit::Years),
            0.04,
        )
        .build()
        .unwrap();
        assert_eq!(swap.fixed_leg().cashflows().len(), 1);
        assert_eq!(swap.floating_leg().cashflows().len(), 1);
        let CashflowType::FloatingRateCoupon(coupon) = &swap.floating_leg().cashflows()[0] else {
            panic!("expected overnight coupon")
        };
        assert_eq!(coupon.accrual_start_date(), Date::new(2026, 10, 13));
        assert_eq!(coupon.accrual_end_date(), Date::new(2027, 10, 13));
        assert_eq!(coupon.payment_date(), Date::new(2027, 10, 15));
        assert!(coupon.overnight_conventions().is_some());
    }

    #[test]
    fn ois_payment_lag_does_not_extend_accrual() {
        let conventions = OisConventions::new()
            .with_settlement_days(0)
            .with_payment_lag(2);
        let swap = MakeOvernightIndexSwap::<f64>::new(
            MarketIndex::SOFR,
            Date::new(2026, 1, 2),
            Period::new(1, TimeUnit::Months),
            0.04,
        )
        .with_conventions(conventions)
        .build()
        .unwrap();
        let CashflowType::FloatingRateCoupon(coupon) = &swap.floating_leg().cashflows()[0] else {
            panic!("expected overnight coupon")
        };
        assert_eq!(coupon.accrual_end_date(), Date::new(2026, 2, 2));
        assert_eq!(coupon.payment_date(), Date::new(2026, 2, 4));
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{assert_legs, empty_market, p, reference},
        prelude::*,
        time::calendars::target::TARGET,
    };

    #[test]
    fn ois_resolution_keeps_relative_stubs_on_the_same_settlement_schedule() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let calendar = Calendar::TARGET(TARGET::new());
        let first = calendar
            .advance(
                reference(),
                p("3M")?,
                Some(BusinessDayConvention::ModifiedFollowing),
                false,
            )
            .expect("valid date advancement");
        let end = reference() + p("2Y")?;
        let last = calendar
            .advance(
                end,
                Period::new(-3, TimeUnit::Months),
                Some(BusinessDayConvention::ModifiedFollowing),
                false,
            )
            .expect("valid date advancement");
        let maker = MakeOvernightIndexSwap::<f64>::from_tenor(MarketIndex::ESTR, p("2Y")?, 0.03)
            .with_conventions(
                OisConventions::new()
                    .with_end_of_month(false)
                    .with_fixed_frequency(Frequency::Semiannual)
                    .with_overnight_frequency(Frequency::Semiannual),
            )
            .with_start_tenor(p("0D")?)
            .with_first_coupon_tenor(p("3M")?)
            .with_next_to_last_coupon_tenor(Period::new(-3, TimeUnit::Months));
        let resolved = maker.build_using(ctx)?;
        let explicit = maker
            .with_reference_date(reference())
            .with_start_date(reference())
            .with_maturity_date(end)
            .with_first_coupon_date(first)
            .with_next_to_last_coupon_date(last)
            .build()?;
        assert_legs(&resolved, &explicit)?;
        Ok(())
    }

    #[test]
    fn ois_resolves_dates_before_building_legs() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let ois = MakeOvernightIndexSwap::<f64>::from_tenor(MarketIndex::ESTR, p("2Y")?, 0.03);
        let explicit =
            MakeOvernightIndexSwap::<f64>::new(MarketIndex::ESTR, reference(), p("2Y")?, 0.03)
                .build()?;
        assert_legs(&ois.build_using(ctx)?, &explicit)?;
        assert!(ois.build().is_err());
        Ok(())
    }
}
