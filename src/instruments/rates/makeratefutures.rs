use crate::{
    core::trade::Side,
    indices::marketindex::MarketIndex,
    indices::rateindex::RateIndexDetails,
    instruments::rates::ratefutures::RateFutures,
    rates::interestrate::RateDefinition,
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// A builder for creating a [`RateFutures`] instance.
#[derive(Default)]
pub struct MakeRateFutures {
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
    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the market index for the reference rate (e.g., SOFR, TermSOFR3m).
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the contract start / fixing date.
    #[must_use]
    pub fn with_start_date(mut self, date: Date) -> Self {
        self.start_date = Some(date);
        self
    }

    /// Sets the end date of the accrual period.
    #[must_use]
    pub fn with_end_date(mut self, date: Date) -> Self {
        self.end_date = Some(date);
        self
    }

    /// Sets the futures price (e.g. 95.25).
    #[must_use]
    pub fn with_futures_price(mut self, price: f64) -> Self {
        self.futures_price = Some(price);
        self
    }

    /// Sets the contract size. Defaults to 2500.0 (CME SOFR 3M convention:
    /// notional $1M × $25/bp ÷ 100 = $2500 per point).
    #[must_use]
    pub fn with_contract_size(mut self, size: f64) -> Self {
        self.contract_size = Some(size);
        self
    }

    /// Sets the rate definition (day counter, compounding, frequency).
    #[must_use]
    pub fn with_rate_definition(mut self, rd: RateDefinition) -> Self {
        self.rate_definition = Some(rd);
        self
    }

    /// Sets the side (defaults to `LongRecieve`).
    #[must_use]
    pub fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Builds the [`RateFutures`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<RateFutures> {
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let market_index = self
            .market_index
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
