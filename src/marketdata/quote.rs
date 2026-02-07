use serde::{Deserialize, Serialize};

use crate::currencies::currency::Currency;
use crate::marketdata::volatilitysurface::VolatilityType;
use crate::utils::errors::{AtlasError, Result};
use crate::{
    indices::marketindex::MarketIndex,
    time::{date::Date, period::Period},
};

/// # `Level`
///
/// Quote level enumeration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Level {
    /// Mid (average between Bid and Ask) price.
    Mid,
    /// Bid (buy) price.
    Bid,
    /// Ask (sell) price.
    Ask,
}

/// # `QuoteLevels`
/// Quote levels associated with an instrument identifier.
///
/// When multiple levels are provided the `mid` is preferred, otherwise `bid/ask`
/// are used to compute a fallback representative value.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct QuoteLevels {
    /// Mid price/level.
    #[serde(default)]
    mid: Option<f64>,
    /// Bid price/level.
    #[serde(default)]
    bid: Option<f64>,
    /// Ask price/level.
    #[serde(default)]
    ask: Option<f64>,
}

impl QuoteLevels {
    /// Creates quote levels from optional values.
    #[must_use]
    pub const fn new(mid: Option<f64>, bid: Option<f64>, ask: Option<f64>) -> Self {
        Self { mid, bid, ask }
    }

    /// Creates quote levels with only a mid value.
    #[must_use]
    pub const fn with_mid(mid: f64) -> Self {
        Self {
            mid: Some(mid),
            bid: None,
            ask: None,
        }
    }
    /// Returns the mid quote if available.
    #[must_use]
    pub const fn mid(&self) -> Option<f64> {
        self.mid
    }

    /// Returns the bid quote if available.
    #[must_use]
    pub const fn bid(&self) -> Option<f64> {
        self.bid
    }

    /// Returns the ask quote if available.
    #[must_use]
    pub const fn ask(&self) -> Option<f64> {
        self.ask
    }

    /// Resolves a representative quote value.
    ///
    /// ## Errors
    /// Returns an error if none of mid, bid, or ask are available.
    pub fn value(&self, level: &Level) -> Result<f64> {
        match level {
            Level::Mid => self
                .mid
                .ok_or(AtlasError::NotFoundErr("No mid quote available".into())),
            Level::Bid => self
                .bid
                .ok_or(AtlasError::NotFoundErr("No bid quote available".into())),
            Level::Ask => self
                .ask
                .ok_or(AtlasError::NotFoundErr("No ask quote available".into())),
        }
    }
}

/// # `QuoteRecord`
/// Quote record compatible with serde deserialization.
///
/// This supports JSON rows of the form:
/// `{ "instrument": "USD-SWAP|maturity=2026-01-01|strike=0.02", "mid": 0.15 }`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteRecord {
    /// Instrument identifier containing embedded metadata.
    instrument: String,
    /// Quote levels for the instrument.
    #[serde(flatten)]
    levels: QuoteLevels,
}

/// # `StrikeType`
///
/// Represents if the strike is quotes in absolute or relative values.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum StrikeType {
    /// Absolute strike.
    Absolute,
    /// Relative (moneyness) strike.
    Relative,
}

/// # `QuoteInstruments`
///
/// Represents the type of instruments that can be handled by the quoting system.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum QuoteInstrument {
    /// Deposit instrument.
    Deposit,
    /// Basis swap instrument.
    BasisSwap,
    /// OIS swap instrument.
    OIS, // this will require bootstrapping
    /// Call instrument.
    Call, // this is direct?
    /// Put instrument.
    Put,
    /// Cross currency swap instrument.
    CrossCurrencySwap,
    /// Forward points.
    ForwardPoints, // this is actually a quote type? not an instrument per-se
    /// Basis swap instrument.
    OutrightForward,
    /// Basis swap instrument.
    Future,
    /// Basis swap instrument.
    ConvexityAdjustment,
    /// Basis swap instrument.
    CapletFloorlet,
    /// Basis swap instrument.
    Swaption,
    /// Basis swap instrument.
    CapFloor, // this will require stripping
    /// Fx parity.
    Fx,
}

/// # `OptionStrategy`
///
/// Represents the strategy for which the volatility quotes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum OptionStrategy {
    /// Straddle strategy.
    Straddle,
    /// Strangle strategy.
    Strangle,
    /// Risk reversal strategy.
    RiskReversal,
    /// Butterfly strategy.
    Butterfly,
}

/// # `QuoteDetails`
///
/// A `QuoteDetails` contains all details related to a particular price. It a
/// union of various instrument types and their associated parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteDetails {
    market_index: MarketIndex,
    instrument: QuoteInstrument, // BasisSwap, OIS, CapletFloorlet, CapFloor, Swaption
    #[serde(default)]
    strategy: Option<OptionStrategy>, // Straddle, Strangle
    #[serde(default)]
    vol_type: Option<VolatilityType>, // Black, Normal
    #[serde(default)]
    rate: Option<f64>,
    #[serde(default)]
    price: Option<f64>,
    #[serde(default)]
    coupon_rate: Option<f64>,
    #[serde(default)]
    pay_currency: Option<Currency>,
    #[serde(default)]
    receive_currency: Option<Currency>,
    #[serde(default)]
    strike: Option<f64>,
    #[serde(default)]
    strike_type: Option<StrikeType>, // Absolute, relative
    #[serde(default)]
    maturity: Option<Date>,
    #[serde(default)]
    tenor: Option<Period>,
    #[serde(default)]
    vol_shift: Option<f64>,
}

impl QuoteDetails {
    /// Creates a new quote details container with required fields.
    #[must_use]
    pub fn new(market_index: MarketIndex, instrument: QuoteInstrument) -> Self {
        Self {
            market_index,
            instrument,
            strategy: None,
            vol_type: None,
            rate: None,
            price: None,
            coupon_rate: None,
            pay_currency: None,
            receive_currency: None,
            strike: None,
            strike_type: None,
            maturity: None,
            tenor: None,
            vol_shift: None,
        }
    }

    /// Parses an instrument identifier of the form `INSTRUMENT|key=value|...`.
    ///
    /// ## Errors
    /// Returns an error if the identifier is malformed.

    /// Returns the instrument base identifier.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the instrument base identifier.
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }

    /// Returns the strike, if present.
    #[must_use]
    pub const fn strike(&self) -> Option<f64> {
        self.strike
    }

    /// Returns the shift, if present.
    #[must_use]
    pub const fn shift(&self) -> Option<f64> {
        self.vol_shift
    }

    /// Returns the maturity, if present.
    #[must_use]
    pub const fn maturity(&self) -> Option<Date> {
        self.maturity
    }

    /// Returns the tenor, if present.
    #[must_use]
    pub const fn tenor(&self) -> Option<Period> {
        self.tenor
    }
}

// QuoteDetails should implement "from_str"

/// # `Quote`
///
/// Contains the quote information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quote {
    quote_details: QuoteDetails,
    quote_levels: QuoteLevels,
}

impl Quote {
    /// Creates a new quote.
    #[must_use]
    pub const fn new(quote_details: QuoteDetails, quote_levels: QuoteLevels) -> Self {
        Self {
            quote_details,
            quote_levels,
        }
    }

    /// Returns the quote details.
    #[must_use]
    pub const fn quote_details(&self) -> &QuoteDetails {
        &self.quote_details
    }

    /// Returns the quote levels.
    #[must_use]
    pub const fn quote_levels(&self) -> &QuoteLevels {
        &self.quote_levels
    }
}
