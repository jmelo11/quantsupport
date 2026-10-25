use crate::{currencies::currency::Currency, time::date::Date};

/// Coordinates for an FX observation; interpretation belongs to the consumer.
///
/// An explicit pair requests quote-currency units per one base-currency unit.
/// [`PricingContext`](crate::core::pricingcontext::PricingContext) resolves
/// such pairs from its spot FX store, including available triangulation paths.
/// A single currency requests the spot observations needed for conversion to
/// the context's base currency. The pricer composes that conversion on its AD
/// tape. The context does not use [`Self::date`] to construct a forward FX rate;
/// a simulation model may interpret it relative to its own domestic currency.
#[derive(Clone)]
pub struct FxRequest {
    base: Currency,
    quote: Option<Currency>,
    date: Option<Date>,
}

impl FxRequest {
    /// Requests conversion inputs from this currency to the consumer's base currency.
    #[must_use]
    pub const fn single(base: Currency) -> Self {
        Self {
            base,
            quote: None,
            date: None,
        }
    }

    /// Requests quote-currency units per base-currency unit.
    #[must_use]
    pub const fn pair(base: Currency, quote: Currency) -> Self {
        Self {
            base,
            quote: Some(quote),
            date: None,
        }
    }

    /// Sets an observation date for model-backed consumers.
    ///
    /// The pricing context's spot-store resolution ignores this date.
    #[must_use]
    pub const fn with_date(mut self, date: Date) -> Self {
        self.date = Some(date);
        self
    }

    /// Returns the base currency.
    #[must_use]
    pub const fn base(&self) -> Currency {
        self.base
    }

    /// Returns the quote currency, if specified.
    #[must_use]
    pub const fn quote(&self) -> Option<Currency> {
        self.quote
    }

    /// Returns the date for the FX rate, if specified.
    #[must_use]
    pub const fn date(&self) -> Option<Date> {
        self.date
    }

    /// Returns `true` if this is a two-currency (pair) request.
    #[must_use]
    pub const fn is_pair(&self) -> bool {
        self.quote.is_some()
    }
}
