use crate::{
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

/// A [`FixedRateBond`] represents a bond that pays periodic fixed-rate coupons
/// and repays its principal at maturity.
pub struct FixedRateBond {
    identifier: String,
    units: f64,
    leg: Leg,
    market_index: MarketIndex,
    currency: Currency,
}

impl FixedRateBond {
    /// Creates a new [`FixedRateBond`].
    #[must_use]
    pub const fn new(
        identifier: String,
        units: f64,
        leg: Leg,
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

    /// Returns the units of the bond.
    #[must_use]
    pub const fn units(&self) -> f64 {
        self.units
    }

    /// Returns a reference to the inner leg.
    #[must_use]
    pub const fn leg(&self) -> &Leg {
        &self.leg
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the currency of payment.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl Instrument for FixedRateBond {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
}

impl LegsProvider for FixedRateBond {
    fn legs(&self) -> &[Leg] {
        std::slice::from_ref(&self.leg)
    }
}

/// Represents a trade of a fixed rate bond instrument.
pub struct FixedRateBondTrade {
    instrument: FixedRateBond,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl FixedRateBondTrade {
    /// Creates a new [`FixedRateBondTrade`].
    #[must_use]
    pub const fn new(
        instrument: FixedRateBond,
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

impl Trade<FixedRateBond> for FixedRateBondTrade {
    fn instrument(&self) -> &FixedRateBond {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for FixedRateBondTrade {
    fn legs(&self) -> &[Leg] {
        self.instrument.legs()
    }
}
