use serde::{Deserialize, Serialize};

use crate::{
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::rates::capletfloorlet::CapletFloorlet,
    time::date::Date,
    volatility::volatilityindexing::Strike,
};

/// Direction of the cap/floor strip.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CapFloorType {
    /// Cap — a strip of caplets (pays when rate exceeds strike).
    Cap,
    /// Floor — a strip of floorlets (pays when rate is below strike).
    Floor,
}

/// A [`CapFloor`] represents a strip of caplets or floorlets.
#[derive(Clone)]
pub struct CapFloor {
    identifier: String,
    caplet_floorlets: Vec<CapletFloorlet>,
    market_index: MarketIndex,
    start_date: Date,
    end_date: Date,
    currency: Currency,
    payoff_type: CapFloorType,
    strike: Strike,
}

impl CapFloor {
    /// Creates a new [`CapFloor`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        identifier: String,
        caplet_floorlets: Vec<CapletFloorlet>,
        market_index: MarketIndex,
        currency: Currency,
        start_date: Date,
        end_date: Date,
        payoff_type: CapFloorType,
        strike: Strike,
    ) -> Self {
        Self {
            identifier,
            caplet_floorlets,
            market_index,
            start_date,
            end_date,
            currency,
            payoff_type,
            strike,
        }
    }

    /// Returns the associated market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the currency.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns the strike rate.
    #[must_use]
    pub const fn strike(&self) -> Strike {
        self.strike
    }

    /// Whether this is a cap or a floor.
    #[must_use]
    pub const fn payoff_type(&self) -> CapFloorType {
        self.payoff_type
    }

    /// Returns the caplet/floorlet strip.
    #[must_use]
    pub fn caplet_floorlets(&self) -> &[CapletFloorlet] {
        &self.caplet_floorlets
    }

    /// Returns the last fixing date across all caplet/floorlets.
    #[must_use]
    pub fn last_fixing_date(&self) -> Option<Date> {
        self.caplet_floorlets
            .iter()
            .max_by(|x, y| x.fixing_date().cmp(&y.fixing_date()))
            .map(CapletFloorlet::fixing_date)
    }

    /// Return the start date of the cap/floor.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the end date of the cap/floor.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        self.end_date
    }
}

impl Instrument for CapFloor {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

/// Represents a trade on a cap/floor strip.
pub struct CapFloorTrade {
    instrument: CapFloor,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl CapFloorTrade {
    /// Creates a new [`CapFloorTrade`].
    #[must_use]
    pub const fn new(instrument: CapFloor, trade_date: Date, notional: f64, side: Side) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl Trade<CapFloor> for CapFloorTrade {
    fn instrument(&self) -> &CapFloor {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl Discountable for CapFloor {
    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
    }

    fn currency(&self) -> Currency {
        self.currency
    }

    fn discount_index(&self) -> Option<MarketIndex> {
        Some(self.market_index.clone())
    }
}
