use crate::{currencies::currency::Currency, time::date::Date};

/// Request for an FX rate involving one or two currencies.
///
/// - **Two currencies** (`base` + `quote`): explicit pair (e.g. USD/EUR for an FX forward).
///   The context will triangulate to the reporting currency if needed.
/// - **One currency** (`base` only, `quote = None`): the cashflow pays in `base`.
///   The context resolves the conversion to the reporting currency.
#[derive(Clone)]
pub struct FxRequest {
    base: Currency,
    quote: Option<Currency>,
    date: Option<Date>,
}

impl FxRequest {
    /// Creates a new [`FxRequest`] with a single currency.
    /// The context will resolve conversion to the reporting currency.
    #[must_use]
    pub const fn single(base: Currency) -> Self {
        Self {
            base,
            quote: None,
            date: None,
        }
    }

    /// Creates a new [`FxRequest`] with an explicit base/quote pair.
    /// The context will triangulate both to the reporting currency.
    #[must_use]
    pub const fn pair(base: Currency, quote: Currency) -> Self {
        Self {
            base,
            quote: Some(quote),
            date: None,
        }
    }

    /// Sets the request date, if any.
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
