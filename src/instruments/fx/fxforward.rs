use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    time::{date::Date, daycounter::DayCounter},
};

/// Settlement convention for an [`FxForward`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxForwardSettlement {
    /// Physical delivery of the two currencies on the delivery date.
    Deliverable,
    /// Cash settlement against a fixing observed before settlement.
    NonDeliverable {
        /// Date on which the fixing used for cash settlement is observed.
        fixing_date: Date,
        /// Currency in which the NDF cash difference is settled.
        settlement_currency: Currency,
    },
}

/// An [`FxForward`] represents a contract to exchange a notional amount of one currency
/// for another on a specified delivery date.
///
/// The contract can be quoted either as an outright forward price or as forward points,
/// and can settle physically or as a non-deliverable forward (NDF).
#[derive(Debug)]
pub struct FxForward {
    identifier: String,
    delivery_date: Date,
    forward_price: Option<f64>,
    forward_points: Option<f64>,
    base_currency: Currency,
    quote_currency: Currency,
    day_counter: DayCounter,
    settlement: FxForwardSettlement,
}

impl FxForward {
    /// Creates a new [`FxForward`].
    #[must_use]
    pub const fn new(
        identifier: String,
        delivery_date: Date,
        forward_price: Option<f64>,
        forward_points: Option<f64>,
        base_currency: Currency,
        quote_currency: Currency,
        day_counter: DayCounter,
        settlement: FxForwardSettlement,
    ) -> Self {
        Self {
            identifier,
            delivery_date,
            forward_price,
            forward_points,
            base_currency,
            quote_currency,
            day_counter,
            settlement,
        }
    }

    /// Returns the delivery date.
    #[must_use]
    pub const fn delivery_date(&self) -> Date {
        self.delivery_date
    }

    /// Returns the agreed outright forward price, when quoted directly.
    #[must_use]
    pub const fn forward_price(&self) -> Option<f64> {
        self.forward_price
    }

    /// Returns the agreed forward rate.
    #[must_use]
    pub const fn forward_rate(&self) -> Option<f64> {
        self.forward_price
    }

    /// Returns the forward points (the difference between the forward rate and the spot rate).
    #[must_use]
    pub const fn forward_points(&self) -> Option<f64> {
        self.forward_points
    }

    /// Returns true if this trade is quoted as an outright forward price.
    #[must_use]
    pub const fn is_outright(&self) -> bool {
        self.forward_price.is_some()
    }

    /// Returns true if this trade stores forward points.
    #[must_use]
    pub const fn has_forward_points(&self) -> bool {
        self.forward_points.is_some()
    }

    /// Returns the settlement convention.
    #[must_use]
    pub const fn settlement(&self) -> FxForwardSettlement {
        self.settlement
    }

    /// Returns true if the contract settles by physical delivery.
    #[must_use]
    pub const fn is_deliverable(&self) -> bool {
        match self.settlement {
            FxForwardSettlement::Deliverable => true,
            FxForwardSettlement::NonDeliverable { .. } => false,
        }
    }

    /// Returns true if the contract is a non-deliverable forward.
    #[must_use]
    pub const fn is_ndf(&self) -> bool {
        !self.is_deliverable()
    }

    /// Returns the fixing date for an NDF, if any.
    #[must_use]
    pub const fn fixing_date(&self) -> Option<Date> {
        match self.settlement {
            FxForwardSettlement::Deliverable => None,
            FxForwardSettlement::NonDeliverable { fixing_date, .. } => Some(fixing_date),
        }
    }

    /// Returns the settlement currency for an NDF, if any.
    #[must_use]
    pub const fn settlement_currency(&self) -> Option<Currency> {
        match self.settlement {
            FxForwardSettlement::Deliverable => None,
            FxForwardSettlement::NonDeliverable {
                settlement_currency,
                ..
            } => Some(settlement_currency),
        }
    }

    /// Returns the base currency (the currency being bought).
    #[must_use]
    pub const fn base_currency(&self) -> Currency {
        self.base_currency
    }

    /// Returns the quote currency (the currency being sold).
    #[must_use]
    pub const fn quote_currency(&self) -> Currency {
        self.quote_currency
    }

    /// Returns the day count convention.
    #[must_use]
    pub const fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }
}

impl Instrument for FxForward {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }
}

/// Represents a trade of an FX forward.
pub struct FxForwardTrade {
    instrument: FxForward,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl FxForwardTrade {
    /// Creates a new [`FxForwardTrade`].
    ///
    /// `notional` is in base-currency terms.
    #[must_use]
    pub const fn new(instrument: FxForward, trade_date: Date, notional: f64, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount in the base currency.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<FxForward> for FxForwardTrade {
    fn instrument(&self) -> &FxForward {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
