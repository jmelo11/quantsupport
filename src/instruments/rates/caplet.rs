use serde::{Deserialize, Serialize};

use crate::{
    core::{contextmanager::ContextManager, instrument::Instrument, trade::Trade},
    indices::marketindex::MarketIndex,
    rates::interestrate::RateDefinition,
    time::date::Date,
    utils::errors::Result,
};

/// Option type for interest rate cap/floor products.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CapFloorType {
    /// Cap: a call option on the floating rate (pays if rate > strike).
    Cap,
    /// Floor: a put option on the floating rate (pays if rate < strike).
    Floor,
}

/// # `Caplet`
///
/// Represents a single caplet (or floorlet) instrument under the Black model.
///
/// A caplet covers one period of an interest rate cap. It pays
/// `max(L(T_start) - K, 0) * α * N` at `T_pay`, where `L` is the floating
/// rate fixing at `start_date`, `K` is the strike, `α` is the accrual factor
/// for the period `[start_date, end_date]`, and `N` is the notional.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Caplet {
    name: String,
    market_index: MarketIndex,
    /// Fixing / option expiry date.
    start_date: Date,
    /// End of the floating period.
    end_date: Date,
    /// Payment date (defaults to `end_date`).
    payment_date: Date,
    /// Cap (call) or floor (put) type.
    option_type: CapFloorType,
    /// Strike rate.
    strike: f64,
    /// Rate definition used to compute the accrual factor.
    rate_definition: RateDefinition,
}

impl Caplet {
    /// Creates a new `Caplet`.
    #[must_use]
    pub const fn new(
        name: String,
        market_index: MarketIndex,
        start_date: Date,
        end_date: Date,
        payment_date: Date,
        option_type: CapFloorType,
        strike: f64,
        rate_definition: RateDefinition,
    ) -> Self {
        Self {
            name,
            market_index,
            start_date,
            end_date,
            payment_date,
            option_type,
            strike,
            rate_definition,
        }
    }

    /// Returns the market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the fixing / expiry date of the option.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the end date of the floating period.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        self.end_date
    }

    /// Returns the payment date of the caplet.
    #[must_use]
    pub const fn payment_date(&self) -> Date {
        self.payment_date
    }

    /// Returns the option type (cap or floor).
    #[must_use]
    pub const fn option_type(&self) -> CapFloorType {
        self.option_type
    }

    /// Returns the strike rate.
    #[must_use]
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Returns the rate definition used to compute the accrual factor.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Computes the accrual factor `α = year_fraction(start_date, end_date)`.
    #[must_use]
    pub fn accrual_factor(&self) -> f64 {
        self.rate_definition
            .day_counter()
            .year_fraction(self.start_date, self.end_date)
    }
}

impl Instrument for Caplet {
    fn identifier(&self) -> String {
        self.name.clone()
    }

    fn resolve(&self, _: &ContextManager) -> Result<Self> {
        Ok(self.clone())
    }
}

/// # `CapletTrade`
///
/// Represents a trade on a single caplet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapletTrade {
    instrument: Caplet,
    trade_date: Date,
    notional: f64,
}

impl CapletTrade {
    /// Creates a new `CapletTrade`.
    #[must_use]
    pub const fn new(instrument: Caplet, trade_date: Date, notional: f64) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
        }
    }

    /// Returns the notional of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<Caplet> for CapletTrade {
    fn instrument(&self) -> Caplet {
        self.instrument.clone()
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }
}
