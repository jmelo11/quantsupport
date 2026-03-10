use crate::{
    ad::adreal::{ADReal, IsReal},
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, RateType},
        fixedincome::fixedratedeposit::FixedRateDeposit,
    },
    rates::interestrate::{InterestRate, RateDefinition},
    time::{date::Date, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// A builder for creating a [`FixedRateDeposit`] instance, allowing for a flexible and stepwise construction process.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let rate_def = RateDefinition::new(
///     DayCounter::Actual360,
///     Compounding::Simple,
///     Frequency::Annual,
/// );
///
/// let deposit = MakeFixedRateDeposit::default()
///     .with_identifier("DEPO-3M".to_string())
///     .with_start_date(Date::new(2024, 1, 1))
///     .with_maturity_date(Date::new(2024, 4, 1))
///     .with_rate(0.05)
///     .with_notional(1_000_000.0)
///     .with_rate_definition(rate_def)
///     .with_market_index(MarketIndex::SOFR)
///     .with_currency(Currency::USD)
///     .build()
///     .expect("failed to build fixed rate deposit");
///
/// assert_eq!(deposit.identifier(), "DEPO-3M");
/// ```
#[derive(Default)]
pub struct MakeFixedRateDeposit {
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
}

impl MakeFixedRateDeposit {
    /// Sets the start date of the fixed rate deposit.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the end date of the fixed rate deposit.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self
    }

    /// Sets the interest rate of the fixed rate deposit.
    #[must_use]
    pub const fn with_rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the notional amount of the fixed rate deposit.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the rate definition of the fixed rate deposit.
    #[must_use]
    pub const fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the market index associated with the fixed rate deposit.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the currency of the fixed rate deposit.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the units of the fixed rate deposit.
    /// If not set, it defaults to 100.0.
    #[must_use]
    pub const fn with_units(mut self, units: f64) -> Self {
        self.units = Some(units);
        self
    }

    /// Sets the identifier of the fixed rate deposit.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the side of the fixed rate deposit (defaults to `LongRecieve` if not set).
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Builds the [`FixedRateDeposit`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<FixedRateDeposit> {
        let notional = self
            .notional
            .ok_or_else(|| QSError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| QSError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| QSError::ValueNotSetErr("Maturity date".into()))?;
        let rate = self
            .rate
            .ok_or_else(|| QSError::ValueNotSetErr("Rate".into()))?;
        let rate_definition = self
            .rate_definition
            .ok_or_else(|| QSError::ValueNotSetErr("Rate definition".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| QSError::ValueNotSetErr("Market index ".into()))?;

        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;

        let units = self.units.unwrap_or(100.0);
        let side = self.side.unwrap_or(Side::LongRecieve);

        let interest_rate = InterestRate::from_rate_definition(ADReal::new(rate), rate_definition);

        let leg = MakeLeg::default()
            .with_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_currency(currency)
            .with_market_index(market_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Fixed)
            .with_rate(interest_rate)
            .with_payment_frequency(Frequency::Once)
            .bullet()
            .build()?;

        Ok(FixedRateDeposit::new(
            identifier,
            units,
            leg,
            market_index,
            currency,
        ))
    }
}
