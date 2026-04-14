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

/// Swaption option type — payer or receiver.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SwaptionType {
    /// Right to enter a **payer** swap (pay fixed / receive floating).
    Payer,
    /// Right to enter a **receiver** swap (receive fixed / pay floating).
    Receiver,
}

/// A [`EuropeanSwaption`] represents an option on an interest rate swap.
///
/// The holder has the right, but not the obligation, to enter into
/// the underlying [`Swap`] at expiry.
#[derive(Clone)]
pub struct EuropeanSwaption<T: Scalar> {
    identifier: String,
    underlying: Swap<T>,
    expiry_date: Date,
    underlying_type: SwaptionType,
    strike: f64,
    market_index: MarketIndex,
    currency: Currency,
}

impl<T> EuropeanSwaption<T>
where
    T: Scalar,
{
    /// Creates a new [`Swaption`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identifier: String,
        underlying: Swap<T>,
        expiry_date: Date,
        underlying_type: SwaptionType,
        strike: f64,
        market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            underlying,
            expiry_date,
            underlying_type,
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
    pub const fn expiry_date(&self) -> Date {
        self.expiry_date
    }

    /// Returns the swaption type (payer or receiver).
    #[must_use]
    pub const fn underlying_type(&self) -> SwaptionType {
        self.underlying_type
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

impl<T> Instrument for EuropeanSwaption<T>
where
    T: Scalar,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for EuropeanSwaption<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        self.underlying.legs()
    }
}

/// Represents a trade on a swaption.
pub struct EuropeanSwaptionTrade<T: Scalar> {
    instrument: EuropeanSwaption<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> EuropeanSwaptionTrade<T>
where
    T: Scalar,
{
    /// Creates a new [`EuropeanSwaptionTrade`].
    #[must_use]
    pub const fn new(
        instrument: EuropeanSwaption<T>,
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

impl<T> Trade<EuropeanSwaption<T>> for EuropeanSwaptionTrade<T>
where
    T: Scalar,
{
    fn instrument(&self) -> &EuropeanSwaption<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl From<EuropeanSwaption<f64>> for EuropeanSwaption<DualFwd> {
    fn from(value: EuropeanSwaption<f64>) -> Self {
        Self::new(
            value.identifier,
            value.underlying.into(),
            value.expiry_date,
            value.underlying_type,
            value.strike,
            value.market_index,
            value.currency,
        )
    }
}

impl From<EuropeanSwaption<DualFwd>> for EuropeanSwaption<f64> {
    fn from(value: EuropeanSwaption<DualFwd>) -> Self {
        Self::new(
            value.identifier,
            value.underlying.into(),
            value.expiry_date,
            value.underlying_type,
            value.strike,
            value.market_index,
            value.currency,
        )
    }
}

impl From<EuropeanSwaptionTrade<f64>> for EuropeanSwaptionTrade<DualFwd> {
    fn from(value: EuropeanSwaptionTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<EuropeanSwaptionTrade<DualFwd>> for EuropeanSwaptionTrade<f64> {
    fn from(value: EuropeanSwaptionTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}
