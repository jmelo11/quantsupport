#[cfg(test)]
use crate::ad::dual::DualFwd;
use crate::{
    ad::scalar::Scalar,
    core::{
        makeinstrument::{resolve_date, MakeInstrument},
        trade::Side,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, RateType},
        rates::basisswap::BasisSwap,
    },
    time::{
        calendar::Calendar,
        calendars::nullcalendar::NullCalendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
        period::Period,
    },
    utils::errors::{QSError, Result},
};
use std::marker::PhantomData;

/// A builder for creating a [`BasisSwap`] instance (floating-vs-floating interest rate swap).
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let basis_swap = MakeBasisSwap::<DualFwd>::default()
///     .with_identifier("BASIS-3M6M".to_string())
///     .with_start_date(Date::new(2024, 1, 1))
///     .with_maturity_date(Date::new(2026, 1, 1))
///     .with_notional(5_000_000.0)
///     .with_pay_market_index(MarketIndex::SOFR)
///     .with_receive_market_index(MarketIndex::TermSOFR3m)
///     .with_currency(Currency::USD)
///     .with_pay_spread(0.001)
///     .with_receive_spread(0.0)
///     .build()
///     .expect("failed to build basis swap");
///
/// assert_eq!(basis_swap.identifier(), "BASIS-3M6M");
/// assert!(!basis_swap.pay_leg().cashflows().is_empty());
/// assert!(!basis_swap.receive_leg().cashflows().is_empty());
/// ```
#[derive(Clone, Default)]
pub struct MakeBasisSwap<T: Scalar> {
    start_tenor: Option<Period>,
    maturity_tenor: Option<Period>,
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    notional: Option<f64>,
    pay_spread: Option<f64>,
    receive_spread: Option<f64>,
    identifier: Option<String>,
    pay_market_index: Option<MarketIndex>,
    receive_market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    pay_leg_frequency: Option<Frequency>,
    receive_leg_frequency: Option<Frequency>,
    pay_day_counter: Option<DayCounter>,
    receive_day_counter: Option<DayCounter>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    _marker: PhantomData<T>,
}

impl<T> MakeBasisSwap<T>
where
    T: Scalar,
{
    /// Sets start date as a tenor from the context evaluation date.
    /// Replaces an explicitly supplied start date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_tenor = Some(tenor);
        self.start_date = None;
        self
    }

    /// Sets maturity date as a tenor from the resolved start date.
    /// Replaces an explicitly supplied maturity date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_maturity_tenor(mut self, tenor: Period) -> Self {
        self.maturity_tenor = Some(tenor);
        self.maturity_date = None;
        self
    }

    /// Sets maturity as a tenor from the resolved start date.
    #[must_use]
    pub const fn with_tenor(self, tenor: Period) -> Self {
        self.with_maturity_tenor(tenor)
    }

    /// Sets pay day counter for coupon accrual without changing index quoting.
    #[must_use]
    pub const fn with_pay_day_counter(mut self, dc: DayCounter) -> Self {
        self.pay_day_counter = Some(dc);
        self
    }

    /// Sets receive day counter for coupon accrual without changing index quoting.
    #[must_use]
    pub const fn with_receive_day_counter(mut self, dc: DayCounter) -> Self {
        self.receive_day_counter = Some(dc);
        self
    }

    /// Sets the start date.
    #[must_use]
    pub const fn with_start_date(mut self, date: Date) -> Self {
        self.start_date = Some(date);
        self.start_tenor = None;
        self
    }

    /// Sets the maturity date.
    #[must_use]
    pub const fn with_maturity_date(mut self, date: Date) -> Self {
        self.maturity_date = Some(date);
        self.maturity_tenor = None;
        self
    }

    /// Sets the notional amount.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the spread on the pay leg.
    #[must_use]
    pub const fn with_pay_spread(mut self, spread: f64) -> Self {
        self.pay_spread = Some(spread);
        self
    }

    /// Sets the spread on the receive leg.
    #[must_use]
    pub const fn with_receive_spread(mut self, spread: f64) -> Self {
        self.receive_spread = Some(spread);
        self
    }

    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the market index for the pay leg.
    #[must_use]
    pub fn with_pay_market_index(mut self, idx: MarketIndex) -> Self {
        self.pay_market_index = Some(idx);
        self
    }

    /// Sets the market index for the receive leg.
    #[must_use]
    pub fn with_receive_market_index(mut self, idx: MarketIndex) -> Self {
        self.receive_market_index = Some(idx);
        self
    }

    /// Sets the currency of the swap.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Stores a compatibility side that `build` does not use; the generated
    /// pay/receive legs retain their prescribed directions.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the pay leg payment frequency.
    #[must_use]
    pub const fn with_pay_leg_frequency(mut self, freq: Frequency) -> Self {
        self.pay_leg_frequency = Some(freq);
        self
    }

    /// Sets the receive leg payment frequency.
    #[must_use]
    pub const fn with_receive_leg_frequency(mut self, freq: Frequency) -> Self {
        self.receive_leg_frequency = Some(freq);
        self
    }

    /// Sets the calendar for business day adjustments.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Sets the business day convention.
    #[must_use]
    pub const fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Sets the date generation rule.
    #[must_use]
    pub const fn with_date_generation_rule(mut self, rule: DateGenerationRule) -> Self {
        self.date_generation_rule = Some(rule);
        self
    }

    /// Sets the end-of-month flag.
    #[must_use]
    pub const fn with_end_of_month(mut self, eom: bool) -> Self {
        self.end_of_month = Some(eom);
        self
    }
}

impl<T: Scalar> MakeInstrument for MakeBasisSwap<T> {
    type Instrument = BasisSwap<T>;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none() && self.maturity_tenor.is_none()
    }

    /// Builds the [`BasisSwap`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeBasisSwap has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if [self.start_date, self.maturity_date].contains(&Some(Date::empty())) {
            return Err(QSError::InvalidValueErr(
                "MakeBasisSwap dates must not be empty".into(),
            ));
        }
        let notional = self
            .notional
            .ok_or_else(|| QSError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| QSError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| QSError::ValueNotSetErr("Maturity date".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let pay_market_index = self
            .pay_market_index
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Pay market index".into()))?;
        let receive_market_index = self
            .receive_market_index
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Receive market index".into()))?;
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;

        let pay_spread = self.pay_spread.unwrap_or(0.0);
        let receive_spread = self.receive_spread.unwrap_or(0.0);
        let pay_freq = self.pay_leg_frequency.unwrap_or(Frequency::Quarterly);
        let receive_freq = self.receive_leg_frequency.unwrap_or(Frequency::Quarterly);

        // Pay leg
        let mut pay_leg = MakeLeg::<T>::default()
            .with_leg_id(0)
            .with_notional(notional)
            .with_side(Side::PayShort)
            .with_currency(currency)
            .with_forward_index(pay_market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(pay_spread)
            .with_payment_frequency(pay_freq)
            .bullet()
            .with_calendar(self.calendar.clone())
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        // Receive leg
        let mut receive_leg = MakeLeg::<T>::default()
            .with_leg_id(1)
            .with_notional(notional)
            .with_side(Side::LongReceive)
            .with_currency(currency)
            .with_forward_index(receive_market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(receive_spread)
            .with_payment_frequency(receive_freq)
            .bullet()
            .with_calendar(self.calendar.clone())
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        if let Some(dc) = self.pay_day_counter {
            pay_leg = pay_leg.with_floating_day_counter(dc);
        }
        if let Some(dc) = self.receive_day_counter {
            receive_leg = receive_leg.with_floating_day_counter(dc);
        }

        Ok(BasisSwap::new(
            identifier,
            pay_leg,
            receive_leg,
            pay_market_index,
            receive_market_index,
            currency,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let calendar = self
            .calendar
            .clone()
            .unwrap_or_else(|| Calendar::NullCalendar(NullCalendar::new()));
        let convention = self
            .business_day_convention
            .unwrap_or(BusinessDayConvention::Unadjusted);
        let date_calendar = &calendar;
        let date_convention = convention;
        let date_end_of_month = self.end_of_month.unwrap_or(false);
        resolved.start_date = Some(resolve_date(
            self.start_date,
            self.start_tenor,
            reference_date,
            "start_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.start_tenor = None;
        resolved.maturity_date = Some(resolve_date(
            self.maturity_date,
            self.maturity_tenor,
            resolved
                .start_date
                .ok_or_else(|| QSError::ValueNotSetErr("start_date".into()))?,
            "maturity_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.maturity_tenor = None;
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::instrument::Instrument;

    fn base_builder() -> MakeBasisSwap<DualFwd> {
        MakeBasisSwap::<DualFwd>::default()
            .with_identifier("basis_swap_test".to_string())
            .with_start_date(Date::new(2024, 1, 1))
            .with_maturity_date(Date::new(2025, 1, 1))
            .with_notional(1_000_000.0)
            .with_pay_market_index(MarketIndex::SOFR)
            .with_receive_market_index(MarketIndex::TermSOFR3m)
            .with_currency(Currency::USD)
    }

    #[test]
    fn test_build_basis_swap_success() {
        let result = base_builder().build();
        assert!(result.is_ok(), "expected basis swap build to succeed");

        let swap = result.unwrap();
        assert_eq!(swap.identifier(), "basis_swap_test");
        assert_eq!(swap.currency(), Currency::USD);
        assert_eq!(swap.pay_forward_index(), MarketIndex::SOFR);
        assert_eq!(swap.receive_forward_index(), MarketIndex::TermSOFR3m);
        assert!(!swap.pay_leg().cashflows().is_empty());
        assert!(!swap.receive_leg().cashflows().is_empty());
    }

    #[test]
    fn test_build_basis_swap_missing_receive_index_fails() {
        let result = MakeBasisSwap::<DualFwd>::default()
            .with_identifier("basis_swap_missing_receive".to_string())
            .with_start_date(Date::new(2024, 1, 1))
            .with_maturity_date(Date::new(2025, 1, 1))
            .with_notional(1_000_000.0)
            .with_pay_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .build();

        assert!(
            result.is_err(),
            "expected missing receive market index to fail"
        );
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{assert_legs, empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn basis_swap_resolves_dates_before_building_legs() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let end = reference() + p("2Y")?;
        let basis = MakeBasisSwap::<f64>::default()
            .with_identifier("basis".into())
            .with_start_tenor(p("0D")?)
            .with_tenor(p("2Y")?)
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_pay_market_index(MarketIndex::EURIBOR3m)
            .with_receive_market_index(MarketIndex::EURIBOR6m);
        assert_legs(
            &basis.build_using(ctx)?,
            &basis
                .with_start_date(reference())
                .with_maturity_date(end)
                .build()?,
        )?;
        Ok(())
    }
}
