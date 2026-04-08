use crate::currencies::currency::Currency;

/// Request for an FX spot rate between two currencies.
pub struct FxRequest {
    base: Currency,
    quote: Currency,
}

impl FxRequest {
    /// Creates a new [`FxRequest`] with the specified base and quote currencies.
    #[must_use]
    pub const fn new(base: Currency, quote: Currency) -> Self {
        Self { base, quote }
    }

    /// Returns the base currency.
    #[must_use]
    pub const fn base(&self) -> Currency {
        self.base
    }

    /// Returns the quote currency.
    #[must_use]
    pub const fn quote(&self) -> Currency {
        self.quote
    }
}
