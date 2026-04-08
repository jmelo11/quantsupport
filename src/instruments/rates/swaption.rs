use serde::{Deserialize, Serialize};

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        instrument::Instrument,
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
#[derive(Clone)]
pub struct Swaption<T: Scalar> {
    identifier: String,
    underlying: Swap<T>,
    expiry: Date,
    underlying_type: SwaptionType,
    exercise_type: SwaptionExerciseType,
    strike: f64,
    market_index: MarketIndex,
    currency: Currency,
}

impl<T> Swaption<T>
where
    T: Scalar,
{
    /// Creates a new [`Swaption`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identifier: String,
        underlying: Swap<T>,
        expiry: Date,
        underlying_type: SwaptionType,
        exercise_type: SwaptionExerciseType,
        strike: f64,
        market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            underlying,
            expiry,
            underlying_type,
            exercise_type,
            strike,
            market_index,
            currency,
        }
    }

    /// Returns a reference to the underlying swap.
    #[must_use]
    pub const fn underlying(&self) -> &Swap<T> {
        &self.underlying
    }

    /// Returns the option expiry date.
    #[must_use]
    pub const fn expiry(&self) -> Date {
        self.expiry
    }

    /// Returns the swaption type (payer or receiver).
    #[must_use]
    pub const fn underlying_type(&self) -> SwaptionType {
        self.underlying_type
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

impl<T> Instrument for Swaption<T>
where
    T: Scalar,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for Swaption<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        self.underlying.legs()
    }
}

/// Represents a trade on a swaption.
pub struct SwaptionTrade<T: Scalar> {
    instrument: Swaption<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> SwaptionTrade<T>
where
    T: Scalar,
{
    /// Creates a new [`SwaptionTrade`].
    #[must_use]
    pub const fn new(instrument: Swaption<T>, trade_date: Date, notional: f64, side: Side) -> Self {
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

impl<T> Trade<Swaption<T>> for SwaptionTrade<T>
where
    T: Scalar,
{
    fn instrument(&self) -> &Swaption<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl From<Swaption<f64>> for Swaption<DualFwd> {
    fn from(value: Swaption<f64>) -> Self {
        Self::new(
            value.identifier,
            value.underlying.into(),
            value.expiry,
            value.underlying_type,
            value.exercise_type,
            value.strike,
            value.market_index,
            value.currency,
        )
    }
}

impl From<Swaption<DualFwd>> for Swaption<f64> {
    fn from(value: Swaption<DualFwd>) -> Self {
        Self::new(
            value.identifier,
            value.underlying.into(),
            value.expiry,
            value.underlying_type,
            value.exercise_type,
            value.strike,
            value.market_index,
            value.currency,
        )
    }
}

impl From<SwaptionTrade<f64>> for SwaptionTrade<DualFwd> {
    fn from(value: SwaptionTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<SwaptionTrade<DualFwd>> for SwaptionTrade<f64> {
    fn from(value: SwaptionTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}
