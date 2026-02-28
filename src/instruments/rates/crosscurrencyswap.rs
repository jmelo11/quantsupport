use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    time::date::Date,
};

/// A [`CrossCurrencySwap`] represents a swap in which two legs are denominated in different
/// currencies. Typically one leg pays a fixed (or floating) rate in one currency while the
/// other pays a floating (or fixed) rate in a second currency, with notional exchanges at
/// inception and maturity.
pub struct CrossCurrencySwap {
    identifier: String,
    legs: Vec<Leg>,
    domestic_currency: Currency,
    foreign_currency: Currency,
    domestic_market_index: MarketIndex,
    foreign_market_index: MarketIndex,
}

impl CrossCurrencySwap {
    /// Creates a new [`CrossCurrencySwap`].
    ///
    /// `domestic_leg` is the leg denominated in the domestic currency (stored at index 0);
    /// `foreign_leg` is the leg denominated in the foreign currency (stored at index 1).
    #[must_use]
    pub fn new(
        identifier: String,
        domestic_leg: Leg,
        foreign_leg: Leg,
        domestic_currency: Currency,
        foreign_currency: Currency,
        domestic_market_index: MarketIndex,
        foreign_market_index: MarketIndex,
    ) -> Self {
        Self {
            identifier,
            legs: vec![domestic_leg, foreign_leg],
            domestic_currency,
            foreign_currency,
            domestic_market_index,
            foreign_market_index,
        }
    }

    /// Returns a reference to the domestic leg (leg 0).
    #[must_use]
    pub fn domestic_leg(&self) -> &Leg {
        &self.legs[0]
    }

    /// Returns a reference to the foreign leg (leg 1).
    #[must_use]
    pub fn foreign_leg(&self) -> &Leg {
        &self.legs[1]
    }

    /// Returns the domestic currency.
    #[must_use]
    pub const fn domestic_currency(&self) -> Currency {
        self.domestic_currency
    }

    /// Returns the foreign currency.
    #[must_use]
    pub const fn foreign_currency(&self) -> Currency {
        self.foreign_currency
    }

    /// Returns the domestic market index.
    #[must_use]
    pub fn domestic_market_index(&self) -> MarketIndex {
        self.domestic_market_index.clone()
    }

    /// Returns the foreign market index.
    #[must_use]
    pub fn foreign_market_index(&self) -> MarketIndex {
        self.foreign_market_index.clone()
    }
}

impl Instrument for CrossCurrencySwap {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }
}

impl LegsProvider for CrossCurrencySwap {
    fn legs(&self) -> &[Leg] {
        &self.legs
    }
}

/// Represents a trade of a cross-currency swap.
pub struct CrossCurrencySwapTrade {
    instrument: CrossCurrencySwap,
    trade_date: Date,
    domestic_notional: f64,
    foreign_notional: f64,
    side: Side,
}

impl CrossCurrencySwapTrade {
    /// Creates a new [`CrossCurrencySwapTrade`].
    #[must_use]
    pub const fn new(
        instrument: CrossCurrencySwap,
        trade_date: Date,
        domestic_notional: f64,
        foreign_notional: f64,
        side: Side,
    ) -> Self {
        Self {
            instrument,
            trade_date,
            domestic_notional,
            foreign_notional,
            side,
        }
    }

    /// Returns the domestic notional amount.
    #[must_use]
    pub const fn domestic_notional(&self) -> f64 {
        self.domestic_notional
    }

    /// Returns the foreign notional amount.
    #[must_use]
    pub const fn foreign_notional(&self) -> f64 {
        self.foreign_notional
    }
}

impl Trade<CrossCurrencySwap> for CrossCurrencySwapTrade {
    fn instrument(&self) -> &CrossCurrencySwap {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for CrossCurrencySwapTrade {
    fn legs(&self) -> &[Leg] {
        self.instrument.legs()
    }
}
