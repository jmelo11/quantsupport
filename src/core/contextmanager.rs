use crate::{
    currencies::currency::Currency,
    models::ModelParameters,
    quotes::{fixingstore::FixingStore, quote::Level, quotestore::QuoteStore},
    time::date::Date,
};

/// Manages the context for instrument evaluation, including market data access, quote level preferences,
/// base currency settings, and a list of model parameter sets for multiple model types.
pub struct ContextManager {
    quote_store: QuoteStore,
    fixing_store: FixingStore,
    quote_level: Level, // Placeholder to select the type of quote we want to use
    base_currency: Currency,
    models: Vec<ModelParameters>,
}

impl ContextManager {
    /// Creates a new pricing data context.
    #[must_use]
    pub const fn new(quote_store: QuoteStore, fixing_store: FixingStore) -> Self {
        Self {
            quote_store,
            fixing_store,
            quote_level: Level::Mid,
            base_currency: Currency::USD,
            models: Vec::new(),
        }
    }

    /// Sets the quote level used for market value extraction.
    #[must_use]
    pub const fn with_quote_level(mut self, quote_level: Level) -> Self {
        self.quote_level = quote_level;
        self
    }

    /// Returns the market data provider.
    #[must_use]
    pub const fn quote_store(&self) -> &QuoteStore {
        &self.quote_store
    }

    /// Returns the fixings provider.
    #[must_use]
    pub const fn fixing_store(&self) -> &FixingStore {
        &self.fixing_store
    }

    /// Returns the quote level preference.
    #[must_use]
    pub const fn quote_level(&self) -> Level {
        self.quote_level
    }

    /// Returns the base currency for reporting.
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Sets the base currency.
    #[must_use]
    pub const fn with_base_currency(mut self, base_currency: Currency) -> Self {
        self.base_currency = base_currency;
        self
    }

    /// Returns the current reference date.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.quote_store.reference_date()
    }

    /// Sets the model parameter list, replacing any previously registered models.
    #[must_use]
    pub fn with_models(mut self, models: &[ModelParameters]) -> Self {
        models.clone_into(&mut self.models);
        self
    }

    /// Returns the full list of model parameters registered in this context.
    #[must_use]
    pub fn models(&self) -> &[ModelParameters] {
        &self.models
    }
}
