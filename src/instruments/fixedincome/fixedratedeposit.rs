use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        collateral::HasCurrency,
        instrument::{AssetClass, Instrument},
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    time::date::Date,
};

/// A [`FixedRateDeposit`] represents a fixed-rate cash deposit with a single payment at the end (capital plus interest).
pub struct FixedRateDeposit<T: IsReal> {
    identifier: String,
    units: f64,
    leg: Leg<T>,
    market_index: MarketIndex,
    currency: Currency,
}

impl<T> FixedRateDeposit<T>
where
    T: IsReal,
{
    /// Creates a new [`FixedRateDeposit`].
    #[must_use]
    pub const fn new(
        identifier: String,
        units: f64,
        leg: Leg<T>,
        market_index: MarketIndex,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            units,
            leg,
            market_index,
            currency,
        }
    }

    /// Returns the units of the deposit.
    #[must_use]
    pub const fn units(&self) -> f64 {
        self.units
    }

    /// Returns a reference to the inner leg.
    #[must_use]
    pub const fn leg(&self) -> &Leg<T> {
        &self.leg
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }
}

impl<T> HasCurrency for FixedRateDeposit<T>
where
    T: IsReal,
{
    fn currency(&self) -> Currency {
        self.currency
    }
}

impl<T> Instrument for FixedRateDeposit<T>
where
    T: IsReal,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
}

impl LegsProvider for FixedRateDeposit<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        std::slice::from_ref(&self.leg)
    }
}

/// Represents a trade of a deposit instrument.
pub struct FixedRateDepositTrade<T: IsReal> {
    instrument: FixedRateDeposit<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> FixedRateDepositTrade<T>
where
    T: IsReal,
{
    /// Creates a new [`FixedRateDepositTrade`].
    #[must_use]
    pub const fn new(
        instrument: FixedRateDeposit<T>,
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

    /// Returns the notional amount of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl<T> Trade<FixedRateDeposit<T>> for FixedRateDepositTrade<T>
where
    T: IsReal,
{
    fn instrument(&self) -> &FixedRateDeposit<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for FixedRateDepositTrade<ADReal> {
    fn legs(&self) -> &[Leg<ADReal>] {
        self.instrument.legs()
    }
}

impl From<FixedRateDeposit<f64>> for FixedRateDeposit<ADReal> {
    fn from(value: FixedRateDeposit<f64>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.market_index,
            value.currency,
        )
    }
}

impl From<FixedRateDeposit<ADReal>> for FixedRateDeposit<f64> {
    fn from(value: FixedRateDeposit<ADReal>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.market_index,
            value.currency,
        )
    }
}

impl From<FixedRateDepositTrade<f64>> for FixedRateDepositTrade<ADReal> {
    fn from(value: FixedRateDepositTrade<f64>) -> Self {
        Self::new(value.instrument.into(), value.trade_date, value.notional, value.side)
    }
}

impl From<FixedRateDepositTrade<ADReal>> for FixedRateDepositTrade<f64> {
    fn from(value: FixedRateDepositTrade<ADReal>) -> Self {
        Self::new(value.instrument.into(), value.trade_date, value.notional, value.side)
    }
}
