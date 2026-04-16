use crate::{
    currencies::currency::Currency,
    indices::fxpair::FxPair,
    instruments::fx::fxoption::{FxOption, FxOptionType},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// A builder for creating an [`FxOption`] instance.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let fx_opt = MakeFxOption::default()
///     .with_identifier("EURUSD-1Y-CALL".to_string())
///     .with_expiry_date(Date::new(2027, 4, 11))
///     .with_strike(1.12)
///     .with_option_type(FxOptionType::Call)
///     .with_base_currency(Currency::EUR)
///     .with_quote_currency(Currency::USD)
///     .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
///     .build()
///     .expect("failed to build fx option");
///
/// assert_eq!(fx_opt.strike(), Strike::Absolute(1.12));
/// ```
#[derive(Default)]
pub struct MakeFxOption {
    identifier: Option<String>,
    expiry_date: Option<Date>,
    strike: Option<f64>,
    option_type: Option<FxOptionType>,
    base_currency: Option<Currency>,
    quote_currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    pair: Option<FxPair>,
}

impl MakeFxOption {
    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the expiry date.
    #[must_use]
    pub const fn with_expiry_date(mut self, date: Date) -> Self {
        self.expiry_date = Some(date);
        self
    }

    /// Sets the strike price.
    #[must_use]
    pub const fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Sets the option type (Call or Put).
    #[must_use]
    pub const fn with_option_type(mut self, option_type: FxOptionType) -> Self {
        self.option_type = Some(option_type);
        self
    }

    /// Sets the base currency (the currency being bought in a call).
    #[must_use]
    pub const fn with_base_currency(mut self, currency: Currency) -> Self {
        self.base_currency = Some(currency);
        self
    }

    /// Sets the quote currency.
    #[must_use]
    pub const fn with_quote_currency(mut self, currency: Currency) -> Self {
        self.quote_currency = Some(currency);
        self
    }

    /// Sets the day count convention. Defaults to `Actual360`.
    #[must_use]
    pub const fn with_day_counter(mut self, dc: DayCounter) -> Self {
        self.day_counter = Some(dc);
        self
    }

    /// Sets the FX pair.
    #[must_use]
    pub const fn with_pair(mut self, pair: FxPair) -> Self {
        self.pair = Some(pair);
        self
    }

    /// Builds the [`FxOption`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<FxOption> {
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let expiry_date = self
            .expiry_date
            .ok_or_else(|| QSError::ValueNotSetErr("Expiry date".into()))?;
        let strike = self
            .strike
            .ok_or_else(|| QSError::ValueNotSetErr("Strike".into()))?;
        let option_type = self
            .option_type
            .ok_or_else(|| QSError::ValueNotSetErr("Option type".into()))?;
        let base_currency = self
            .base_currency
            .ok_or_else(|| QSError::ValueNotSetErr("Base currency".into()))?;
        let quote_currency = self
            .quote_currency
            .ok_or_else(|| QSError::ValueNotSetErr("Quote currency".into()))?;
        let pair = self
            .pair
            .ok_or_else(|| QSError::ValueNotSetErr("FX pair".into()))?;

        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual360);

        Ok(FxOption::new(
            identifier,
            expiry_date,
            Strike::Absolute(strike),
            option_type,
            base_currency,
            quote_currency,
            day_counter,
            pair,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::MakeFxOption;
    use crate::{
        currencies::currency::Currency, indices::fxpair::FxPair,
        instruments::fx::fxoption::FxOptionType, time::date::Date,
        volatility::volatilityindexing::Strike,
    };

    #[test]
    fn builds_fx_call_option() {
        let fx_opt = MakeFxOption::default()
            .with_identifier("EURUSD-1Y-CALL".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_strike(1.12)
            .with_option_type(FxOptionType::Call)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build()
            .expect("call option should build");

        assert_eq!(fx_opt.strike(), Strike::Absolute(1.12));
        assert_eq!(fx_opt.option_type(), FxOptionType::Call);
        assert_eq!(fx_opt.base_currency(), Currency::EUR);
        assert_eq!(fx_opt.quote_currency(), Currency::USD);
    }

    #[test]
    fn builds_fx_put_option() {
        let fx_opt = MakeFxOption::default()
            .with_identifier("EURUSD-1Y-PUT".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_strike(1.08)
            .with_option_type(FxOptionType::Put)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build()
            .expect("put option should build");

        assert_eq!(fx_opt.option_type(), FxOptionType::Put);
    }

    #[test]
    fn missing_strike_fails() {
        let result = MakeFxOption::default()
            .with_identifier("EURUSD-1Y-CALL".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_option_type(FxOptionType::Call)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build();

        assert!(result.is_err());
    }
}
