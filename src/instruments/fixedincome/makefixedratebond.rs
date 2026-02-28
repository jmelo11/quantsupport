use crate::{
    ad::adreal::{ADReal, IsReal},
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, PaymentStructure, RateType},
        fixedincome::fixedratebond::FixedRateBond,
    },
    rates::interestrate::{InterestRate, RateDefinition},
    time::{
        calendar::Calendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{AtlasError, Result},
};

/// A builder for creating a [`FixedRateBond`] instance, allowing for a flexible and stepwise construction process.
#[derive(Default)]
pub struct MakeFixedRateBond {
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    rate: Option<f64>,
    units: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    rate_definition: Option<RateDefinition>,
    market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    payment_frequency: Option<Frequency>,
    payment_structure: Option<PaymentStructure>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    first_coupon_date: Option<Date>,
}

impl MakeFixedRateBond {
    /// Sets the start date of the bond.
    #[must_use]
    pub fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the maturity date of the bond.
    #[must_use]
    pub fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self
    }

    /// Sets the coupon rate of the bond.
    #[must_use]
    pub fn with_rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the notional amount of the bond.
    #[must_use]
    pub fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the rate definition of the bond.
    #[must_use]
    pub fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the market index associated with the bond.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the currency of the bond.
    #[must_use]
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the units of the bond. Defaults to 100.0 if not set.
    #[must_use]
    pub fn with_units(mut self, units: f64) -> Self {
        self.units = Some(units);
        self
    }

    /// Sets the identifier of the bond.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the side of the bond (defaults to `LongRecieve` if not set).
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the coupon payment frequency (e.g., `Semiannual`, `Quarterly`).
    #[must_use]
    pub fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the payment structure (e.g., `Bullet`, `EqualRedemptions`). Defaults to `Bullet`.
    #[must_use]
    pub fn with_payment_structure(mut self, structure: PaymentStructure) -> Self {
        self.payment_structure = Some(structure);
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

    /// Sets the end-of-month flag for schedule generation.
    #[must_use]
    pub fn with_end_of_month(mut self, eom: bool) -> Self {
        self.end_of_month = Some(eom);
        self
    }

    /// Sets the first coupon date (for long/short first coupon periods).
    #[must_use]
    pub fn with_first_coupon_date(mut self, date: Date) -> Self {
        self.first_coupon_date = Some(date);
        self
    }

    /// Builds the [`FixedRateBond`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<FixedRateBond> {
        let notional = self
            .notional
            .ok_or_else(|| AtlasError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Maturity date".into()))?;
        let rate = self
            .rate
            .ok_or_else(|| AtlasError::ValueNotSetErr("Rate".into()))?;
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

        let units = self.units.unwrap_or(100.0);
        let side = self.side.unwrap_or(Side::LongRecieve);
        let payment_frequency = self.payment_frequency.unwrap_or(Frequency::Semiannual);
        let structure = self.payment_structure.unwrap_or(PaymentStructure::Bullet);

        let interest_rate = InterestRate::from_rate_definition(ADReal::new(rate), rate_definition);

        let leg = MakeLeg::default()
            .set_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_currency(currency)
            .with_market_index(market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Fixed)
            .with_rate(interest_rate)
            .with_payment_frequency(payment_frequency)
            .with_structure(structure)
            .with_calendar(self.calendar)
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .with_first_coupon_date(self.first_coupon_date)
            .build()?;

        Ok(FixedRateBond::new(
            identifier,
            units,
            leg,
            market_index,
            currency,
        ))
    }
}
