use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    indices::marketindex::MarketIndex,
    rates::interestrate::RateDefinition,
    time::date::Date,
};

/// A [`RateFutures`] represents an exchange-traded interest rate futures contract.
///
/// Examples include SOFR futures and Euribor futures. The contract price is quoted as
/// $100 - \text{implied rate}$, so a price of 95.00 implies a 5% rate.
///
/// The contract settles at expiry based on the realised fixing of the reference
/// rate over the accrual period `[start_date, end_date]`.
pub struct RateFutures {
    identifier: String,
    market_index: MarketIndex,
    /// IMM or contract start date (fixing date).
    start_date: Date,
    /// End of the rate accrual period.
    end_date: Date,
    /// Futures price (e.g. 95.25).
    futures_price: f64,
    /// Contract notional per basis point (e.g. $25 for CME SOFR 3M futures).
    contract_size: f64,
    /// Rate definition for the underlying rate (day counter, compounding, frequency).
    rate_definition: RateDefinition,
}

impl RateFutures {
    /// Creates a new [`RateFutures`].
    #[must_use]
    pub const fn new(
        identifier: String,
        market_index: MarketIndex,
        start_date: Date,
        end_date: Date,
        futures_price: f64,
        contract_size: f64,
        rate_definition: RateDefinition,
    ) -> Self {
        Self {
            identifier,
            market_index,
            start_date,
            end_date,
            futures_price,
            contract_size,
            rate_definition,
        }
    }

    /// Returns the market index for the reference rate.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the contract start / fixing date.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the end date of the accrual period.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        self.end_date
    }

    /// Returns the futures price.
    #[must_use]
    pub const fn futures_price(&self) -> f64 {
        self.futures_price
    }

    /// Returns the implied rate as $100 - \text{price}$ (in percentage points).
    #[must_use]
    pub fn implied_rate(&self) -> f64 {
        (100.0 - self.futures_price) / 100.0
    }

    /// Returns the contract size (notional per basis point).
    #[must_use]
    pub const fn contract_size(&self) -> f64 {
        self.contract_size
    }

    /// Returns the rate definition.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Computes the accrual factor for the period.
    #[must_use]
    pub fn accrual_factor(&self) -> f64 {
        self.rate_definition
            .day_counter()
            .year_fraction(self.start_date, self.end_date)
    }
}

impl Instrument for RateFutures {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }
}

/// Represents a trade (position) in a rate futures contract.
pub struct RateFuturesTrade {
    instrument: RateFutures,
    trade_date: Date,
    num_contracts: f64,
    side: Side,
}

impl RateFuturesTrade {
    /// Creates a new [`RateFuturesTrade`].
    #[must_use]
    pub const fn new(
        instrument: RateFutures,
        trade_date: Date,
        num_contracts: f64,
        side: Side,
    ) -> Self {
        Self {
            instrument,
            trade_date,
            num_contracts,
            side,
        }
    }

    /// Returns the number of contracts.
    #[must_use]
    pub const fn num_contracts(&self) -> f64 {
        self.num_contracts
    }

    /// Returns the total notional exposure.
    #[must_use]
    pub fn notional(&self) -> f64 {
        self.num_contracts * self.instrument.contract_size()
    }
}

impl Trade<RateFutures> for RateFuturesTrade {
    fn instrument(&self) -> &RateFutures {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
