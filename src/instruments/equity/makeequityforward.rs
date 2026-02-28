use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::equity::equityforward::EquityForward,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{AtlasError, Result},
};

/// A builder for creating an [`EquityForward`] instance.
#[derive(Default)]
pub struct MakeEquityForward {
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    delivery_date: Option<Date>,
    strike: Option<f64>,
    currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    side: Option<Side>,
}

impl MakeEquityForward {
    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the market index for the underlying equity.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the delivery date.
    #[must_use]
    pub fn with_delivery_date(mut self, date: Date) -> Self {
        self.delivery_date = Some(date);
        self
    }

    /// Sets the forward (strike) price.
    #[must_use]
    pub fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Sets the currency.
    #[must_use]
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the day count convention. Defaults to `Actual360`.
    #[must_use]
    pub fn with_day_counter(mut self, dc: DayCounter) -> Self {
        self.day_counter = Some(dc);
        self
    }

    /// Sets the side (defaults to `LongRecieve`).
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Builds the [`EquityForward`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<EquityForward> {
        let identifier = self
            .identifier
            .ok_or_else(|| AtlasError::ValueNotSetErr("Identifier".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;
        let delivery_date = self
            .delivery_date
            .ok_or_else(|| AtlasError::ValueNotSetErr("Delivery date".into()))?;
        let strike = self
            .strike
            .ok_or_else(|| AtlasError::ValueNotSetErr("Strike".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Currency".into()))?;

        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual360);

        Ok(EquityForward::new(
            identifier,
            market_index,
            delivery_date,
            strike,
            currency,
            day_counter,
        ))
    }
}
