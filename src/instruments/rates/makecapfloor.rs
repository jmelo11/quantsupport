use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, RateType},
        rates::capfloor::{CapFloor, CapFloorType},
    },
    rates::interestrate::RateDefinition,
    time::{
        calendar::Calendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{AtlasError, Result},
};

/// A builder for creating a [`CapFloor`] instance (an interest rate cap or floor).
///
/// A cap (floor) is a strip of caplets (floorlets), modelled as a single
/// floating-rate leg whose coupons carry an embedded option.
#[derive(Default)]
pub struct MakeCapFloor {
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    strike: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    rate_definition: Option<RateDefinition>,
    market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    cap_floor_type: Option<CapFloorType>,
    frequency: Option<Frequency>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
}

impl MakeCapFloor {
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

    /// Sets the strike rate.
    #[must_use]
    pub fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(strike);
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

    /// Sets the rate definition for the floating leg.
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

    /// Sets the currency.
    #[must_use]
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side (buyer or seller of the cap/floor).
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the cap/floor type.
    #[must_use]
    pub fn with_cap_floor_type(mut self, cap_floor_type: CapFloorType) -> Self {
        self.cap_floor_type = Some(cap_floor_type);
        self
    }

    /// Sets the payment frequency.
    #[must_use]
    pub fn with_frequency(mut self, frequency: Frequency) -> Self {
        self.frequency = Some(frequency);
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

    /// Builds the [`CapFloor`] instance.
    ///
    /// # Errors
    /// Returns an error when required fields are missing or the underlying
    /// leg builder fails.
    pub fn build(self) -> Result<CapFloor> {
        let notional = self
            .notional
            .ok_or_else(|| AtlasError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Maturity date".into()))?;
        let strike = self
            .strike
            .ok_or_else(|| AtlasError::ValueNotSetErr("Strike".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Currency".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;
        let identifier = self
            .identifier
            .ok_or_else(|| AtlasError::ValueNotSetErr("Identifier".into()))?;
        let cap_floor_type = self
            .cap_floor_type
            .ok_or_else(|| AtlasError::ValueNotSetErr("CapFloorType".into()))?;

        let side = self.side.unwrap_or(Side::LongRecieve);
        let frequency = self.frequency.unwrap_or(Frequency::Quarterly);

        // Build a floating leg with the embedded cap or floor strike.
        let mut leg_builder = MakeLeg::default()
            .set_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_currency(currency)
            .with_market_index(market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_payment_frequency(frequency)
            .bullet()
            .with_calendar(self.calendar)
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month);

        leg_builder = match cap_floor_type {
            CapFloorType::Cap => leg_builder.with_caplet_strike(Some(strike)),
            CapFloorType::Floor => leg_builder.with_floorlet_strike(Some(strike)),
        };

        let leg = leg_builder.build()?;

        Ok(CapFloor::new(
            identifier,
            leg,
            market_index,
            currency,
            strike,
            cap_floor_type,
        ))
    }
}
