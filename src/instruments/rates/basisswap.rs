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

/// A [`BasisSwap`] represents a floating-vs-floating interest rate swap.
///
/// Both legs reference different floating rate indices (e.g., SOFR 3M vs SOFR 1M,
/// or two different tenor indices). Each leg may carry a different spread.
pub struct BasisSwap {
    identifier: String,
    legs: Vec<Leg>,
    pay_market_index: MarketIndex,
    receive_market_index: MarketIndex,
    currency: Currency,
}

impl BasisSwap {
    /// Creates a new [`BasisSwap`].
    ///
    /// `pay_leg` is the leg being paid (index 0); `receive_leg` is the leg being received (index 1).
    #[must_use]
    pub fn new(
        identifier: String,
        pay_leg: Leg,
        receive_leg: Leg,
        pay_market_index: MarketIndex,
        receive_market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            legs: vec![pay_leg, receive_leg],
            pay_market_index,
            receive_market_index,
            currency,
        }
    }

    /// Returns a reference to the pay leg (leg 0).
    #[must_use]
    pub fn pay_leg(&self) -> &Leg {
        &self.legs[0]
    }

    /// Returns a reference to the receive leg (leg 1).
    #[must_use]
    pub fn receive_leg(&self) -> &Leg {
        &self.legs[1]
    }

    /// Returns the pay-side market index.
    #[must_use]
    pub fn pay_market_index(&self) -> MarketIndex {
        self.pay_market_index.clone()
    }

    /// Returns the receive-side market index.
    #[must_use]
    pub fn receive_market_index(&self) -> MarketIndex {
        self.receive_market_index.clone()
    }

    /// Returns the currency of the swap.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl Instrument for BasisSwap {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }
}

impl LegsProvider for BasisSwap {
    fn legs(&self) -> &[Leg] {
        &self.legs
    }
}

/// Represents a trade of a basis swap.
pub struct BasisSwapTrade {
    instrument: BasisSwap,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl BasisSwapTrade {
    /// Creates a new [`BasisSwapTrade`].
    #[must_use]
    pub const fn new(instrument: BasisSwap, trade_date: Date, notional: f64, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<BasisSwap> for BasisSwapTrade {
    fn instrument(&self) -> &BasisSwap {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for BasisSwapTrade {
    fn legs(&self) -> &[Leg] {
        self.instrument.legs()
    }
}
