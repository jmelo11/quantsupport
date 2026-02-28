use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, RateType},
        rates::floatfloatcrosscurrencyswap::FloatFloatCrossCurrencySwap,
    },
    time::{
        calendar::Calendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{AtlasError, Result},
};

/// A builder for creating a [`FloatFloatCrossCurrencySwap`] instance
/// (both legs floating, each in a different currency).
#[derive(Default)]
pub struct MakeFloatFloatCrossCurrencySwap {
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    domestic_notional: Option<f64>,
    foreign_notional: Option<f64>,
    domestic_spread: Option<f64>,
    foreign_spread: Option<f64>,
    identifier: Option<String>,
    domestic_currency: Option<Currency>,
    foreign_currency: Option<Currency>,
    domestic_market_index: Option<MarketIndex>,
    foreign_market_index: Option<MarketIndex>,
    side: Option<Side>,
    domestic_leg_frequency: Option<Frequency>,
    foreign_leg_frequency: Option<Frequency>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
}

impl MakeFloatFloatCrossCurrencySwap {
    /// Sets the start date.
    #[must_use]
    pub fn with_start_date(mut self, date: Date) -> Self {
        self.start_date = Some(date);
        self
    }

    /// Sets the maturity date.
    #[must_use]
    pub fn with_maturity_date(mut self, date: Date) -> Self {
        self.maturity_date = Some(date);
        self
    }

    /// Sets the domestic notional amount.
    #[must_use]
    pub fn with_domestic_notional(mut self, notional: f64) -> Self {
        self.domestic_notional = Some(notional);
        self
    }

    /// Sets the foreign notional amount.
    #[must_use]
    pub fn with_foreign_notional(mut self, notional: f64) -> Self {
        self.foreign_notional = Some(notional);
        self
    }

    /// Sets the spread on the domestic floating leg.
    #[must_use]
    pub fn with_domestic_spread(mut self, spread: f64) -> Self {
        self.domestic_spread = Some(spread);
        self
    }

    /// Sets the spread on the foreign floating leg.
    #[must_use]
    pub fn with_foreign_spread(mut self, spread: f64) -> Self {
        self.foreign_spread = Some(spread);
        self
    }

    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the domestic currency.
    #[must_use]
    pub fn with_domestic_currency(mut self, currency: Currency) -> Self {
        self.domestic_currency = Some(currency);
        self
    }

    /// Sets the foreign currency.
    #[must_use]
    pub fn with_foreign_currency(mut self, currency: Currency) -> Self {
        self.foreign_currency = Some(currency);
        self
    }

    /// Sets the domestic market index.
    #[must_use]
    pub fn with_domestic_market_index(mut self, idx: MarketIndex) -> Self {
        self.domestic_market_index = Some(idx);
        self
    }

    /// Sets the foreign market index.
    #[must_use]
    pub fn with_foreign_market_index(mut self, idx: MarketIndex) -> Self {
        self.foreign_market_index = Some(idx);
        self
    }

    /// Sets the side (domestic-leg perspective).
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the domestic leg payment frequency.
    #[must_use]
    pub fn with_domestic_leg_frequency(mut self, freq: Frequency) -> Self {
        self.domestic_leg_frequency = Some(freq);
        self
    }

    /// Sets the foreign leg payment frequency.
    #[must_use]
    pub fn with_foreign_leg_frequency(mut self, freq: Frequency) -> Self {
        self.foreign_leg_frequency = Some(freq);
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

    /// Builds the [`FloatFloatCrossCurrencySwap`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<FloatFloatCrossCurrencySwap> {
        let start_date = self
            .start_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Maturity date".into()))?;
        let domestic_notional = self
            .domestic_notional
            .ok_or_else(|| AtlasError::ValueNotSetErr("Domestic notional".into()))?;
        let foreign_notional = self
            .foreign_notional
            .ok_or_else(|| AtlasError::ValueNotSetErr("Foreign notional".into()))?;
        let domestic_currency = self
            .domestic_currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Domestic currency".into()))?;
        let foreign_currency = self
            .foreign_currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Foreign currency".into()))?;
        let domestic_market_index = self
            .domestic_market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Domestic market index".into()))?;
        let foreign_market_index = self
            .foreign_market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Foreign market index".into()))?;
        let identifier = self
            .identifier
            .ok_or_else(|| AtlasError::ValueNotSetErr("Identifier".into()))?;

        let side = self.side.unwrap_or(Side::LongRecieve);
        let domestic_spread = self.domestic_spread.unwrap_or(0.0);
        let foreign_spread = self.foreign_spread.unwrap_or(0.0);
        let domestic_freq = self.domestic_leg_frequency.unwrap_or(Frequency::Quarterly);
        let foreign_freq = self.foreign_leg_frequency.unwrap_or(Frequency::Quarterly);

        // Domestic (floating) leg
        let domestic_leg = MakeLeg::default()
            .set_leg_id(0)
            .with_notional(domestic_notional)
            .with_side(side)
            .with_currency(domestic_currency)
            .with_market_index(domestic_market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(domestic_spread)
            .with_payment_frequency(domestic_freq)
            .bullet()
            .with_calendar(self.calendar.clone())
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        // Foreign (floating) leg — opposite side
        let foreign_side = match side {
            Side::LongRecieve => Side::PayShort,
            Side::PayShort => Side::LongRecieve,
        };

        let foreign_leg = MakeLeg::default()
            .set_leg_id(1)
            .with_notional(foreign_notional)
            .with_side(foreign_side)
            .with_currency(foreign_currency)
            .with_market_index(foreign_market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(foreign_spread)
            .with_payment_frequency(foreign_freq)
            .bullet()
            .with_calendar(self.calendar)
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        Ok(FloatFloatCrossCurrencySwap::new(
            identifier,
            domestic_leg,
            foreign_leg,
            domestic_currency,
            foreign_currency,
            domestic_market_index,
            foreign_market_index,
        ))
    }
}
