use crate::{
    ad::adreal::{ADReal, IsReal},
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

/// A [`FloatFloatCrossCurrencySwap`] is a cross-currency swap with two floating legs.
///
/// Both legs pay floating rates (each referencing an index in its own currency) plus
/// optional spreads. Notional exchanges typically occur at inception and maturity at
/// the initial FX spot rate.
pub struct FloatFloatCrossCurrencySwap<T: IsReal> {
    identifier: String,
    legs: Vec<Leg<T>>,
    domestic_currency: Currency,
    foreign_currency: Currency,
    domestic_market_index: MarketIndex,
    foreign_market_index: MarketIndex,
}

impl<T> FloatFloatCrossCurrencySwap<T>
where
    T: IsReal,
{
    /// Creates a new [`FloatFloatCrossCurrencySwap`].
    ///
    /// `domestic_leg` is the floating leg in the domestic currency (index 0);
    /// `foreign_leg` is the floating leg in the foreign currency (index 1).
    #[must_use]
    pub fn new(
        identifier: String,
        domestic_leg: Leg<T>,
        foreign_leg: Leg<T>,
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
    pub fn domestic_market_index(&self) -> MarketIndex {
        self.domestic_market_index.clone()
    }

    /// Returns the foreign market index.
    #[must_use]
    pub fn foreign_market_index(&self) -> MarketIndex {
        self.foreign_market_index.clone()
    }
}

impl<T> Instrument for FloatFloatCrossCurrencySwap<T>
where
    T: IsReal,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }
}

impl LegsProvider for FloatFloatCrossCurrencySwap<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        &self.legs
    }
}

/// Represents a trade of a float-vs-float cross-currency swap.
pub struct FloatFloatCrossCurrencySwapTrade<T: IsReal> {
    instrument: FloatFloatCrossCurrencySwap<T>,
    trade_date: Date,
    domestic_notional: f64,
    foreign_notional: f64,
    side: Side,
}

impl<T> FloatFloatCrossCurrencySwapTrade<T>
where
    T: IsReal,
{
    /// Creates a new [`FloatFloatCrossCurrencySwapTrade`].
    #[must_use]
    pub const fn new(
        instrument: FloatFloatCrossCurrencySwap<T>,
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

impl<T> Trade<FloatFloatCrossCurrencySwap<T>> for FloatFloatCrossCurrencySwapTrade<T>
where
    T: IsReal,
{
    fn instrument(&self) -> &FloatFloatCrossCurrencySwap<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for FloatFloatCrossCurrencySwapTrade<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        self.instrument.legs()
    }
}

impl From<FloatFloatCrossCurrencySwap<f64>> for FloatFloatCrossCurrencySwap<ADReal> {
    fn from(value: FloatFloatCrossCurrencySwap<f64>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("domestic leg must exist").into(),
            legs.next().expect("foreign leg must exist").into(),
            value.domestic_currency,
            value.foreign_currency,
            value.domestic_market_index,
            value.foreign_market_index,
        )
    }
}

impl From<FloatFloatCrossCurrencySwap<ADReal>> for FloatFloatCrossCurrencySwap<f64> {
    fn from(value: FloatFloatCrossCurrencySwap<ADReal>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("domestic leg must exist").into(),
            legs.next().expect("foreign leg must exist").into(),
            value.domestic_currency,
            value.foreign_currency,
            value.domestic_market_index,
            value.foreign_market_index,
        )
    }
}

impl From<FloatFloatCrossCurrencySwapTrade<f64>> for FloatFloatCrossCurrencySwapTrade<ADReal> {
    fn from(value: FloatFloatCrossCurrencySwapTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.domestic_notional,
            value.foreign_notional,
            value.side,
        )
    }
}

impl From<FloatFloatCrossCurrencySwapTrade<ADReal>> for FloatFloatCrossCurrencySwapTrade<f64> {
    fn from(value: FloatFloatCrossCurrencySwapTrade<ADReal>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.domestic_notional,
            value.foreign_notional,
            value.side,
        )
    }
}
