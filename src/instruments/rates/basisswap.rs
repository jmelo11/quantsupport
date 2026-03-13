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

/// A [`BasisSwap`] represents a floating-vs-floating interest rate swap.
///
/// Both legs reference different floating rate indices (e.g., SOFR 3M vs SOFR 1M,
/// or two different tenor indices). Each leg may carry a different spread.
pub struct BasisSwap<T: IsReal> {
    identifier: String,
    legs: Vec<Leg<T>>,
    pay_market_index: MarketIndex,
    receive_market_index: MarketIndex,
    currency: Currency,
}

impl<T> BasisSwap<T>
where
    T: IsReal,
{
    /// Creates a new [`BasisSwap`].
    ///
    /// `pay_leg` is the leg being paid (index 0); `receive_leg` is the leg being received (index 1).
    #[must_use]
    pub fn new(
        identifier: String,
        pay_leg: Leg<T>,
        receive_leg: Leg<T>,
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
    pub fn pay_leg(&self) -> &Leg<T> {
        &self.legs[0]
    }

    /// Returns a reference to the receive leg (leg 1).
    #[must_use]
    pub fn receive_leg(&self) -> &Leg<T> {
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

impl<T> Instrument for BasisSwap<T>
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

impl LegsProvider for BasisSwap<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        &self.legs
    }
}

/// Represents a trade of a basis swap.
pub struct BasisSwapTrade<T: IsReal> {
    instrument: BasisSwap<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> BasisSwapTrade<T>
where
    T: IsReal,
{
    /// Creates a new [`BasisSwapTrade`].
    #[must_use]
    pub const fn new(instrument: BasisSwap<T>, trade_date: Date, notional: f64, side: Side) -> Self {
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

impl<T> Trade<BasisSwap<T>> for BasisSwapTrade<T>
where
    T: IsReal,
{
    fn instrument(&self) -> &BasisSwap<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for BasisSwapTrade<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        self.instrument.legs()
    }
}

impl From<BasisSwap<f64>> for BasisSwap<ADReal> {
    fn from(value: BasisSwap<f64>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("pay leg must exist").into(),
            legs.next().expect("receive leg must exist").into(),
            value.pay_market_index,
            value.receive_market_index,
            value.currency,
        )
    }
}

impl From<BasisSwap<ADReal>> for BasisSwap<f64> {
    fn from(value: BasisSwap<ADReal>) -> Self {
        let mut legs = value.legs.into_iter();
        Self::new(
            value.identifier,
            legs.next().expect("pay leg must exist").into(),
            legs.next().expect("receive leg must exist").into(),
            value.pay_market_index,
            value.receive_market_index,
            value.currency,
        )
    }
}

impl From<BasisSwapTrade<f64>> for BasisSwapTrade<ADReal> {
    fn from(value: BasisSwapTrade<f64>) -> Self {
        Self::new(value.instrument.into(), value.trade_date, value.notional, value.side)
    }
}

impl From<BasisSwapTrade<ADReal>> for BasisSwapTrade<f64> {
    fn from(value: BasisSwapTrade<ADReal>) -> Self {
        Self::new(value.instrument.into(), value.trade_date, value.notional, value.side)
    }
}
