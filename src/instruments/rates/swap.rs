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

/// A [`Swap`] represents a vanilla fixed-float interest rate swap with two legs:
/// a fixed-rate leg and a floating-rate leg.
pub struct Swap {
    identifier: String,
    legs: Vec<Leg>,
    market_index: MarketIndex,
    currency: Currency,
}

impl Swap {
    /// Creates a new [`Swap`].
    ///
    /// `legs[0]` is the fixed leg; `legs[1]` is the floating leg.
    #[must_use]
    pub fn new(
        identifier: String,
        fixed_leg: Leg,
        floating_leg: Leg,
        market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            legs: vec![fixed_leg, floating_leg],
            market_index,
            currency,
        }
    }

    /// Returns a reference to the fixed leg (leg 0).
    #[must_use]
    pub fn fixed_leg(&self) -> &Leg {
        &self.legs[0]
    }

    /// Returns a reference to the floating leg (leg 1).
    #[must_use]
    pub fn floating_leg(&self) -> &Leg {
        &self.legs[1]
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the currency of the swap.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl Instrument for Swap {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }
}

impl LegsProvider for Swap {
    fn legs(&self) -> &[Leg] {
        &self.legs
    }
}

/// Represents a trade of an interest rate swap.
pub struct SwapTrade {
    instrument: Swap,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl SwapTrade {
    /// Creates a new [`SwapTrade`].
    #[must_use]
    pub const fn new(instrument: Swap, trade_date: Date, notional: f64, side: Side) -> Self {
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

impl Trade<Swap> for SwapTrade {
    fn instrument(&self) -> &Swap {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for SwapTrade {
    fn legs(&self) -> &[Leg] {
        self.instrument.legs()
    }
}
