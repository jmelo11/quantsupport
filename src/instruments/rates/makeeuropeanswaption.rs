use crate::{
    ad::scalar::Scalar,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::rates::{
        europeanswaption::{EuropeanSwaption, SwaptionType},
        makeswap::MakeSwap,
    },
    rates::interestrate::RateDefinition,
    time::{
        calendar::Calendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{QSError, Result},
};
use crate::{
    core::resolution::{ResolutionContext, Resolve},
    time::period::Period,
};
use std::marker::PhantomData;

/// A builder for creating an [`EuropeanSwaption`] instance.
///
/// The builder first constructs the underlying swap (via [`MakeSwap`]) and then
/// wraps it with the option-specific parameters (expiry, type, strike).
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
/// let swaption = MakeSwaption::<f64>::default()
///     .with_identifier("SWPTN-5Y10Y".to_string())
///     .with_expiry(Date::new(2024, 6, 1))
///     .with_swap_tenor_date(Date::new(2034, 6, 1))
///     .with_strike(0.03)
///     .with_notional(10_000_000.0)
///     .with_rate_definition(rate_def)
///     .with_market_index(MarketIndex::SOFR)
///     .with_currency(Currency::USD)
///     .with_swaption_type(SwaptionType::Payer)
///     .build()
///     .expect("failed to build swaption");
///
/// assert_eq!(swaption.identifier(), "SWPTN-5Y10Y");
/// assert_eq!(swaption.strike(), 0.03);
/// ```
#[derive(Clone, Default)]
pub struct MakeSwaption<T: Scalar + Default> {
    start_date: Option<Date>,
    start_tenor: Option<Period>,
    swap_tenor_date: Option<Date>,
    swap_tenor: Option<Period>,
    expiry: Option<Date>,
    expiry_tenor: Option<Period>,
    strike: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    rate_definition: Option<RateDefinition>,
    market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    swaption_type: Option<SwaptionType>,
    fixed_leg_frequency: Option<Frequency>,
    floating_leg_frequency: Option<Frequency>,
    floating_day_counter: Option<DayCounter>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    _marker: PhantomData<T>,
}

impl<T> MakeSwaption<T>
where
    T: Scalar + Default,
{
    /// Sets swap tenor date as a tenor from the resolved underlying start date.
    #[must_use]
    pub const fn with_swap_tenor(mut self, tenor: Period) -> Self {
        self.swap_tenor_date = None;
        self.swap_tenor = Some(tenor);
        self
    }

    /// Sets start date as a tenor from the resolved expiry.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_date = None;
        self.start_tenor = Some(tenor);
        self
    }

    /// Sets expiry as a tenor from the resolution reference date.
    #[must_use]
    pub const fn with_expiry_tenor(mut self, tenor: Period) -> Self {
        self.expiry = None;
        self.expiry_tenor = Some(tenor);
        self
    }

    /// Sets the start date of the underlying swap (= option expiry for Europeans).
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self.start_tenor = None;
        self
    }

    /// Sets the maturity date of the underlying swap.
    #[must_use]
    pub const fn with_swap_tenor_date(mut self, date: Date) -> Self {
        self.swap_tenor_date = Some(date);
        self.swap_tenor = None;
        self
    }

    /// Sets the option expiry date.
    #[must_use]
    pub const fn with_expiry(mut self, expiry: Date) -> Self {
        self.expiry = Some(expiry);
        self.expiry_tenor = None;
        self
    }

    /// Sets the strike (fixed rate of the underlying swap).
    #[must_use]
    pub const fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Sets the notional amount.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets floating day counter for coupon accrual without changing index quoting.
    #[must_use]
    pub const fn with_floating_day_counter(mut self, dc: DayCounter) -> Self {
        self.floating_day_counter = Some(dc);
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

    /// Sets the currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the swaption type (payer or receiver).
    #[must_use]
    pub const fn with_swaption_type(mut self, swaption_type: SwaptionType) -> Self {
        self.swaption_type = Some(swaption_type);
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

    /// Builds the [`EuropeanSwaption`] instance.
    ///
    /// # Errors
    /// Returns an error when required fields are missing or the underlying
    /// swap builder fails.
    pub fn build(self) -> Result<EuropeanSwaption<T>> {
        if self.expiry_tenor.is_some() || self.start_tenor.is_some() || self.swap_tenor.is_some() {
            return Err(QSError::InvalidValueErr(
                "Relative dates require maker.resolve(&ResolutionContext)".into(),
            ));
        }
        let strike = self
            .strike
            .ok_or_else(|| QSError::ValueNotSetErr("Strike".into()))?;
        let expiry = self
            .expiry
            .ok_or_else(|| QSError::ValueNotSetErr("Expiry".into()))?;
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let market_index = self
            .market_index
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let swaption_type = self.swaption_type.unwrap_or(SwaptionType::Payer);

        // The underlying swap starts at expiry (European swaption).
        let swap_start = self.start_date.unwrap_or(expiry);
        let swap_maturity = self
            .swap_tenor_date
            .ok_or_else(|| QSError::ValueNotSetErr("Swap tenor date".into()))?;

        // Build the underlying swap via MakeSwap.
        let mut swap_builder = MakeSwap::<T>::default()
            .with_identifier(format!("{identifier}_underlying"))
            .with_start_date(swap_start)
            .with_maturity_date(swap_maturity)
            .with_fixed_rate(strike)
            .with_notional(
                self.notional
                    .ok_or_else(|| QSError::ValueNotSetErr("Notional".into()))?,
            )
            .with_market_index(market_index.clone())
            .with_currency(currency);

        if let Some(rd) = self.rate_definition {
            swap_builder = swap_builder.with_rate_definition(rd);
        }
        if let Some(f) = self.fixed_leg_frequency {
            swap_builder = swap_builder.with_fixed_leg_frequency(f);
        }
        if let Some(f) = self.floating_leg_frequency {
            swap_builder = swap_builder.with_floating_leg_frequency(f);
        }
        if let Some(c) = self.calendar {
            swap_builder = swap_builder.with_calendar(c);
        }
        if let Some(bdc) = self.business_day_convention {
            swap_builder = swap_builder.with_business_day_convention(bdc);
        }
        if let Some(dgr) = self.date_generation_rule {
            swap_builder = swap_builder.with_date_generation_rule(dgr);
        }
        if let Some(eom) = self.end_of_month {
            swap_builder = swap_builder.with_end_of_month(eom);
        }

        if let Some(dc) = self.floating_day_counter {
            swap_builder = swap_builder.with_floating_day_counter(dc);
        }
        let underlying = swap_builder.build()?;

        Ok(EuropeanSwaption::new(
            identifier,
            underlying,
            expiry,
            swaption_type,
            strike,
            market_index,
            currency,
        ))
    }
}

impl<T: Scalar> Resolve for MakeSwaption<T> {
    type Resolved = EuropeanSwaption<T>;

    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<Self::Resolved> {
        let mut resolved = self.clone();
        let details = self
            .market_index
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?
            .rate_index_details()?;
        let calendar = self.calendar.clone().unwrap_or_else(|| details.calendar());
        let convention = self.business_day_convention.unwrap_or_else(|| {
            details
                .swap_conventions(
                    self.swap_tenor
                        .unwrap_or_else(|| Period::new(1, crate::time::enums::TimeUnit::Years)),
                )
                .business_day_convention()
        });
        let dates = context.with_date_conventions(
            &calendar,
            convention,
            self.end_of_month.unwrap_or(false),
        );
        let expiry = dates.resolve_date(
            self.expiry,
            self.expiry_tenor,
            context.reference_date(),
            "Expiry",
        )?;
        let start = if self.start_date.is_some() || self.start_tenor.is_some() {
            dates.resolve_date(
                self.start_date,
                self.start_tenor,
                expiry,
                "Underlying start",
            )?
        } else {
            expiry
        };
        let maturity = dates.resolve_date(
            self.swap_tenor_date,
            self.swap_tenor,
            start,
            "Underlying maturity",
        )?;
        resolved.expiry = Some(expiry);
        resolved.start_date = Some(start);
        resolved.swap_tenor_date = Some(maturity);
        resolved.expiry_tenor = None;
        resolved.start_tenor = None;
        resolved.swap_tenor = None;
        resolved.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ad::dual::DualFwd,
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

    fn base_builder() -> MakeSwaption<DualFwd> {
        MakeSwaption::<DualFwd>::default()
            .with_identifier("swaption_test".to_string())
            .with_expiry(Date::new(2024, 6, 1))
            .with_swap_tenor_date(Date::new(2026, 6, 1))
            .with_strike(0.03)
            .with_notional(1_000_000.0)
            .with_rate_definition(sample_rate_definition())
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_swaption_type(SwaptionType::Payer)
    }

    #[test]
    fn test_build_swaption_success() {
        let result = base_builder().build();
        assert!(result.is_ok(), "expected swaption build to succeed");

        let swaption = result.unwrap();
        assert_eq!(swaption.identifier(), "swaption_test");
        assert_eq!(swaption.currency(), Currency::USD);
        assert_eq!(swaption.market_index(), MarketIndex::SOFR);
        assert_eq!(swaption.strike(), 0.03);
    }

    #[test]
    fn test_build_swaption_missing_swap_tenor_fails() {
        let result = MakeSwaption::<DualFwd>::default()
            .with_identifier("swaption_missing_tenor".to_string())
            .with_expiry(Date::new(2024, 6, 1))
            .with_strike(0.03)
            .with_notional(1_000_000.0)
            .with_rate_definition(sample_rate_definition())
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .build();

        assert!(result.is_err(), "expected missing swap tenor to fail");
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::resolution::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn swaption_resolves_expiry_and_underlying_tenor() -> Result<()> {
        let market = empty_market(reference());
        let ctx = ResolutionContext::new(&market);
        let swaption = MakeSwaption::<f64>::default()
            .with_identifier("swaption".into())
            .with_expiry_tenor(p("1Y")?)
            .with_swap_tenor(p("5Y")?)
            .with_notional(100_000.0)
            .with_strike(0.03)
            .with_currency(Currency::EUR)
            .with_market_index(MarketIndex::EURIBOR6m)
            .with_business_day_convention(BusinessDayConvention::Unadjusted)
            .resolve(&ctx)?;
        assert_eq!(swaption.expiry_date(), reference() + p("1Y")?);
        assert_eq!(
            swaption.underlying().fixed_leg().last_payment_date(),
            reference() + p("6Y")?
        );
        Ok(())
    }
}
