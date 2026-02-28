use crate::{
    ad::adreal::{ADReal, IsReal},
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
    utils::errors::{AtlasError, Result},
};

/// A builder for creating a [`Swap`] instance (vanilla fixed-float interest rate swap).
#[derive(Default)]
pub struct MakeSwap {
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
}

impl MakeSwap {
    /// Sets the start date.
    #[must_use]
    pub fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the maturity date.
    #[must_use]
    pub fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self
    }

    /// Sets the fixed leg coupon rate.
    #[must_use]
    pub fn with_fixed_rate(mut self, rate: f64) -> Self {
        self.fixed_rate = Some(rate);
        self
    }

    /// Sets the floating leg spread over the index.
    #[must_use]
    pub fn with_spread(mut self, spread: f64) -> Self {
        self.spread = Some(spread);
        self
    }

    /// Sets the notional amount.
    #[must_use]
    pub fn with_notional(mut self, notional: f64) -> Self {
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
    pub fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
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
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side. `LongRecieve` means receive-fixed / pay-floating;
    /// `PayShort` means pay-fixed / receive-floating.
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the fixed leg payment frequency.
    #[must_use]
    pub fn with_fixed_leg_frequency(mut self, frequency: Frequency) -> Self {
        self.fixed_leg_frequency = Some(frequency);
        self
    }

    /// Sets the floating leg payment frequency.
    #[must_use]
    pub fn with_floating_leg_frequency(mut self, frequency: Frequency) -> Self {
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
    pub fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Sets the date generation rule.
    #[must_use]
    pub fn with_date_generation_rule(mut self, rule: DateGenerationRule) -> Self {
        self.date_generation_rule = Some(rule);
        self
    }

    /// Sets the end-of-month flag.
    #[must_use]
    pub fn with_end_of_month(mut self, eom: bool) -> Self {
        self.end_of_month = Some(eom);
        self
    }

    /// Builds the [`Swap`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<Swap> {
        let notional = self
            .notional
            .ok_or_else(|| AtlasError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Maturity date".into()))?;
        let fixed_rate = self
            .fixed_rate
            .ok_or_else(|| AtlasError::ValueNotSetErr("Fixed rate".into()))?;
        let rate_definition = self
            .rate_definition
            .ok_or_else(|| AtlasError::ValueNotSetErr("Rate definition".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Currency".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;
        let identifier = self
            .identifier
            .ok_or_else(|| AtlasError::ValueNotSetErr("Identifier".into()))?;

        let side = self.side.unwrap_or(Side::LongRecieve);
        let spread = self.spread.unwrap_or(0.0);
        let fixed_leg_frequency = self.fixed_leg_frequency.unwrap_or(Frequency::Semiannual);
        let floating_leg_frequency = self.floating_leg_frequency.unwrap_or(Frequency::Quarterly);

        let interest_rate =
            InterestRate::from_rate_definition(ADReal::new(fixed_rate), rate_definition);

        // Fixed leg: receive side matches the swap side
        let fixed_leg = MakeLeg::default()
            .set_leg_id(0)
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
            Side::LongRecieve => Side::PayShort,
            Side::PayShort => Side::LongRecieve,
        };

        let floating_leg = MakeLeg::default()
            .set_leg_id(1)
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
