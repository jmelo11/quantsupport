use crate::{
    ad::adreal::{ADReal, IsReal},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon},
        fixedincome::fixedratedeposit::FixedRateDeposit,
    },
    rates::interestrate::{InterestRate, RateDefinition},
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// A builder for creating a [`FixedRateDeposit`] instance, allowing for a flexible and stepwise construction process.
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
}

impl MakeFixedRateDeposit {
    /// Sets the start date of the fixed rate deposit.
    #[must_use]
    pub fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the end date of the fixed rate deposit.
    #[must_use]
    pub fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self
    }

    /// Sets the interest rate of the fixed rate deposit.
    #[must_use]
    pub fn with_rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the notional amount of the fixed rate deposit.
    #[must_use]
    pub fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the rate definition of the fixed rate deposit.
    #[must_use]
    pub fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
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
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the units of the fixed rate deposit.
    /// If not set, it defaults to 100.0.
    #[must_use]
    pub fn with_units(mut self, units: f64) -> Self {
        self.units = Some(units);
        self
    }

    /// Sets the identifier of the fixed rate deposit.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Builds the [`FixedRateDeposit`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    pub fn build(self) -> Result<FixedRateDeposit> {
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
            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index ".into()))?;

        let identifier = self
            .identifier
            .ok_or_else(|| AtlasError::ValueNotSetErr("Indetifier".into()))?;

        let units = self.units.unwrap_or(100.0);
        let redemption = SimpleCashflow::new(notional, maturity_date);
        let interest_rate = InterestRate::from_rate_definition(ADReal::new(rate), rate_definition);
        let coupon = FixedRateCoupon::new(
            notional,
            Box::new(interest_rate),
            start_date,
            maturity_date,
            maturity_date,
        );

        Ok(FixedRateDeposit::new(
            identifier,
            units,
            interest_rate,
            coupon,
            redemption,
            market_index,
            currency,
        ))
    }
}
