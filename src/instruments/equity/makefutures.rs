use crate::{
    core::resolution::{ResolutionContext, Resolve},
    time::period::Period,
};
use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::equity::futures::Futures,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A builder for creating a [`Futures`] instance.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let futures = MakeFutures::default()
///     .with_identifier("ES-2024M".to_string())
///     .with_market_index(MarketIndex::SOFR)
///     .with_expiry_date(Date::new(2024, 6, 21))
///     .with_futures_price(5200.0)
///     .with_contract_size(50.0)
///     .with_currency(Currency::USD)
///     .build()
///     .expect("failed to build futures");
///
/// assert_eq!(futures.futures_price(), 5200.0);
/// ```
#[derive(Clone, Default)]
pub struct MakeFutures {
    expiry_tenor: Option<Period>,
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
    /// Sets expiry date as a tenor from the resolution reference date.
    /// Replaces an explicitly supplied expiry date. Resolve this maker with a context.
    #[must_use]
    pub const fn with_expiry_tenor(mut self, tenor: Period) -> Self {
        self.expiry_tenor = Some(tenor);
        self.expiry_date = None;
        self
    }

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
        self.expiry_tenor = None;
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

    /// Sets the side (defaults to `LongReceive`).
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
        if self.expiry_tenor.is_some() {
            return Err(QSError::InvalidValueErr(
                "Relative dates require maker.resolve(&ResolutionContext)".into(),
            ));
        }
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

impl Resolve for MakeFutures {
    type Resolved = Futures;

    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<Self::Resolved> {
        let mut resolved = self.clone();
        let dates = *context;
        resolved.expiry_date = Some(dates.resolve_date(
            self.expiry_date,
            self.expiry_tenor,
            context.reference_date(),
            "expiry_date",
        )?);
        resolved.expiry_tenor = None;
        resolved.build()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::resolution::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn equity_future_resolves_relative_expiry() -> Result<()> {
        let market = empty_market(reference());
        let ctx = ResolutionContext::new(&market);
        let expiry = reference() + p("1Y")?;
        let equity = MarketIndex::Other("ACME".into());
        let future = MakeFutures::default()
            .with_identifier("equity-future".into())
            .with_expiry_tenor(p("1Y")?)
            .with_market_index(equity)
            .with_currency(Currency::EUR)
            .with_futures_price(100.0)
            .resolve(&ctx)?;
        assert_eq!(future.expiry_date(), expiry);
        Ok(())
    }
}
