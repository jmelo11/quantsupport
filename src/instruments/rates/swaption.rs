use serde::{Deserialize, Serialize};

use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{cashflows::leg::Leg, rates::swap::Swap},
    time::date::Date,
};

/// Swaption exercise type (European only for now).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SwaptionExerciseType {
    /// European — exercisable only at expiry.
    European,
}

/// Swaption option type — payer or receiver.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SwaptionType {
    /// Right to enter a **payer** swap (pay fixed / receive floating).
    Payer,
    /// Right to enter a **receiver** swap (receive fixed / pay floating).
    Receiver,
}

/// A [`Swaption`] represents an option on an interest rate swap.
///
/// The holder has the right, but not the obligation, to enter into
/// the underlying [`Swap`] at expiry.
pub struct Swaption {
    identifier: String,
    underlying: Swap,
    expiry: Date,
    swaption_type: SwaptionType,
    exercise_type: SwaptionExerciseType,
    strike: f64,
    market_index: MarketIndex,
    currency: Currency,
}

impl Swaption {
    /// Creates a new [`Swaption`].
    #[must_use]
    pub const fn new(
        identifier: String,
        underlying: Swap,
        expiry: Date,
        swaption_type: SwaptionType,
        exercise_type: SwaptionExerciseType,
        strike: f64,
        market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            underlying,
            expiry,
            swaption_type,
            exercise_type,
            strike,
            market_index,
            currency,
        }
    }

    /// Returns a reference to the underlying swap.
    #[must_use]
    pub const fn underlying(&self) -> &Swap {
        &self.underlying
    }

    /// Returns the option expiry date.
    #[must_use]
    pub const fn expiry(&self) -> Date {
        self.expiry
    }

    /// Returns the swaption type (payer or receiver).
    #[must_use]
    pub const fn swaption_type(&self) -> SwaptionType {
        self.swaption_type
    }

    /// Returns the exercise type.
    #[must_use]
    pub const fn exercise_type(&self) -> SwaptionExerciseType {
        self.exercise_type
    }

    /// Returns the strike (fixed rate of the underlying swap).
    #[must_use]
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the currency.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl Instrument for Swaption {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }
}

impl LegsProvider for Swaption {
    fn legs(&self) -> &[Leg] {
        self.underlying.legs()
    }
}

/// Represents a trade on a swaption.
pub struct SwaptionTrade {
    instrument: Swaption,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl SwaptionTrade {
    /// Creates a new [`SwaptionTrade`].
    #[must_use]
    pub const fn new(
        instrument: Swaption,
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

impl Trade<Swaption> for SwaptionTrade {
    fn instrument(&self) -> &Swaption {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for SwaptionTrade {
    fn legs(&self) -> &[Leg] {
        self.instrument.legs()
    }
}
