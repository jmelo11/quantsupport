use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    instruments::fx::fxforward::FxForward,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A builder for creating an [`FxForward`] instance.
#[derive(Default)]
pub struct MakeFxForward {
    identifier: Option<String>,
    delivery_date: Option<Date>,
    forward_rate: Option<f64>,
    base_currency: Option<Currency>,
    quote_currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    side: Option<Side>,
}

impl MakeFxForward {
    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the delivery date.
    #[must_use]
    pub fn with_delivery_date(mut self, date: Date) -> Self {
        self.delivery_date = Some(date);
        self
    }

    /// Sets the agreed forward exchange rate.
    #[must_use]
    pub fn with_forward_rate(mut self, rate: f64) -> Self {
        self.forward_rate = Some(rate);
        self
    }

    /// Sets the base currency (the currency being bought).
    #[must_use]
    pub fn with_base_currency(mut self, currency: Currency) -> Self {
        self.base_currency = Some(currency);
        self
    }

    /// Sets the quote currency (the currency being sold).
    #[must_use]
    pub fn with_quote_currency(mut self, currency: Currency) -> Self {
        self.quote_currency = Some(currency);
        self
    }

    /// Sets the day count convention. Defaults to `Actual360`.
    #[must_use]
    pub fn with_day_counter(mut self, dc: DayCounter) -> Self {
        self.day_counter = Some(dc);
        self
    }

    /// Sets the side (defaults to `LongRecieve` — buying base currency).
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Builds the [`FxForward`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<FxForward> {
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let delivery_date = self
            .delivery_date
            .ok_or_else(|| QSError::ValueNotSetErr("Delivery date".into()))?;
        let forward_rate = self
            .forward_rate
            .ok_or_else(|| QSError::ValueNotSetErr("Forward rate".into()))?;
        let base_currency = self
            .base_currency
            .ok_or_else(|| QSError::ValueNotSetErr("Base currency".into()))?;
        let quote_currency = self
            .quote_currency
            .ok_or_else(|| QSError::ValueNotSetErr("Quote currency".into()))?;

        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual360);

        Ok(FxForward::new(
            identifier,
            delivery_date,
            forward_rate,
            base_currency,
            quote_currency,
            day_counter,
        ))
    }
}
