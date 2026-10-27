use crate::{
    core::{
        makeinstrument::{resolve_date, MakeInstrument},
        trade::Side,
    },
    indices::marketindex::MarketIndex,
    instruments::rates::ratefutures::RateFutures,
    rates::interestrate::RateDefinition,
    time::{
        calendars::nullcalendar::NullCalendar, date::Date, enums::BusinessDayConvention,
        period::Period,
    },
    utils::errors::{QSError, Result},
};

/// A builder for creating a [`RateFutures`] instance.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let rate_futures = MakeRateFutures::default()
///     .with_identifier("SR3-M24".to_string())
///     .with_market_index(MarketIndex::SOFR)
///     .with_start_date(Date::new(2024, 3, 20))
///     .with_end_date(Date::new(2024, 6, 20))
///     .with_futures_price(95.25)
///     .build()
///     .expect("failed to build rate futures");
///
/// assert_eq!(rate_futures.futures_price(), 95.25);
/// assert_eq!(rate_futures.contract_size(), 2500.0);
/// ```
#[derive(Clone, Default)]
pub struct MakeRateFutures {
    start_tenor: Option<Period>,
    end_tenor: Option<Period>,
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    start_date: Option<Date>,
    end_date: Option<Date>,
    futures_price: Option<f64>,
    contract_size: Option<f64>,
    rate_definition: Option<RateDefinition>,
    side: Option<Side>,
}

impl MakeRateFutures {
    /// Sets start date as a tenor from the context evaluation date.
    /// Replaces an explicitly supplied start date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_tenor = Some(tenor);
        self.start_date = None;
        self
    }

    /// Sets end date as a tenor from the resolved start date.
    /// Replaces an explicitly supplied end date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_end_tenor(mut self, tenor: Period) -> Self {
        self.end_tenor = Some(tenor);
        self.end_date = None;
        self
    }

    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the market index for the reference rate (e.g., SOFR, `TermSOFR3m`).
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the contract start / fixing date.
    #[must_use]
    pub const fn with_start_date(mut self, date: Date) -> Self {
        self.start_date = Some(date);
        self.start_tenor = None;
        self
    }

    /// Sets the end date of the accrual period.
    #[must_use]
    pub const fn with_end_date(mut self, date: Date) -> Self {
        self.end_date = Some(date);
        self.end_tenor = None;
        self
    }

    /// Sets the futures price (e.g. 95.25).
    #[must_use]
    pub const fn with_futures_price(mut self, price: f64) -> Self {
        self.futures_price = Some(price);
        self
    }

    /// Sets currency per quoted price point; defaults to `2500.0`.
    ///
    /// A one-basis-point rate change moves the quote by `0.01`, corresponding
    /// to 25 currency units per contract with the default multiplier.
    #[must_use]
    pub const fn with_contract_size(mut self, size: f64) -> Self {
        self.contract_size = Some(size);
        self
    }

    /// Sets the rate definition (day counter, compounding, frequency).
    #[must_use]
    pub const fn with_rate_definition(mut self, rd: RateDefinition) -> Self {
        self.rate_definition = Some(rd);
        self
    }

    /// Stores a compatibility setting that is not used by `build`.
    ///
    /// Set the position direction on `RateFuturesTrade` instead.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }
}

impl MakeInstrument for MakeRateFutures {
    type Instrument = RateFutures;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none() && self.end_tenor.is_none()
    }

    /// Builds the [`RateFutures`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeRateFutures has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if [self.start_date, self.end_date].contains(&Some(Date::empty())) {
            return Err(QSError::InvalidValueErr(
                "MakeRateFutures dates must not be empty".into(),
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
        let start_date = self
            .start_date
            .ok_or_else(|| QSError::ValueNotSetErr("Start date".into()))?;
        let end_date = self
            .end_date
            .ok_or_else(|| QSError::ValueNotSetErr("End date".into()))?;
        let futures_price = self
            .futures_price
            .ok_or_else(|| QSError::ValueNotSetErr("Futures price".into()))?;

        let rate_definition = if let Some(rd) = self.rate_definition {
            rd
        } else {
            market_index.rate_index_details()?.rate_definition()
        };

        let contract_size = self.contract_size.unwrap_or(2500.0);

        Ok(RateFutures::new(
            identifier,
            market_index,
            start_date,
            end_date,
            futures_price,
            contract_size,
            rate_definition,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let date_calendar = &NullCalendar::new();
        let date_convention = BusinessDayConvention::Unadjusted;
        let date_end_of_month = false;
        resolved.start_date = Some(resolve_date(
            self.start_date,
            self.start_tenor,
            reference_date,
            "start_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.start_tenor = None;
        resolved.end_date = Some(resolve_date(
            self.end_date,
            self.end_tenor,
            resolved
                .start_date
                .ok_or_else(|| QSError::ValueNotSetErr("start_date".into()))?,
            "end_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.end_tenor = None;
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_builder() -> MakeRateFutures {
        MakeRateFutures::default()
            .with_identifier("rate_fut_test".to_string())
            .with_market_index(MarketIndex::SOFR)
            .with_start_date(Date::new(2024, 3, 20))
            .with_end_date(Date::new(2024, 6, 20))
            .with_futures_price(95.25)
    }

    #[test]
    fn test_build_rate_futures_success_with_default_contract_size() {
        let result = base_builder().build();
        assert!(result.is_ok(), "expected rate futures build to succeed");

        let fut = result.unwrap();
        assert_eq!(fut.market_index(), MarketIndex::SOFR);
        assert_eq!(fut.futures_price(), 95.25);
        assert_eq!(fut.contract_size(), 2500.0);
        assert_eq!(
            fut.rate_definition(),
            MarketIndex::SOFR
                .rate_index_details()
                .unwrap()
                .rate_definition()
        );
    }

    #[test]
    fn test_build_rate_futures_missing_price_fails() {
        let result = MakeRateFutures::default()
            .with_identifier("rate_fut_missing_price".to_string())
            .with_market_index(MarketIndex::SOFR)
            .with_start_date(Date::new(2024, 3, 20))
            .with_end_date(Date::new(2024, 6, 20))
            .build();

        assert!(result.is_err(), "expected missing futures price to fail");
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn future_resolves_relative_accrual_start_and_end() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let future = MakeRateFutures::default()
            .with_identifier("future".into())
            .with_start_tenor(p("1Y")?)
            .with_end_tenor(p("3M")?)
            .with_market_index(MarketIndex::EURIBOR3m)
            .with_futures_price(96.0)
            .build_using(ctx)?;
        assert_eq!(future.start_date(), reference() + p("1Y")?);
        assert_eq!(future.end_date(), reference() + p("15M")?);
        Ok(())
    }
}
