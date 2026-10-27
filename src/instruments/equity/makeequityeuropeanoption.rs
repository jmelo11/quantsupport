use crate::{
    core::makeinstrument::{resolve_date, MakeInstrument},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::equity::equityeuropeanoption::{EquityEuropeanOption, EuroOptionType},
    time::{
        calendars::nullcalendar::NullCalendar, date::Date, daycounter::DayCounter,
        enums::BusinessDayConvention, period::Period,
    },
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// A builder for creating an [`EquityEuropeanOption`] instance.
#[derive(Clone, Default)]
pub struct MakeEquityEuropeanOption {
    expiry_tenor: Option<Period>,
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    expiry_date: Option<Date>,
    strike: Option<Strike>,
    option_type: Option<EuroOptionType>,
    currency: Option<Currency>,
    day_counter: Option<DayCounter>,
}

impl MakeEquityEuropeanOption {
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
        self.expiry_tenor = None;
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
}

impl MakeInstrument for MakeEquityEuropeanOption {
    type Instrument = EquityEuropeanOption;

    fn is_resolved(&self) -> bool {
        self.expiry_tenor.is_none()
    }

    /// Builds the [`EquityEuropeanOption`] instance.
    ///
    /// # Errors
    /// Returns an error if any required field is missing.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeEquityEuropeanOption has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if self.expiry_date == Some(Date::empty()) {
            return Err(QSError::InvalidValueErr(
                "MakeEquityEuropeanOption dates must not be empty".into(),
            ));
        }
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let market_index = self
            .market_index
            .clone()
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
    use super::MakeEquityEuropeanOption;
    use crate::{
        core::makeinstrument::MakeInstrument, currencies::currency::Currency,
        indices::marketindex::MarketIndex,
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

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn equity_option_resolves_relative_expiry() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let expiry = reference() + p("1Y")?;
        let equity = MarketIndex::Other("ACME".into());
        let option = MakeEquityEuropeanOption::default()
            .with_identifier("equity-option".into())
            .with_expiry_tenor(p("1Y")?)
            .with_market_index(equity.clone())
            .with_currency(Currency::EUR)
            .with_strike(100.0)
            .with_option_type(EuroOptionType::Call)
            .build_using(ctx)?;
        assert_eq!(option.expiry_date(), expiry);
        Ok(())
    }
}
