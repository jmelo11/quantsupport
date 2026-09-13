use crate::{
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::equity::equityeuropeanoption::{EquityEuropeanOption, EuroOptionType},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// A builder for creating an [`EquityEuropeanOption`] instance.
#[derive(Default)]
pub struct MakeEquityEuropeanOption {
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    expiry_date: Option<Date>,
    strike: Option<Strike>,
    option_type: Option<EuroOptionType>,
    currency: Option<Currency>,
    day_counter: Option<DayCounter>,
}

impl MakeEquityEuropeanOption {
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

    /// Sets the expiry date.
    #[must_use]
    pub const fn with_expiry_date(mut self, expiry_date: Date) -> Self {
        self.expiry_date = Some(expiry_date);
        self
    }

    /// Sets an absolute strike price.
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

    /// Sets the option type.
    #[must_use]
    pub const fn with_option_type(mut self, option_type: EuroOptionType) -> Self {
        self.option_type = Some(option_type);
        self
    }

    /// Sets the settlement currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the day count convention. Defaults to `Actual360`.
    #[must_use]
    pub const fn with_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.day_counter = Some(day_counter);
        self
    }

    /// Builds the [`EquityEuropeanOption`] instance.
    ///
    /// # Errors
    /// Returns an error if any required field is missing.
    pub fn build(self) -> Result<EquityEuropeanOption> {
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?;
        let expiry_date = self
            .expiry_date
            .ok_or_else(|| QSError::ValueNotSetErr("Expiry date".into()))?;
        let strike = self
            .strike
            .ok_or_else(|| QSError::ValueNotSetErr("Strike".into()))?;
        let option_type = self
            .option_type
            .ok_or_else(|| QSError::ValueNotSetErr("Option type".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;

        Ok(
            EquityEuropeanOption::new(market_index, expiry_date, strike, option_type, identifier)
                .with_currency(currency)
                .with_day_counter(self.day_counter.unwrap_or(DayCounter::Actual360)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::MakeEquityEuropeanOption;
    use crate::{
        currencies::currency::Currency, indices::marketindex::MarketIndex,
        instruments::equity::equityeuropeanoption::EuroOptionType, time::date::Date,
        volatility::volatilityindexing::Strike,
    };

    #[test]
    fn builds_equity_option_with_relative_strike() {
        let option = MakeEquityEuropeanOption::default()
            .with_identifier("SPX-1Y-CALL".to_string())
            .with_market_index(MarketIndex::Equity("SPX".to_string()))
            .with_expiry_date(Date::new(2027, 4, 11))
            .with_strike_spec(Strike::Relative(0.05))
            .with_option_type(EuroOptionType::Call)
            .with_currency(Currency::USD)
            .build()
            .expect("equity option should build");

        assert_eq!(option.strike(), Strike::Relative(0.05));
        assert_eq!(option.option_type(), &EuroOptionType::Call);
    }
}
