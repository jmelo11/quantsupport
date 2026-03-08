use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::equity::futures::Futures,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A builder for creating a [`Futures`] instance.
#[derive(Default)]
pub struct MakeFutures {
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    expiry_date: Option<Date>,
    futures_price: Option<f64>,
    contract_size: Option<f64>,
    currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    side: Option<Side>,
}

impl MakeFutures {
    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the market index of the underlying.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the expiry date.
    #[must_use]
    pub const fn with_expiry_date(mut self, date: Date) -> Self {
        self.expiry_date = Some(date);
        self
    }

    /// Sets the futures price.
    #[must_use]
    pub const fn with_futures_price(mut self, price: f64) -> Self {
        self.futures_price = Some(price);
        self
    }

    /// Sets the contract size (multiplier). Defaults to 1.0.
    #[must_use]
    pub const fn with_contract_size(mut self, size: f64) -> Self {
        self.contract_size = Some(size);
        self
    }

    /// Sets the currency. Defaults to `USD`.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the day count convention. Defaults to `Actual360`.
    #[must_use]
    pub const fn with_day_counter(mut self, dc: DayCounter) -> Self {
        self.day_counter = Some(dc);
        self
    }

    /// Sets the side (defaults to `LongRecieve`).
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Builds the [`Futures`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<Futures> {
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?;
        let expiry_date = self
            .expiry_date
            .ok_or_else(|| QSError::ValueNotSetErr("Expiry date".into()))?;
        let futures_price = self
            .futures_price
            .ok_or_else(|| QSError::ValueNotSetErr("Futures price".into()))?;

        let contract_size = self.contract_size.unwrap_or(1.0);
        let currency = self.currency.unwrap_or(Currency::USD);
        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual360);

        Ok(Futures::new(
            identifier,
            market_index,
            expiry_date,
            futures_price,
            contract_size,
            currency,
            day_counter,
        ))
    }
}
