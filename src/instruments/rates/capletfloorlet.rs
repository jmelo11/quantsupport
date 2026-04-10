use serde::{Deserialize, Serialize};

use crate::{
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
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
    identifier: String,
    market_index: MarketIndex,
    /// Currency of the caplet/floorlet.
    currency: Currency,
    /// Rate fixing date.
    fixing_date: Date,
    /// Start of the accrual period.
    start_accrual_date: Date,
    /// End of the accrual period.
    end_accrual_date: Date,
    /// Payment date (defaults to `end_date`).
    payment_date: Date,
    /// Caplet or floorlet direction.
    payoff_type: CapletFloorletType,
    /// Strike specification (absolute, ATM, or relative to forward).
    strike: Strike,
}

impl CapletFloorlet {
    /// Creates a new `CapletFloorlet`.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identifier: String,
        market_index: MarketIndex,
        currency: Currency,
        fixing_date: Date,
        start_accrual_date: Date,
        end_accrual_date: Date,
        payment_date: Date,
        payoff_type: CapletFloorletType,
        strike: Strike,
    ) -> Self {
        Self {
            identifier,
            market_index,
            currency,
            fixing_date,
            start_accrual_date,
            end_accrual_date,
            payment_date,
            payoff_type,
            strike,
        }
    }

    /// Returns the market index (forecast curve).
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the fixing / expiry date of the option.
    #[must_use]
    pub const fn start_accrual_date(&self) -> Date {
        self.start_accrual_date
    }

    /// Returns the end date of the floating period.
    #[must_use]
    pub const fn end_accrual_date(&self) -> Date {
        self.end_accrual_date
    }

    /// Returns the fixing date of the caplet/floorlet.
    #[must_use]
    pub const fn fixing_date(&self) -> Date {
        self.fixing_date
    }

    /// Returns the payment date of the caplet/floorlet.
    #[must_use]
    pub const fn payment_date(&self) -> Date {
        self.payment_date
    }

    /// Returns the option type (caplet or floorlet).
    #[must_use]
    pub const fn payoff_type(&self) -> CapletFloorletType {
        self.payoff_type
    }

    /// Returns the strike specification.
    #[must_use]
    pub const fn strike(&self) -> Strike {
        self.strike
    }
}

impl Discountable for CapletFloorlet {
    fn currency(&self) -> Currency {
        self.currency
    }
    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        None
    }
}

impl Instrument for CapletFloorlet {
    fn identifier(&self) -> String {
        self.identifier.clone()
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
