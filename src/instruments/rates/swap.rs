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

/// A [`Swap`] represents a vanilla fixed-float interest rate swap with two legs:
/// a fixed-rate leg and a floating-rate leg.
pub struct Swap<T: IsReal> {
    identifier: String,
    legs: Vec<Leg<T>>,
    market_index: MarketIndex,
    currency: Currency,
}

impl<T> Swap<T>
where
    T: IsReal,
{
    /// Creates a new [`Swap`].
    ///
    /// `legs[0]` is the fixed leg; `legs[1]` is the floating leg.
    #[must_use]
    pub fn new(
        identifier: String,
        fixed_leg: Leg<T>,
        floating_leg: Leg<T>,
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
    pub fn fixed_leg(&self) -> &Leg<T> {
        &self.legs[0]
    }

    /// Returns a reference to the floating leg (leg 1).
    #[must_use]
    pub fn floating_leg(&self) -> &Leg<T> {
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

impl<T> Instrument for Swap<T>
where
    T: IsReal,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }
}

impl LegsProvider for Swap<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        &self.legs
    }
}

/// Represents a trade of an interest rate swap.
pub struct SwapTrade<T: IsReal> {
    instrument: Swap<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> SwapTrade<T>
where
    T: IsReal,
{
    /// Creates a new [`SwapTrade`].
    #[must_use]
    pub const fn new(instrument: Swap<T>, trade_date: Date, notional: f64, side: Side) -> Self {
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

impl<T> Trade<Swap<T>> for SwapTrade<T>
where
    T: IsReal,
{
    fn instrument(&self) -> &Swap<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for SwapTrade<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        self.instrument.legs()
    }
}

impl From<Swap<f64>> for Swap<ADReal> {
    fn from(value: Swap<f64>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("fixed leg must exist").into(),
            legs.next().expect("floating leg must exist").into(),
            value.market_index,
            value.currency,
        )
    }
}

impl From<Swap<ADReal>> for Swap<f64> {
    fn from(value: Swap<ADReal>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("fixed leg must exist").into(),
            legs.next().expect("floating leg must exist").into(),
            value.market_index,
            value.currency,
        )
    }
}

impl From<SwapTrade<f64>> for SwapTrade<ADReal> {
    fn from(value: SwapTrade<f64>) -> Self {
        Self::new(value.instrument.into(), value.trade_date, value.notional, value.side)
    }
}

impl From<SwapTrade<ADReal>> for SwapTrade<f64> {
    fn from(value: SwapTrade<ADReal>) -> Self {
        Self::new(value.instrument.into(), value.trade_date, value.notional, value.side)
    }
}
