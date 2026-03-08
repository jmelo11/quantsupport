use serde::{Deserialize, Serialize};

use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    indices::marketindex::MarketIndex,
    rates::interestrate::RateDefinition,
    time::date::Date,
    volatility::volatilityindexing::Strike,
};

/// Option type for a single-period caplet/floorlet.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CapletFloorletType {
    /// Caplet: a call option on the floating rate (pays if rate > strike).
    Caplet,
    /// Floorlet: a put option on the floating rate (pays if rate < strike).
    Floorlet,
}

/// Represents a single caplet or floorlet instrument under the Black model.
///
/// A caplet covers one period of an interest rate cap. It pays
/// $max(L(T_\text{start}) - K, 0) * \alpha * N$ at $T_\text{pay}$, where $L$ is the floating
/// rate fixing at `start_date`, $K$ is the strike, $\alpha$ is the accrual factor
/// for the period `[start_date, end_date]`, and `N` is the notional.
///
/// Collateralization and payment discounting conventions are determined at the
/// context / `MarketDataProvider` level, not on the instrument itself.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapletFloorlet {
    name: String,
    market_index: MarketIndex,
    /// Fixing / option expiry date.
    start_date: Date,
    /// End of the floating period.
    end_date: Date,
    /// Payment date (defaults to `end_date`).
    payment_date: Date,
    /// Caplet or floorlet direction.
    option_type: CapletFloorletType,
    /// Strike specification (absolute, ATM, or relative to forward).
    strike: Strike,
    /// Rate definition used to derive the forward rate and accrual factor.
    rate_definition: RateDefinition,
}

impl CapletFloorlet {
    /// Creates a new `CapletFloorlet`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        name: String,
        market_index: MarketIndex,
        start_date: Date,
        end_date: Date,
        payment_date: Date,
        option_type: CapletFloorletType,
        strike: Strike,
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

    /// Returns the market index (forecast curve).
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

    /// Returns the payment date of the caplet/floorlet.
    #[must_use]
    pub const fn payment_date(&self) -> Date {
        self.payment_date
    }

    /// Returns the option type (caplet or floorlet).
    #[must_use]
    pub const fn option_type(&self) -> CapletFloorletType {
        self.option_type
    }

    /// Returns the strike specification.
    #[must_use]
    pub const fn strike(&self) -> Strike {
        self.strike
    }

    /// Returns the rate definition used to derive the forward rate and accrual factor.
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

impl Instrument for CapletFloorlet {
    fn identifier(&self) -> String {
        self.name.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }
}

/// Represents a trade on a single caplet or floorlet.
#[derive(Clone)]
pub struct CapletFloorletTrade {
    instrument: CapletFloorlet,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl CapletFloorletTrade {
    /// Creates a new [`CapletFloorletTrade`].
    #[must_use]
    pub const fn new(
        instrument: CapletFloorlet,
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

    /// Returns the notional of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<CapletFloorlet> for CapletFloorletTrade {
    fn instrument(&self) -> &CapletFloorlet {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}
