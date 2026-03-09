use crate::{
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

/// A [`FloatingRateNote`] represents a bond that pays periodic floating-rate coupons
/// (typically referencing an interest rate index plus a spread) and repays its principal at maturity.
pub struct FloatingRateNote {
    identifier: String,
    units: f64,
    leg: Leg,
    market_index: MarketIndex,
    currency: Currency,
}

impl FloatingRateNote {
    /// Creates a new [`FloatingRateNote`].
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

    /// Returns the units of the note.
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
}

impl HasCurrency for FloatingRateNote {
    fn currency(&self) -> Currency {
        self.currency
    }
}

impl Instrument for FloatingRateNote {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
}

impl LegsProvider for FloatingRateNote {
    fn legs(&self) -> &[Leg] {
        std::slice::from_ref(&self.leg)
    }
}

/// Represents a trade of a floating rate note instrument.
pub struct FloatingRateNoteTrade {
    instrument: FloatingRateNote,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl FloatingRateNoteTrade {
    /// Creates a new [`FloatingRateNoteTrade`].
    #[must_use]
    pub const fn new(
        instrument: FloatingRateNote,
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

impl Trade<FloatingRateNote> for FloatingRateNoteTrade {
    fn instrument(&self) -> &FloatingRateNote {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl LegsProvider for FloatingRateNoteTrade {
    fn legs(&self) -> &[Leg] {
        self.instrument.legs()
    }
}
