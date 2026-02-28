use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, RateType},
        rates::basisswap::BasisSwap,
    },
    time::{
        calendar::Calendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{AtlasError, Result},
};

/// A builder for creating a [`BasisSwap`] instance (floating-vs-floating interest rate swap).
#[derive(Default)]
pub struct MakeBasisSwap {
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
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
}

impl MakeBasisSwap {
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

    /// Sets the notional amount.
    #[must_use]
    pub fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the spread on the pay leg.
    #[must_use]
    pub fn with_pay_spread(mut self, spread: f64) -> Self {
        self.pay_spread = Some(spread);
        self
    }

    /// Sets the spread on the receive leg.
    #[must_use]
    pub fn with_receive_spread(mut self, spread: f64) -> Self {
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
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side.
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the pay leg payment frequency.
    #[must_use]
    pub fn with_pay_leg_frequency(mut self, freq: Frequency) -> Self {
        self.pay_leg_frequency = Some(freq);
        self
    }

    /// Sets the receive leg payment frequency.
    #[must_use]
    pub fn with_receive_leg_frequency(mut self, freq: Frequency) -> Self {
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

    /// Builds the [`BasisSwap`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<BasisSwap> {
        let notional = self
            .notional
            .ok_or_else(|| AtlasError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Maturity date".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Currency".into()))?;
        let pay_market_index = self
            .pay_market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Pay market index".into()))?;
        let receive_market_index = self
            .receive_market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Receive market index".into()))?;
        let identifier = self
            .identifier
            .ok_or_else(|| AtlasError::ValueNotSetErr("Identifier".into()))?;

        let pay_spread = self.pay_spread.unwrap_or(0.0);
        let receive_spread = self.receive_spread.unwrap_or(0.0);
        let pay_freq = self.pay_leg_frequency.unwrap_or(Frequency::Quarterly);
        let receive_freq = self.receive_leg_frequency.unwrap_or(Frequency::Quarterly);

        // Pay leg
        let pay_leg = MakeLeg::default()
            .set_leg_id(0)
            .with_notional(notional)
            .with_side(Side::PayShort)
            .with_currency(currency)
            .with_market_index(pay_market_index.clone())
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
        let receive_leg = MakeLeg::default()
            .set_leg_id(1)
            .with_notional(notional)
            .with_side(Side::LongRecieve)
            .with_currency(currency)
            .with_market_index(receive_market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(receive_spread)
            .with_payment_frequency(receive_freq)
            .bullet()
            .with_calendar(self.calendar)
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .build()?;

        Ok(BasisSwap::new(
            identifier,
            pay_leg,
            receive_leg,
            pay_market_index,
            receive_market_index,
            currency,
        ))
    }
}
