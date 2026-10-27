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
        rates::swap::Swap,
    },
    rates::interestrate::{InterestRate, RateDefinition},
    time::{
        calendar::Calendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency, TimeUnit},
        period::Period,
    },
    utils::errors::{QSError, Result},
};
use std::marker::PhantomData;

/// A builder for creating a [`Swap`] instance (vanilla fixed-float interest rate swap).
///
/// Omitted coupon conventions, payment frequencies and calendars use the
/// floating index's swap metadata. Explicit builder values take precedence.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let rate_def = RateDefinition::new(
///     DayCounter::Actual360,
///     Compounding::Simple,
///     Frequency::Semiannual,
/// );
///
/// let swap = MakeSwap::<DualFwd>::default()
///     .with_identifier("IRS-5Y".to_string())
///     .with_start_date(Date::new(2024, 1, 1))
///     .with_maturity_date(Date::new(2029, 1, 1))
///     .with_fixed_rate(0.03)
///     .with_notional(10_000_000.0)
///     .with_rate_definition(rate_def)
///     .with_market_index(MarketIndex::SOFR)
///     .with_currency(Currency::USD)
///     .with_fixed_leg_frequency(Frequency::Semiannual)
///     .with_floating_leg_frequency(Frequency::Quarterly)
///     .build()
///     .expect("failed to build swap");
///
/// assert_eq!(swap.identifier(), "IRS-5Y");
/// assert!(!swap.fixed_leg().cashflows().is_empty());
/// assert!(!swap.floating_leg().cashflows().is_empty());
/// ```
#[derive(Clone, Default)]
pub struct MakeSwap<T: Scalar> {
    start_tenor: Option<Period>,
    maturity_tenor: Option<Period>,
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    fixed_rate: Option<f64>,
    spread: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    rate_definition: Option<RateDefinition>,
    market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    fixed_leg_frequency: Option<Frequency>,
    floating_leg_frequency: Option<Frequency>,
    floating_day_counter: Option<DayCounter>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    termination_date_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    _marker: PhantomData<T>,
}

impl<T> MakeSwap<T>
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

    /// Sets floating day counter for coupon accrual without changing index quoting.
    #[must_use]
    pub const fn with_floating_day_counter(mut self, dc: DayCounter) -> Self {
        self.floating_day_counter = Some(dc);
        self
    }

    /// Sets the start date.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self.start_tenor = None;
        self
    }

    /// Sets the maturity date.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self.maturity_tenor = None;
        self
    }

    /// Sets the fixed leg coupon rate.
    #[must_use]
    pub const fn with_fixed_rate(mut self, rate: f64) -> Self {
        self.fixed_rate = Some(rate);
        self
    }

    /// Sets the floating leg spread over the index.
    #[must_use]
    pub const fn with_spread(mut self, spread: f64) -> Self {
        self.spread = Some(spread);
        self
    }

    /// Sets the notional amount.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the rate definition for the fixed leg.
    #[must_use]
    pub const fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the market index for the floating leg.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the currency of the swap.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side. `LongReceive` means receive-fixed / pay-floating;
    /// `PayShort` means pay-fixed / receive-floating.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the fixed leg payment frequency.
    #[must_use]
    pub const fn with_fixed_leg_frequency(mut self, frequency: Frequency) -> Self {
        self.fixed_leg_frequency = Some(frequency);
        self
    }

    /// Sets the floating leg payment frequency.
    #[must_use]
    pub const fn with_floating_leg_frequency(mut self, frequency: Frequency) -> Self {
        self.floating_leg_frequency = Some(frequency);
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

    /// Sets the business day convention for the final date of both legs.
    #[must_use]
    pub const fn with_termination_date_convention(
        mut self,
        convention: BusinessDayConvention,
    ) -> Self {
        self.termination_date_convention = Some(convention);
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

impl<T: Scalar> MakeInstrument for MakeSwap<T> {
    type Instrument = Swap<T>;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none() && self.maturity_tenor.is_none()
    }

    /// Builds the [`Swap`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    #[allow(clippy::too_many_lines)]
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeSwap has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if [self.start_date, self.maturity_date].contains(&Some(Date::empty())) {
            return Err(QSError::InvalidValueErr(
                "MakeSwap dates must not be empty".into(),
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
        let fixed_rate = self
            .fixed_rate
            .ok_or_else(|| QSError::ValueNotSetErr("Fixed rate".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let market_index = self
            .market_index
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?;
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;

        let side = self.side.unwrap_or(Side::LongReceive);
        let spread = self.spread.unwrap_or(0.0);
        let index_details = market_index.rate_index_details()?;
        let tenor = Period::new(
            i32::try_from(maturity_date - start_date).map_err(|_| {
                QSError::InvalidValueErr("Swap tenor exceeds supported range".into())
            })?,
            TimeUnit::Days,
        );
        let conventions = index_details.swap_conventions(tenor);
        let rate_definition = self
            .rate_definition
            .unwrap_or_else(|| conventions.fixed_rate_definition());
        let fixed_leg_frequency = self
            .fixed_leg_frequency
            .unwrap_or_else(|| conventions.fixed_frequency());
        let floating_leg_frequency = self
            .floating_leg_frequency
            .unwrap_or_else(|| conventions.floating_frequency());
        let floating_day_counter = self
            .floating_day_counter
            .unwrap_or_else(|| conventions.floating_day_counter());
        let calendar = Some(
            self.calendar
                .clone()
                .unwrap_or_else(|| index_details.calendar()),
        );
        let business_day_convention = Some(
            self.business_day_convention
                .unwrap_or_else(|| conventions.business_day_convention()),
        );
        let termination_date_convention =
            self.termination_date_convention.or(business_day_convention);

        let interest_rate =
            InterestRate::from_rate_definition(T::scalar(fixed_rate), rate_definition);

        // Fixed leg: receive side matches the swap side
        let fixed_leg = MakeLeg::<T>::default()
            .with_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_currency(currency)
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Fixed)
            .with_rate(interest_rate)
            .with_payment_frequency(fixed_leg_frequency)
            .bullet()
            .with_calendar(calendar.clone())
            .with_business_day_convention(business_day_convention)
            .with_termination_date_convention(termination_date_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        // Floating leg: opposite side
        let floating_side = match side {
            Side::LongReceive => Side::PayShort,
            Side::PayShort => Side::LongReceive,
        };

        let mut floating_leg = MakeLeg::<T>::default()
            .with_leg_id(1)
            .with_notional(notional)
            .with_side(floating_side)
            .with_currency(currency)
            .with_forward_index(market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(spread)
            .with_payment_frequency(floating_leg_frequency)
            .bullet()
            .with_calendar(calendar)
            .with_business_day_convention(business_day_convention)
            .with_termination_date_convention(termination_date_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        floating_leg = floating_leg.with_floating_day_counter(floating_day_counter);

        Ok(Swap::new(
            identifier,
            fixed_leg,
            floating_leg,
            market_index,
            currency,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let details = self
            .market_index
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?
            .rate_index_details()?;
        let calendar = self.calendar.clone().unwrap_or_else(|| details.calendar());
        let convention_tenor = self
            .maturity_tenor
            .unwrap_or_else(|| Period::new(1, TimeUnit::Years));
        let convention = self.business_day_convention.unwrap_or_else(|| {
            details
                .swap_conventions(convention_tenor)
                .business_day_convention()
        });
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
    use crate::{
        core::instrument::Instrument,
        rates::compounding::Compounding,
        time::{daycounter::DayCounter, enums::Frequency},
    };

    fn sample_rate_definition() -> RateDefinition {
        RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        )
    }

    fn base_builder() -> MakeSwap<DualFwd> {
        MakeSwap::<DualFwd>::default()
            .with_identifier("swap_test".to_string())
            .with_start_date(Date::new(2024, 1, 1))
            .with_maturity_date(Date::new(2025, 1, 1))
            .with_fixed_rate(0.03)
            .with_notional(1_000_000.0)
            .with_rate_definition(sample_rate_definition())
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
    }

    #[test]
    fn test_build_swap_success() {
        let result = base_builder().build();
        assert!(result.is_ok(), "expected swap build to succeed");

        let swap = result.unwrap();
        assert_eq!(swap.identifier(), "swap_test");
        assert_eq!(swap.currency(), Currency::USD);
        assert_eq!(swap.forward_index(), MarketIndex::SOFR);
        assert!(!swap.fixed_leg().cashflows().is_empty());
        assert!(!swap.floating_leg().cashflows().is_empty());
    }

    #[test]
    fn test_build_swap_missing_fixed_rate_fails() {
        let result = MakeSwap::<DualFwd>::default()
            .with_identifier("swap_missing_fixed_rate".to_string())
            .with_start_date(Date::new(2024, 1, 1))
            .with_maturity_date(Date::new(2025, 1, 1))
            .with_notional(1_000_000.0)
            .with_rate_definition(sample_rate_definition())
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .build();

        assert!(result.is_err(), "expected missing fixed rate to fail");
    }

    #[test]
    fn omitted_conventions_use_index_metadata_in_direct_builder() -> Result<()> {
        use crate::instruments::cashflows::{cashflow::Cashflow, cashflowtype::CashflowType};

        let swap = MakeSwap::<f64>::default()
            .with_identifier("euribor_defaults".into())
            .with_start_date(Date::new(2026, 10, 2))
            .with_maturity_date(Date::new(2027, 10, 2))
            .with_fixed_rate(0.03)
            .with_notional(1.0)
            .with_market_index(MarketIndex::EURIBOR6m)
            .with_currency(Currency::EUR)
            .build()?;
        let fixed: Vec<_> = swap
            .fixed_leg()
            .cashflows()
            .iter()
            .filter_map(|cf| match cf {
                CashflowType::FixedRateCoupon(coupon) => Some(coupon),
                _ => None,
            })
            .collect();
        let floating: Vec<_> = swap
            .floating_leg()
            .cashflows()
            .iter()
            .filter_map(|cf| match cf {
                CashflowType::FloatingRateCoupon(coupon) => Some(coupon),
                _ => None,
            })
            .collect();
        assert_eq!(fixed.len(), 1);
        assert_eq!(floating.len(), 2);
        let fixed = fixed[0];
        assert_eq!(fixed.rate().day_counter(), DayCounter::Thirty360);
        // TARGET rolls the Saturday maturity to Monday.
        assert_eq!(fixed.accrual_end_date(), Date::new(2027, 10, 4));
        assert!((fixed.amount()? - 0.03 * 362.0 / 360.0).abs() < 1e-14);
        for coupon in floating {
            assert_eq!(coupon.day_counter(), DayCounter::Actual360);
        }
        Ok(())
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{assert_legs, cashflows, empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn swap_definitions_are_reusable_and_match_absolute_construction() -> Result<()> {
        let first = empty_market(reference());
        let second = empty_market(Date::new(2027, 3, 25));
        let maker = MakeSwap::<f64>::default()
            .with_identifier("swap".into())
            .with_start_tenor(p("1Y")?)
            .with_tenor(p("5Y")?)
            .with_notional(100_000.0)
            .with_fixed_rate(0.03)
            .with_currency(Currency::EUR)
            .with_market_index(MarketIndex::EURIBOR6m)
            .with_business_day_convention(BusinessDayConvention::Unadjusted);
        let a = maker.build_using(&first)?;
        let b = maker.build_using(&second)?;
        let again = maker.build_using(&first)?;
        let explicit = maker
            .clone()
            .with_start_date(Date::new(2027, 2, 24))
            .with_maturity_date(Date::new(2032, 2, 24))
            .build()?;
        assert_legs(&a, &explicit)?;
        assert_legs(&a, &again)?;
        assert_ne!(cashflows(a.fixed_leg())?, cashflows(b.fixed_leg())?);
        assert!(maker.build().is_err());
        Ok(())
    }
}
