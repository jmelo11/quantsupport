use crate::{
    core::{contextmanager::ContextManager, instrument::Instrument, trade::Trade},
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::Result,
};

/// Represents the type of a European option.
#[derive(Clone)]
pub enum EuroOptionType {
    /// Call option type.
    Call,
    /// Put option type.
    Put,
}

/// Represents a European equity option instrument.
#[derive(Clone)]
pub struct EquityEuroOption {
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
}

/// Represents a trade of a European equity option.
pub struct EquityEuroOptionTrade {
    /// The underlying instrument.
    instrument: EquityEuroOption,
    /// The notional amount of the trade.
    notional: f64,
    /// The date the trade was executed.
    trade_date: Date,
}

impl EquityEuroOption {
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
}

impl EquityEuroOptionTrade {
    /// Creates a new equity option trade.
    #[must_use]
    pub const fn new(instrument: EquityEuroOption, notional: f64, trade_date: Date) -> Self {
        Self {
            instrument,
            notional,
            trade_date,
        }
    }

    /// Returns the notional amount of this trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Instrument for EquityEuroOption {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn resolve(&self, _: &ContextManager) -> Result<Self> {
        Ok(self.clone())
    }
}

impl Trade<EquityEuroOption> for EquityEuroOptionTrade {
    fn instrument(&self) -> EquityEuroOption {
        self.instrument.clone()
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }
}
