use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::{date::Date, daycounter::DayCounter},
};

/// A [`Futures`] represents an exchange-traded futures contract.
///
/// It models an exchange-traded futures contract on an underlying asset
/// (equity index, commodity, interest rate, etc.). Unlike forwards, futures are
/// marked-to-market daily through the exchange clearing house.
#[allow(clippy::struct_field_names)]
pub struct Futures {
    identifier: String,
    market_index: MarketIndex,
    expiry_date: Date,
    futures_price: f64,
    contract_size: f64,
    currency: Currency,
    day_counter: DayCounter,
}

impl Futures {
    /// Creates a new [`Futures`].
    #[must_use]
    pub const fn new(
        identifier: String,
        market_index: MarketIndex,
        expiry_date: Date,
        futures_price: f64,
        contract_size: f64,
        currency: Currency,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            identifier,
            market_index,
            expiry_date,
            futures_price,
            contract_size,
            currency,
            day_counter,
        }
    }

    /// Returns the market index for the underlying.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the expiry date of the contract.
    #[must_use]
    pub const fn expiry_date(&self) -> Date {
        self.expiry_date
    }

    /// Returns the agreed futures price.
    #[must_use]
    pub const fn futures_price(&self) -> f64 {
        self.futures_price
    }

    /// Returns the contract size (multiplier).
    #[must_use]
    pub const fn contract_size(&self) -> f64 {
        self.contract_size
    }

    /// Returns the currency of the contract.
    #[must_use]
    pub const fn currency(&self) -> &Currency {
        &self.currency
    }

    /// Returns the day count convention.
    #[must_use]
    pub const fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }
}

impl Instrument for Futures {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Equity
    }
}

/// Represents a trade (position) in a futures contract.
pub struct FuturesTrade {
    instrument: Futures,
    trade_date: Date,
    num_contracts: f64,
    side: Side,
}

impl FuturesTrade {
    /// Creates a new [`FuturesTrade`].
    #[must_use]
    pub const fn new(
        instrument: Futures,
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

    /// Returns the number of contracts in the position.
    #[must_use]
    pub const fn num_contracts(&self) -> f64 {
        self.num_contracts
    }

    /// Returns the total notional exposure (number of contracts × contract size × futures price).
    #[must_use]
    pub fn notional(&self) -> f64 {
        self.num_contracts * self.instrument.contract_size() * self.instrument.futures_price()
    }
}

impl Trade<Futures> for FuturesTrade {
    fn instrument(&self) -> &Futures {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
