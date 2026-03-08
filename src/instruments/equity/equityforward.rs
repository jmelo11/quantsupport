use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::{date::Date, daycounter::DayCounter},
};

/// An [`EquityForward`] represents a forward contract on an equity underlying (stock or index).
/// The holder agrees to buy (or sell) the underlying at a pre-agreed forward price on the
/// delivery date.
pub struct EquityForward {
    identifier: String,
    market_index: MarketIndex,
    delivery_date: Date,
    strike: f64,
    currency: Currency,
    day_counter: DayCounter,
}

impl EquityForward {
    /// Creates a new [`EquityForward`].
    #[must_use]
    pub const fn new(
        identifier: String,
        market_index: MarketIndex,
        delivery_date: Date,
        strike: f64,
        currency: Currency,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            identifier,
            market_index,
            delivery_date,
            strike,
            currency,
            day_counter,
        }
    }

    /// Returns the market index for the underlying equity.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the delivery date.
    #[must_use]
    pub const fn delivery_date(&self) -> Date {
        self.delivery_date
    }

    /// Returns the agreed forward (strike) price.
    #[must_use]
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Returns the currency.
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

impl Instrument for EquityForward {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Equity
    }
}

/// Represents a trade of an equity forward.
pub struct EquityForwardTrade {
    instrument: EquityForward,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl EquityForwardTrade {
    /// Creates a new [`EquityForwardTrade`].
    #[must_use]
    pub const fn new(
        instrument: EquityForward,
        trade_date: Date,
        notional: f64,
        side: Side,
    ) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<EquityForward> for EquityForwardTrade {
    fn instrument(&self) -> &EquityForward {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
