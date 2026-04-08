use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        instrument::Instrument,
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    time::date::Date,
};

/// A [`FixFloatCrossCurrencySwap`] represents a swap with legs in different currencies.
///
/// Typically one leg pays a fixed (or floating) rate in one currency while the
/// other pays a floating (or fixed) rate in a second currency, with notional exchanges at
/// inception and maturity.
#[derive(Clone)]
pub struct FixFloatCrossCurrencySwap<T: Scalar> {
    identifier: String,
    legs: Vec<Leg<T>>,
    domestic_currency: Currency,
    foreign_currency: Currency,
    forward_index: MarketIndex,
}

impl<T> FixFloatCrossCurrencySwap<T>
where
    T: Scalar,
{
    /// Creates a new [`FixFloatCrossCurrencySwap`].
    ///
    /// `domestic_leg` is the leg denominated in the domestic currency (stored at index 0);
    /// `foreign_leg` is the leg denominated in the foreign currency (stored at index 1).
    #[must_use]
    pub fn new(
        identifier: String,
        domestic_leg: Leg<T>,
        foreign_leg: Leg<T>,
        domestic_currency: Currency,
        foreign_currency: Currency,
        forward_index: MarketIndex,
    ) -> Self {
        Self {
            identifier,
            legs: vec![domestic_leg, foreign_leg],
            domestic_currency,
            foreign_currency,
            forward_index,
        }
    }

    /// Returns a reference to the domestic leg (leg 0).
    #[must_use]
    pub fn domestic_leg(&self) -> &Leg<T> {
        &self.legs[0]
    }

    /// Returns a reference to the foreign leg (leg 1).
    #[must_use]
    pub fn foreign_leg(&self) -> &Leg<T> {
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
    pub fn forward_index(&self) -> MarketIndex {
        self.forward_index.clone()
    }
}

impl<T> Instrument for FixFloatCrossCurrencySwap<T>
where
    T: Scalar,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for FixFloatCrossCurrencySwap<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        &self.legs
    }
}

/// Represents a trade of a cross-currency swap.
pub struct FixFloatCrossCurrencySwapTrade<T: Scalar> {
    instrument: FixFloatCrossCurrencySwap<T>,
    trade_date: Date,
    domestic_notional: f64,
    foreign_notional: f64,
    side: Side,
}

impl<T> FixFloatCrossCurrencySwapTrade<T>
where
    T: Scalar,
{
    /// Creates a new [`FixFloatCrossCurrencySwapTrade`].
    #[must_use]
    pub const fn new(
        instrument: FixFloatCrossCurrencySwap<T>,
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

impl<T> LegsProvider<T> for FixFloatCrossCurrencySwapTrade<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

impl<T> Trade<FixFloatCrossCurrencySwap<T>> for FixFloatCrossCurrencySwapTrade<T>
where
    T: Scalar,
{
    fn instrument(&self) -> &FixFloatCrossCurrencySwap<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

#[allow(clippy::expect_used)]
impl From<FixFloatCrossCurrencySwap<f64>> for FixFloatCrossCurrencySwap<DualFwd> {
    fn from(value: FixFloatCrossCurrencySwap<f64>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("domestic leg must exist").into(),
            legs.next().expect("foreign leg must exist").into(),
            value.domestic_currency,
            value.foreign_currency,
            value.forward_index,
        )
    }
}

#[allow(clippy::expect_used)]
impl From<FixFloatCrossCurrencySwap<DualFwd>> for FixFloatCrossCurrencySwap<f64> {
    fn from(value: FixFloatCrossCurrencySwap<DualFwd>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("domestic leg must exist").into(),
            legs.next().expect("foreign leg must exist").into(),
            value.domestic_currency,
            value.foreign_currency,
            value.forward_index,
        )
    }
}

impl From<FixFloatCrossCurrencySwapTrade<f64>> for FixFloatCrossCurrencySwapTrade<DualFwd> {
    fn from(value: FixFloatCrossCurrencySwapTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.domestic_notional,
            value.foreign_notional,
            value.side,
        )
    }
}

impl From<FixFloatCrossCurrencySwapTrade<DualFwd>> for FixFloatCrossCurrencySwapTrade<f64> {
    fn from(value: FixFloatCrossCurrencySwapTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.domestic_notional,
            value.foreign_notional,
            value.side,
        )
    }
}
