use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    time::date::Date,
};

/// A [`FixedRateBond`] represents a bond that pays periodic fixed-rate coupons
/// and repays its principal at maturity.
pub struct FixedRateBond<T: IsReal> {
    identifier: String,
    units: f64,
    leg: Leg<T>,
    discount_index: Option<MarketIndex>,
    currency: Currency,
}

impl<T> FixedRateBond<T>
where
    T: IsReal,
{
    /// Creates a new [`FixedRateBond`].
    #[must_use]
    pub const fn new(
        identifier: String,
        units: f64,
        leg: Leg<T>,
        discount_index: Option<MarketIndex>,
        currency: Currency,
    ) -> Self {
        Self {
            identifier,
            units,
            leg,
            discount_index,
            currency,
        }
    }

    /// Returns the units of the bond.
    #[must_use]
    pub const fn units(&self) -> f64 {
        self.units
    }

    /// Returns a reference to the inner leg.
    #[must_use]
    pub const fn leg(&self) -> &Leg<T> {
        &self.leg
    }
}

impl<T> Instrument for FixedRateBond<T>
where
    T: IsReal,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for FixedRateBond<T>
where
    T: IsReal,
{
    fn legs(&self) -> &[Leg<T>] {
        std::slice::from_ref(&self.leg)
    }
}

impl<T> Discountable for FixedRateBond<T>
where
    T: IsReal,
{
    fn currency(&self) -> Currency {
        self.currency
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        self.discount_index.clone()
    }
}

/// Represents a trade of a fixed rate bond instrument.
pub struct FixedRateBondTrade<T: IsReal> {
    instrument: FixedRateBond<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T> FixedRateBondTrade<T>
where
    T: IsReal,
{
    /// Creates a new [`FixedRateBondTrade`].
    #[must_use]
    pub const fn new(
        instrument: FixedRateBond<T>,
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

impl<T> Trade<FixedRateBond<T>> for FixedRateBondTrade<T>
where
    T: IsReal,
{
    fn instrument(&self) -> &FixedRateBond<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl<T> LegsProvider<T> for FixedRateBondTrade<T>
where
    T: IsReal,
{
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

impl From<FixedRateBond<f64>> for FixedRateBond<ADReal> {
    fn from(value: FixedRateBond<f64>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.discount_index,
            value.currency,
        )
    }
}

impl From<FixedRateBond<ADReal>> for FixedRateBond<f64> {
    fn from(value: FixedRateBond<ADReal>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.discount_index,
            value.currency,
        )
    }
}

impl From<FixedRateBondTrade<f64>> for FixedRateBondTrade<ADReal> {
    fn from(value: FixedRateBondTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<FixedRateBondTrade<ADReal>> for FixedRateBondTrade<f64> {
    fn from(value: FixedRateBondTrade<ADReal>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}
