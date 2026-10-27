use crate::{
    core::makeinstrument::{resolve_date, MakeInstrument},
    currencies::currency::Currency,
    indices::{fxpair::FxPair, marketindex::MarketIndex},
    instruments::{
        equity::equityeuropeanoption::EuroOptionType, fx::fxeuropeanoption::FxEuropeanOption,
    },
    time::{
        calendars::nullcalendar::NullCalendar, date::Date, daycounter::DayCounter,
        enums::BusinessDayConvention, period::Period,
    },
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// A builder for creating an [`FxEuropeanOption`] instance.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let fx_opt = MakeFxEuropeanOption::default()
///     .with_identifier("EURUSD-1Y-CALL".to_string())
///     .with_expiry_date(Date::new(2027, 4, 11))
///     .with_strike(1.12)
///     .with_option_type(EuroOptionType::Call)
///     .with_base_currency(Currency::EUR)
///     .with_quote_currency(Currency::USD)
///     .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
///     .build()
///     .expect("failed to build fx option");
///
/// assert_eq!(fx_opt.strike(), Strike::Absolute(1.12));
/// ```
#[derive(Clone, Default)]
pub struct MakeFxEuropeanOption {
    expiry_tenor: Option<Period>,
    identifier: Option<String>,
    expiry_date: Option<Date>,
    strike: Option<Strike>,
    option_type: Option<EuroOptionType>,
    base_currency: Option<Currency>,
    quote_currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    pair: Option<FxPair>,
}

impl MakeFxEuropeanOption {
    /// Sets expiry date as a tenor from the context evaluation date.
    /// Replaces an explicitly supplied expiry date. Use `build_using` when an external anchor is needed.
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

    /// Sets the expiry date.
    #[must_use]
    pub const fn with_expiry_date(mut self, date: Date) -> Self {
        self.expiry_date = Some(date);
        self.expiry_tenor = None;
        self
    }

    /// Sets the strike price.
    #[must_use]
    pub const fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(Strike::Absolute(strike));
        self
    }

    /// Sets an absolute, ATM, or relative strike specification.
    #[must_use]
    pub const fn with_strike_spec(mut self, strike: Strike) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Sets the option type (Call or Put).
    #[must_use]
    pub const fn with_option_type(mut self, option_type: EuroOptionType) -> Self {
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
}

impl MakeInstrument for MakeFxEuropeanOption {
    type Instrument = FxEuropeanOption;

    fn is_resolved(&self) -> bool {
        self.expiry_tenor.is_none()
    }

    /// Builds the [`FxEuropeanOption`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeFxEuropeanOption has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if self.expiry_date == Some(Date::empty()) {
            return Err(QSError::InvalidValueErr(
                "MakeFxEuropeanOption dates must not be empty".into(),
            ));
        }
        let identifier = self
            .identifier
            .clone()
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

        let market_index = MarketIndex::FxPair(pair);
        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual360);

        Ok(FxEuropeanOption::new(
            identifier,
            market_index,
            expiry_date,
            strike,
            option_type,
            base_currency,
            quote_currency,
            day_counter,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let date_calendar = &NullCalendar::new();
        let date_convention = BusinessDayConvention::Unadjusted;
        let date_end_of_month = false;
        resolved.expiry_date = Some(resolve_date(
            self.expiry_date,
            self.expiry_tenor,
            reference_date,
            "expiry_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.expiry_tenor = None;
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::MakeFxEuropeanOption;
    use crate::{
        core::makeinstrument::MakeInstrument, currencies::currency::Currency,
        indices::fxpair::FxPair, instruments::equity::equityeuropeanoption::EuroOptionType,
        time::date::Date, volatility::volatilityindexing::Strike,
    };

    #[test]
    fn builds_fx_call_option() {
        let fx_opt = MakeFxEuropeanOption::default()
            .with_identifier("EURUSD-1Y-CALL".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_strike(1.12)
            .with_option_type(EuroOptionType::Call)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build()
            .expect("call option should build");

        assert_eq!(fx_opt.strike(), Strike::Absolute(1.12));
        assert_eq!(fx_opt.option_type(), EuroOptionType::Call);
        assert_eq!(fx_opt.base_currency(), Currency::EUR);
        assert_eq!(fx_opt.quote_currency(), Currency::USD);
    }

    #[test]
    fn builds_fx_put_option() {
        let fx_opt = MakeFxEuropeanOption::default()
            .with_identifier("EURUSD-1Y-PUT".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_strike(1.08)
            .with_option_type(EuroOptionType::Put)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build()
            .expect("put option should build");

        assert_eq!(fx_opt.option_type(), EuroOptionType::Put);
    }

    #[test]
    fn preserves_relative_strike() {
        let fx_opt = MakeFxEuropeanOption::default()
            .with_identifier("EURUSD-1Y-CALL".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_strike_spec(Strike::Relative(0.05))
            .with_option_type(EuroOptionType::Call)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build()
            .expect("call option should build");

        assert_eq!(fx_opt.strike(), Strike::Relative(0.05));
    }

    #[test]
    fn missing_strike_fails() {
        let result = MakeFxEuropeanOption::default()
            .with_identifier("EURUSD-1Y-CALL".to_string())
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_option_type(EuroOptionType::Call)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD).unwrap())
            .build();

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn fx_option_resolves_relative_expiry() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let expiry = reference() + p("1Y")?;
        let fx_option = MakeFxEuropeanOption::default()
            .with_identifier("fx-option".into())
            .with_expiry_tenor(p("1Y")?)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_pair(FxPair::new(Currency::EUR, Currency::USD)?)
            .with_strike(1.1)
            .with_option_type(EuroOptionType::Call)
            .build_using(ctx)?;
        assert_eq!(fx_option.expiry_date(), expiry);
        Ok(())
    }
}
