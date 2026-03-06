use serde::{Deserialize, Serialize};

use crate::{
    core::{
        instrument::{AssetClass, Instrument},
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::rates::capletfloorlet::CapletFloorlet,
    time::date::Date,
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
pub struct CapFloor {
    identifier: String,
    caplet_floorlets: Vec<CapletFloorlet>,
    market_index: MarketIndex,
    currency: Currency,
    strike: f64,
    cap_floor_type: CapFloorType,
}

impl CapFloor {
    /// Creates a new [`CapFloor`].
    #[must_use]
    pub fn new(
        identifier: String,
        caplet_floorlets: Vec<CapletFloorlet>,
        market_index: MarketIndex,
        currency: Currency,
        strike: f64,
        cap_floor_type: CapFloorType,
    ) -> Self {
        Self {
            identifier,
            caplet_floorlets,
            market_index,
            currency,
            strike,
            cap_floor_type,
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
    pub const fn strike(&self) -> f64 {
        self.strike
    }

    /// Whether this is a cap or a floor.
    #[must_use]
    pub const fn cap_floor_type(&self) -> CapFloorType {
        self.cap_floor_type
    }

    /// Returns the caplet/floorlet strip.
    #[must_use]
    pub fn caplet_floorlets(&self) -> &[CapletFloorlet] {
        &self.caplet_floorlets
    }
}

impl Instrument for CapFloor {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::InterestRate
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
