use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    time::{date::Date, daycounter::DayCounter},
};

/// An [`FxForward`] represents a contract to exchange a notional amount of one currency
/// for another at a pre-agreed forward rate on a specified delivery date.
pub struct FxForward {
    identifier: String,
    delivery_date: Date,
    forward_rate: f64,
    base_currency: Currency,
    quote_currency: Currency,
    day_counter: DayCounter,
}

impl FxForward {
    /// Creates a new [`FxForward`].
    #[must_use]
    pub const fn new(
        identifier: String,
        delivery_date: Date,
        forward_rate: f64,
        base_currency: Currency,
        quote_currency: Currency,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            identifier,
            delivery_date,
            forward_rate,
            base_currency,
            quote_currency,
            day_counter,
        }
    }

    /// Returns the delivery date.
    #[must_use]
    pub const fn delivery_date(&self) -> Date {
        self.delivery_date
    }

    /// Returns the agreed forward rate.
    #[must_use]
    pub const fn forward_rate(&self) -> f64 {
        self.forward_rate
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
