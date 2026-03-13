use crate::{
    ad::adreal::IsReal,
    core::trade::Side,
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
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{QSError, Result},
};
#[cfg(test)]
use crate::ad::adreal::ADReal;
use std::marker::PhantomData;

/// A builder for creating a [`Swap`] instance (vanilla fixed-float interest rate swap).
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
/// let swap = MakeSwap::<ADReal>::default()
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
#[derive(Default)]
pub struct MakeSwap<T: IsReal> {
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
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    _marker: PhantomData<T>,
}

impl<T> MakeSwap<T>
where
    T: IsReal,
{
    /// Sets the start date.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the maturity date.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
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

    /// Builds the [`Swap`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<Swap<T>> {
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
        let rate_definition = self
            .rate_definition
            .ok_or_else(|| QSError::ValueNotSetErr("Rate definition".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?;
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;

        let side = self.side.unwrap_or(Side::LongReceive);
        let spread = self.spread.unwrap_or(0.0);
        let fixed_leg_frequency = self.fixed_leg_frequency.unwrap_or(Frequency::Semiannual);
        let floating_leg_frequency = self.floating_leg_frequency.unwrap_or(Frequency::Quarterly);

        let interest_rate = InterestRate::from_rate_definition(T::new(fixed_rate), rate_definition);

        // Fixed leg: receive side matches the swap side
        let fixed_leg = MakeLeg::<T>::default()
            .with_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_currency(currency)
            .with_market_index(market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Fixed)
            .with_rate(interest_rate)
            .with_payment_frequency(fixed_leg_frequency)
            .bullet()
            .with_calendar(self.calendar.clone())
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        // Floating leg: opposite side
        let floating_side = match side {
            Side::LongReceive => Side::PayShort,
            Side::PayShort => Side::LongReceive,
        };

        let floating_leg = MakeLeg::<T>::default()
            .with_leg_id(1)
            .with_notional(notional)
            .with_side(floating_side)
            .with_currency(currency)
            .with_market_index(market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(spread)
            .with_payment_frequency(floating_leg_frequency)
            .bullet()
            .with_calendar(self.calendar)
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        Ok(Swap::new(
            identifier,
            fixed_leg,
            floating_leg,
            market_index,
            currency,
        ))
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

    fn base_builder() -> MakeSwap<ADReal> {
        MakeSwap::<ADReal>::default()
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
        assert_eq!(swap.market_index(), MarketIndex::SOFR);
        assert!(!swap.fixed_leg().cashflows().is_empty());
        assert!(!swap.floating_leg().cashflows().is_empty());
    }

    #[test]
    fn test_build_swap_missing_fixed_rate_fails() {
        let result = MakeSwap::<ADReal>::default()
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
}
