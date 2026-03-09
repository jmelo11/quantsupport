use crate::{
    core::{
        collateral::HasCurrency,
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::{date::Date, daycounter::DayCounter},
};

/// Represents the type of a European option.
#[derive(Clone)]
pub enum EuroOptionType {
    /// Call option type.
    Call,
    /// Put option type.
    Put,
}

/// # `EquityEuropeanOption`
///
/// Represents a European equity option instrument.
#[derive(Clone)]
pub struct EquityEuropeanOption {
    /// The market index for this option.
    market_index: MarketIndex,
    /// The expiry date of the option.
    expiry_date: Date,
    /// The strike price of the option.
    strike: f64,
    /// The type of the option (Call or Put).
    option_type: EuroOptionType,
    /// The unique identifier for this option.
    identifier: String,
    /// Day count convention for the option (e.g., "ACT/365").
    day_counter: DayCounter,
    /// Currency
    currency: Currency,
}

/// Represents a trade of a European equity option.
pub struct EquityEuropeanOptionTrade {
    /// The underlying instrument.
    instrument: EquityEuropeanOption,
    /// The notional amount of the trade.
    notional: f64,
    /// The date the trade was executed.
    trade_date: Date,
    /// Side of the trade
    side: Side,
}

impl EquityEuropeanOption {
    /// Creates a new european equity option.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        expiry_date: Date,
        strike: f64,
        option_type: EuroOptionType,
        identifier: String,
    ) -> Self {
        Self {
            market_index,
            expiry_date,
            strike,
            option_type,
            identifier,
            day_counter: DayCounter::Actual360,
            currency: Currency::USD,
        }
    }

    /// Returns the market index of this option.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the expiry date of this option.
    #[must_use]
    pub const fn expiry_date(&self) -> Date {
        self.expiry_date
    }

    /// Returns the strike price of this option.
    #[must_use]
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Returns the type of this option.
    #[must_use]
    pub const fn option_type(&self) -> &EuroOptionType {
        &self.option_type
    }

    /// Returns the day count convention of this option.
    #[must_use]
    pub const fn day_counter(&self) -> &DayCounter {
        &self.day_counter
    }
}

impl HasCurrency for EquityEuropeanOption {
    fn currency(&self) -> Currency {
        self.currency
    }
}

impl EquityEuropeanOptionTrade {
    /// Creates a new equity option trade.
    #[must_use]
    pub const fn new(
        instrument: EquityEuropeanOption,
        notional: f64,
        trade_date: Date,
        side: Side,
    ) -> Self {
        Self {
            instrument,
            notional,
            trade_date,
            side,
        }
    }

    /// Returns the notional amount of this trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Instrument for EquityEuropeanOption {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::Equity
    }
}

impl Trade<EquityEuropeanOption> for EquityEuropeanOptionTrade {
    fn instrument(&self) -> &EquityEuropeanOption {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
