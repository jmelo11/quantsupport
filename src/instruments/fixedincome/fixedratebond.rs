use super::bondpricinganchor::BondPricingAnchor;
use crate::time::daycounter::DayCounter;

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
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
#[derive(Clone)]
pub struct FixedRateBond<T: Scalar> {
    identifier: String,
    units: f64,
    leg: Leg<T>,
    discount_index: Option<MarketIndex>,
    currency: Currency,
}

impl<T> FixedRateBond<T>
where
    T: Scalar,
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
            leg: leg.with_side(Side::LongReceive),
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
    T: Scalar,
{
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T> LegsProvider<T> for FixedRateBond<T>
where
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        std::slice::from_ref(&self.leg)
    }
}

impl<T> Discountable for FixedRateBond<T>
where
    T: Scalar,
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
#[derive(Clone)]
pub struct FixedRateBondTrade<T: Scalar> {
    instrument: FixedRateBond<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
    pricing_anchor: BondPricingAnchor,
    settlement_date: Option<Date>,
    spread_day_counter: Option<DayCounter>,
}

impl<T> FixedRateBondTrade<T>
where
    T: Scalar,
{
    /// Creates a new [`FixedRateBondTrade`].
    #[must_use]
    pub const fn new(
        instrument: FixedRateBond<T>,
        trade_date: Date,
        notional: f64,
        side: Side,
        pricing_anchor: BondPricingAnchor,
    ) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
            pricing_anchor,
            settlement_date: None,
            spread_day_counter: None,
        }
    }

    /// Returns the market quote anchoring this trade's valuation.
    #[must_use]
    pub const fn pricing_anchor(&self) -> BondPricingAnchor {
        self.pricing_anchor
    }

    /// Replaces the trade's pricing anchor.
    #[must_use]
    pub const fn with_pricing_anchor(mut self, anchor: BondPricingAnchor) -> Self {
        self.pricing_anchor = anchor;
        self
    }

    /// Uses an explicit settlement date; otherwise valuation uses the context date.
    #[must_use]
    pub const fn with_settlement_date(mut self, date: Date) -> Self {
        self.settlement_date = Some(date);
        self
    }

    /// Returns the trade's explicit settlement date, if supplied.
    #[must_use]
    pub const fn settlement_date(&self) -> Option<Date> {
        self.settlement_date
    }

    /// Sets the day counter of the spread quote.
    #[must_use]
    pub const fn with_spread_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.spread_day_counter = Some(day_counter);
        self
    }

    /// Returns the explicit spread day counter, if supplied.
    #[must_use]
    pub const fn spread_day_counter(&self) -> Option<DayCounter> {
        self.spread_day_counter
    }

    /// Returns the notional amount of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl<T> Trade<FixedRateBond<T>> for FixedRateBondTrade<T>
where
    T: Scalar,
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
    T: Scalar,
{
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

impl From<FixedRateBond<f64>> for FixedRateBond<DualFwd> {
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

impl From<FixedRateBond<DualFwd>> for FixedRateBond<f64> {
    fn from(value: FixedRateBond<DualFwd>) -> Self {
        Self::new(
            value.identifier,
            value.units,
            value.leg.into(),
            value.discount_index,
            value.currency,
        )
    }
}

impl From<FixedRateBondTrade<f64>> for FixedRateBondTrade<DualFwd> {
    fn from(value: FixedRateBondTrade<f64>) -> Self {
        let mut trade = Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
            value.pricing_anchor,
        );
        trade.settlement_date = value.settlement_date;
        trade.spread_day_counter = value.spread_day_counter;
        trade
    }
}

impl From<FixedRateBondTrade<DualFwd>> for FixedRateBondTrade<f64> {
    fn from(value: FixedRateBondTrade<DualFwd>) -> Self {
        let mut trade = Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
            value.pricing_anchor,
        );
        trade.settlement_date = value.settlement_date;
        trade.spread_day_counter = value.spread_day_counter;
        trade
    }
}

impl crate::xva::makecontigentclaim::IntoContingentClaims for FixedRateBondTrade<f64> {
    fn into_contingent_claims(
        &self,
        trade_id: &str,
    ) -> crate::utils::errors::Result<Vec<crate::xva::contigentclaim::ContingentClaim>> {
        self.instrument
            .leg()
            .clone()
            .with_side(self.side)
            .into_contingent_claims(trade_id)
    }
}
