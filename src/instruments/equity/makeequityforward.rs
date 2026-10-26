use crate::{
    core::resolution::{ResolutionContext, Resolve},
    time::period::Period,
};
use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::equity::equityforward::EquityForward,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A builder for creating an [`EquityForward`] instance.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let forward = MakeEquityForward::default()
///     .with_identifier("SPX-FWD-6M".to_string())
///     .with_market_index(MarketIndex::SOFR)
///     .with_delivery_date(Date::new(2024, 7, 1))
///     .with_strike(5100.0)
///     .with_currency(Currency::USD)
///     .build()
///     .expect("failed to build equity forward");
///
/// assert_eq!(forward.strike(), 5100.0);
/// ```
#[derive(Clone, Default)]
pub struct MakeEquityForward {
    delivery_tenor: Option<Period>,
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    delivery_date: Option<Date>,
    strike: Option<f64>,
    currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    side: Option<Side>,
}

impl MakeEquityForward {
    /// Sets delivery date as a tenor from the resolution reference date.
    /// Replaces an explicitly supplied delivery date. Resolve this maker with a context.
    #[must_use]
    pub const fn with_delivery_tenor(mut self, tenor: Period) -> Self {
        self.delivery_tenor = Some(tenor);
        self.delivery_date = None;
        self
    }

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
    pub const fn with_delivery_date(mut self, date: Date) -> Self {
        self.delivery_date = Some(date);
        self.delivery_tenor = None;
        self
    }

    /// Sets the forward (strike) price.
    #[must_use]
    pub const fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Sets the currency.
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

    /// Builds the [`EquityForward`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<EquityForward> {
        if self.delivery_tenor.is_some() {
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
        let delivery_date = self
            .delivery_date
            .ok_or_else(|| QSError::ValueNotSetErr("Delivery date".into()))?;
        let strike = self
            .strike
            .ok_or_else(|| QSError::ValueNotSetErr("Strike".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;

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

impl Resolve for MakeEquityForward {
    type Resolved = EquityForward;

    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<Self::Resolved> {
        let mut resolved = self.clone();
        let dates = *context;
        resolved.delivery_date = Some(dates.resolve_date(
            self.delivery_date,
            self.delivery_tenor,
            context.reference_date(),
            "delivery_date",
        )?);
        resolved.delivery_tenor = None;
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
    fn equity_forward_resolves_relative_delivery() -> Result<()> {
        let market = empty_market(reference());
        let ctx = ResolutionContext::new(&market);
        let expiry = reference() + p("1Y")?;
        let equity = MarketIndex::Other("ACME".into());
        let forward = MakeEquityForward::default()
            .with_identifier("equity-forward".into())
            .with_delivery_tenor(p("1Y")?)
            .with_market_index(equity.clone())
            .with_currency(Currency::EUR)
            .with_strike(100.0)
            .resolve(&ctx)?;
        assert_eq!(forward.delivery_date(), expiry);
        Ok(())
    }
}
