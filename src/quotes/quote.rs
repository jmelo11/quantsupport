use serde::{Deserialize, Serialize};

use crate::{
    ad::scalar::Scalar,
    currencies::currency::Currency,
    indices::{fxpair::FxPair, marketindex::MarketIndex},
    instruments::{
        equity::{
            equityeuropeanoption::{EquityEuropeanOption, EuroOptionType},
            makeequityeuropeanoption::MakeEquityEuropeanOption,
        },
        fixedincome::{
            fixedratebond::FixedRateBond, fixedratedeposit::FixedRateDeposit,
            makefixedratebond::MakeFixedRateBond, makefixedratedeposit::MakeFixedRateDeposit,
        },
        fx::{
            fxeuropeanoption::FxEuropeanOption, fxforward::FxForward,
            makefxeuropeanoption::MakeFxEuropeanOption, makefxforward::MakeFxForward,
        },
        rates::{
            basisswap::BasisSwap,
            capfloor::{CapFloor, CapFloorType},
            capletfloorlet::CapletFloorlet,
            europeanswaption::EuropeanSwaption,
            fixfloatcrosscurrencyswap::FixFloatCrossCurrencySwap,
            floatfloatcrosscurrencyswap::FloatFloatCrossCurrencySwap,
            makebasisswap::MakeBasisSwap,
            makecapfloor::MakeCapFloor,
            makeeuropeanswaption::MakeSwaption,
            makefixfloatcrosscurrencyswap::MakeFixFloatCrossCurrencySwap,
            makefloatfloatcrosscurrencyswap::MakeFloatFloatCrossCurrencySwap,
            makeovernightindexswap::MakeOvernightIndexSwap,
            makeratefutures::MakeRateFutures,
            makeswap::MakeSwap,
            oisconventions::{OisConventions, OisPillar, OvernightCouponConventions},
            overnightindexswap::OvernightIndexSwap,
            ratefutures::RateFutures,
            swap::Swap,
        },
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{date::Date, daycounter::DayCounter, enums::Frequency, imm::IMM, period::Period},
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::{Strike, VolatilityType},
};

/// Splits a 6-character FX pair string (e.g. `"EURUSD"`) into two currencies.
fn parse_fx_pair(pair: &str) -> Result<(Currency, Currency)> {
    if pair.len() < 6 {
        return Err(QSError::InvalidValueErr(format!(
            "Invalid FX currency pair: {pair}"
        )));
    }
    let base: Currency = pair[..3].to_ascii_uppercase().parse()?;
    let quote_ccy: Currency = pair[3..6].to_ascii_uppercase().parse()?;
    Ok((base, quote_ccy))
}

fn parse_strike(id: &str, kind: &str, value: &str) -> Result<Strike> {
    let strike_kind = kind.parse::<Strike>()?;
    let strike_value = value
        .parse::<f64>()
        .map_err(|e| QSError::InvalidValueErr(format!("Bad strike in {id}: {e}")))?;
    match strike_kind {
        Strike::Absolute(_) => Ok(Strike::Absolute(strike_value)),
        Strike::Relative(_) => Ok(Strike::Relative(strike_value)),
        Strike::Atm => Err(QSError::InvalidValueErr(format!(
            "ATM strike in {id} must not have a strike value"
        ))),
    }
}

/// Quote level enumeration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    /// Explicitly supplied mid level; no bid/ask average is inferred.
    #[default]
    Mid,
    /// Dealer's buying price.
    Bid,
    /// Dealer's selling price.
    Ask,
}

/// Quote levels associated with an instrument identifier. The requested level
/// must be present; missing levels are not inferred from other prices.
///
/// Units depend on the instrument: rates/spreads are decimals, futures use
/// price points (e.g. `95.25`), and bond prices use their configured quote units
/// (commonly 100). Black volatilities are annualized decimals; normal
/// volatilities use underlying rate/price units per square-root year. FX
/// outrights use quote currency per base currency; FX points are raw
/// forward-minus-spot differences, not pip counts.
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

    /// Resolves a representative quote value for the given [`Level`].
    ///
    /// ## Errors
    /// Returns an error if the requested level is not available.
    pub fn value(&self, level: Level) -> Result<f64> {
        match level {
            Level::Mid => self
                .mid
                .ok_or_else(|| QSError::NotFoundErr("No mid quote available".into())),
            Level::Bid => self
                .bid
                .ok_or_else(|| QSError::NotFoundErr("No bid quote available".into())),
            Level::Ask => self
                .ask
                .ok_or_else(|| QSError::NotFoundErr("No ask quote available".into())),
        }
    }
}

/// Represents the type of instruments that can be handled by the quoting system.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuoteInstrument {
    /// A directly observed discount factor at a curve pillar.
    DiscountFactor,
    /// A directly observed annual zero rate at a curve pillar.
    ZeroRate,
    /// Deposit instrument.
    FixedRateDeposit,
    /// Fixed-rate bond instrument.
    FixedRateBond,
    /// Basis swap instrument.
    BasisSwap,
    /// OIS swap instrument.
    OIS,
    /// Regular fixed-versus-floating interest-rate swap instrument.
    Swap,
    /// Equity call option.
    EquityCall,
    /// Equity put option.
    EquityPut,
    /// FX call option.
    FxCall,
    /// FX put option.
    FxPut,
    /// Cross currency swap instrument (fixed vs floating).
    FixFloatCrossCurrencySwap,
    /// Float-float cross currency swap instrument (both legs floating).
    FloatFloatCrossCurrencySwap,
    /// FX forward instrument, quoted as points or an outright according to its strategy.
    FxForward,
    /// Future instrument.
    Future,
    /// Convexity adjustment.
    ConvexityAdjustment,
    /// Caplet or floorlet instrument.
    CapletFloorlet,
    /// Swaption instrument.
    EuropeanSwaption,
    /// Cap/Floor (requires stripping).
    CapFloor,
    /// Credit default swap (par-spread quote).
    Cds,
}

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

impl std::str::FromStr for OptionStrategy {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "straddle" => Ok(Self::Straddle),
            "strangle" => Ok(Self::Strangle),
            "riskreversal" => Ok(Self::RiskReversal),
            "butterfly" => Ok(Self::Butterfly),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown option strategy: {s}"
            ))),
        }
    }
}

/// Strategies for calibrating a curve from fixed-rate bonds.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BondCalibrationStrategy {
    /// Use the bond's market price as the calibration anchor.
    AnchorPrice,
    /// Use the bond's yield to maturity as the calibration anchor.
    AnchorYield,
}

impl std::str::FromStr for BondCalibrationStrategy {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "anchorprice" => Ok(Self::AnchorPrice),
            "anchoryield" => Ok(Self::AnchorYield),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown fixed-rate bond calibration strategy: {s}"
            ))),
        }
    }
}

/// Strategies for calibrating a curve from FX forwards.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FxForwardCalibrationStrategy {
    /// Use the FX forward's forward points as the calibration anchor.
    AnchorForwardPoints,
    /// Use the FX forward's outright price as the calibration anchor.
    AnchorOutrightPrice,
}

impl std::str::FromStr for FxForwardCalibrationStrategy {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "anchorforwardpoints" => Ok(Self::AnchorForwardPoints),
            "anchoroutrightprice" => Ok(Self::AnchorOutrightPrice),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown FX forward calibration strategy: {s}"
            ))),
        }
    }
}

/// `QuoteDetails` describes the instrument being quoted: its product family, currencies,
/// indices, term and any coupon, strike or payment conventions.
///
/// `QuoteDetails` keeps those contract terms together. The observed bid, mid and
/// ask values are stored in `QuoteLevels`; `Quote` combines the details and values.
/// For example, `FixedRateDeposit_USD_SOFR_6M` describes a six-month USD deposit
/// associated with SOFR. A mid value of `0.04` is its quoted rate of 4%.
///
/// ```rust
/// use quantsupport::prelude::{Quote, QuoteDetails, QuoteLevels};
///
/// let details: QuoteDetails = "FixedRateDeposit_USD_SOFR_6M".parse()?;
/// let quote = Quote::new(details, QuoteLevels::with_mid(0.04));
/// # Ok::<(), quantsupport::prelude::QSError>(())
/// ```
///
/// # Reading an identifier
///
/// Identifiers use underscores to separate contract terms. The first word selects
/// the product, which determines the meaning of the remaining fields. For example,
/// `Swap_USD_TermSOFR3m_5Y_Semiannual_Quarterly` describes a five-year USD swap
/// against three-month Term SOFR, with semiannual fixed payments and quarterly
/// floating payments.
///
/// `CCY` is a currency code such as `USD`; `Index` is a market index such as
/// `SOFR`, `ICP` or `TermSOFR3m`. `Tenor` and `Expiry` are periods such as `1D`,
/// `3M`, `5Y` or `1Y6M`. Frequency names include `Annual`, `Semiannual`, `Quarterly`
/// and `Monthly`. Square brackets mark optional parts of an identifier.
///
/// # Parsed fields
///
/// `QuoteDetails` exposes the terms through getters. Optional fields return `None`
/// when absent or inapplicable to the product. An omitted payment frequency,
/// for example, stays unset in the details; the instrument builder supplies its
/// default when constructing the contract.
///
/// | Getter | Meaning |
/// | --- | --- |
/// | `identifier()` | Original market label used to find the quote and identify its sensitivity. |
/// | `instrument()` | Product family, such as `FixedRateDeposit` or `EuropeanSwaption`. |
/// | `currency()`, `market_index()` | Main currency and index for a rate, bond, credit or equity quote. |
/// | `secondary_market_index()` | Receive index of a basis swap, or foreign index of a floating cross-currency swap. |
/// | `pay_currency()`, `receive_currency()` | Leg currencies of a cross-currency swap, or base and quote currencies of an FX pair. |
/// | `tenor()` | Instrument term; for a swaption, the underlying swap's term. |
/// | `option_expiry()` | Time to option expiry. |
/// | `index_tenor()` | Underlying accrual period of a caplet/floorlet. |
/// | `contract_code()` | IMM contract code of a future or convexity-adjustment quote. |
/// | `pay_leg_frequency()`, `receive_leg_frequency()` | Payment frequencies supplied in the identifier. |
/// | `coupon_rate()`, `strike()`, `vol_type()` | Bond coupon, option strike and volatility quoting convention, where applicable. |
/// | `bond_calibration_strategy()` | Whether the bond quote supplies a yield or a price. |
/// | `fx_forward_calibration_strategy()` | Whether the FX forward quote supplies forward points or an outright rate. |
/// | `option_strategy()` | Caplet/floorlet quote strategy, such as `Straddle`. |
///
/// Tenors are relative periods. Builders use the market reference date and the
/// instrument's conventions to resolve actual start, expiry and payment dates.
/// For `Swaption_USD_SOFR_1Y_5Y_Absolute_0.04_Black`, `option_expiry()` is one year,
/// `tenor()` is five years, and `strike()` is 4%. The associated `QuoteLevels`
/// supplies the observed implied volatility.
///
/// # Rates, bonds and credit
///
/// | Identifier | What it describes |
/// | --- | --- |
/// | `FixedRateDeposit_CCY_Index_Tenor` | A deposit's currency, curve index and term. |
/// | `DiscountFactor_CCY_Index_Pillar` | A discount factor at a tenor or ISO date. |
/// | `ZeroRate_CCY_Index_Pillar[_Compounding[_Frequency]]` | An annual decimal zero rate at a tenor or ISO date; defaults to Continuous and Annual. |
/// | `OIS_CCY_Index_Tenor[_OisFields]` | A fixed rate against an overnight index; optional conventions are described under [OIS parameters](#ois-parameters). |
/// | `Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq]]` | A fixed rate against a floating index, with optional fixed and floating payment frequencies. |
/// | `FixedRateBond_CCY_Index_Tenor[_PayFreq]_CouponRate[_BondStrategy]` | A bond's currency, curve index, term, coupon and optional payment frequency and calibration strategy. |
/// | `Future_CCY_Index_IMM` | An interest-rate futures contract identified by its currency, index and IMM code. |
/// | `ConvexityAdjustment_CCY_Index_IMM` | An adjustment quote associated with the specified futures contract. |
/// | `Cds_Entity_CCY_Tenor` | A CDS par spread for a credit entity, currency and term. |
///
/// Deposit and swap quote values are rates, while CDS quote values are par spreads.
/// Rates, coupons and spreads use decimals: `0.04` means 4%.
/// Direct curve quotes are referenced by `CurveConfiguration.quotes`, using its
/// day counter, interpolation and extrapolation settings. Each direct curve
/// uses one quote family and consistent rate conventions. Its sensitivity
/// labels remain the original quote identifiers.
///
/// In `FixedRateBond_USD_CORP_5Y_Semiannual_0.04_AnchorPrice`, the `0.04` in the
/// identifier is the coupon. `AnchorPrice` tells calibration to use the observed
/// quote as the bond price; `AnchorYield` uses it as yield to maturity and is the
/// default if the strategy is omitted. `IMM` is a futures contract code such as `H6`.
///
/// # Basis and cross-currency swaps
///
/// | Identifier | What it describes |
/// | --- | --- |
/// | `BasisSwap_CCY_PayIndex_RecvIndex_Tenor[_PayFreq[_RecvFreq]]` | Two floating legs in one currency; the primary index is the pay index and the secondary index is the receive index. |
/// | `FixFloatCrossCurrencySwap_DomCCY_FloatIndex_ForCCY_Tenor[_DomFreq[_ForFreq]]` | A fixed domestic-currency leg against a floating foreign-currency leg. |
/// | `FloatFloatCrossCurrencySwap_DomCCY_DomIndex_ForIndex_ForCCY_Tenor[_DomFreq[_ForFreq]]` | A domestic floating leg against a foreign floating leg, each with its own index. |
///
/// `DomCCY` and `ForCCY` identify the domestic and foreign legs; `DomFreq` and
/// `ForFreq` set their payment frequencies. For example,
/// `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_5Y` describes five years of USD
/// SOFR cashflows against CLP ICP cashflows.
///
/// # FX and equity
///
/// | Identifier | What it describes |
/// | --- | --- |
/// | `FxForward_PAIR_Tenor_ForwardStrategy` | A forward's currency pair, delivery term and choice of forward-points or outright-price quote. |
/// | `EquityCall_CCY_Ticker_Expiry_StrikeKind_StrikeValue` | A call on the specified equity, with its currency, expiry and strike. |
/// | `EquityPut_CCY_Ticker_Expiry_StrikeKind_StrikeValue` | A put on the specified equity, with its currency, expiry and strike. |
/// | `FxCall_PAIR_Expiry_StrikeKind_StrikeValue` | A currency call with the specified pair, expiry and strike. |
/// | `FxPut_PAIR_Expiry_StrikeKind_StrikeValue` | A currency put with the specified pair, expiry and strike. |
///
/// `PAIR` contains the base currency followed by the quote currency: `EURUSD`
/// means USD per EUR. For an FX forward, `AnchorForwardPoints` uses forward points
/// as the observed quote and `AnchorOutrightPrice` uses the outright forward rate.
/// For example, `FxForward_EURUSD_1M_AnchorOutrightPrice` identifies a one-month
/// EUR/USD forward whose quote is the outright rate.
///
/// Equity and FX option identifiers accept `Absolute` and `Relative` strike kinds.
/// `EquityCall_USD_AAPL_6M_Absolute_150` identifies a six-month USD call on AAPL
/// with a strike of 150. A relative strike is an offset from the forward.
///
/// # Interest-rate options
///
/// | Identifier | What it describes |
/// | --- | --- |
/// | `CapFloor_CCY_Index_Tenor[_Freq]_Strike_VolType` | A cap/floor volatility quote with the full instrument term and optional coupon frequency. |
/// | `CapletFloorlet_CCY_Index_IndexTenor_Expiry_Strike_Strategy_VolType` | A caplet/floorlet volatility quote; index tenor is the underlying accrual period and expiry is the time until it starts. |
/// | `Swaption_CCY_Index_Expiry_SwapTenor[_FixedFreq[_FloatFreq]]_Strike_VolType` | A swaption volatility quote; expiry is the exercise term and swap tenor is the length of the underlying swap. |
///
/// `Strike` expands to `Absolute_0.04`, `Relative_0.01` or `ATM`. `Absolute` specifies
/// the strike itself; `Relative` adds an offset to the forward; `ATM` uses the
/// forward and needs no numeric strike. For example,
/// `Swaption_USD_SOFR_1Y_5Y_ATM_Black` quotes the volatility of an at-the-money
/// option expiring in one year on a five-year SOFR swap.
///
/// `VolType` is `Black` for lognormal volatility or `Normal` for absolute-rate
/// volatility. Volatilities use decimals: a Black value of
/// `0.20` means 20%. The caplet/floorlet `Strategy` field accepts `Straddle`,
/// `Strangle`, `RiskReversal` or `Butterfly`, as defined by `OptionStrategy`.
///
/// # OIS parameters
///
/// OIS quotes can specify settlement, payment schedules, calendars and overnight
/// observation rules after the tenor. `OIS_USD_SOFR_1Y` uses the index preset.
/// To override a later field while keeping earlier defaults, leave the earlier
/// fields empty: `OIS_USD_SOFR_1Y___0_3` keeps both default frequencies, starts
/// with zero settlement days and pays three business days after accrual ends.
/// Omit any trailing fields you do not need.
///
/// | Order | Field | Accepted values / examples | Default |
/// | --- | --- | --- | --- |
/// | 1 | Fixed frequency | `Frequency`, e.g. `Annual`, `Semiannual` | `Annual` |
/// | 2 | Overnight frequency | `Frequency`, e.g. `Annual`, `Quarterly` | `Annual` |
/// | 3 | Settlement days | Nonnegative business-day count | SONIA: `0`; CORRA: `1`; other indices: `2` |
/// | 4 | Payment lag | Nonnegative business-day count | `SOFR` / `SOFRCompounded`: `2`; other indices: `0` |
/// | 5 | Fixed day counter | `DayCounter`, e.g. `Actual360`, `Actual365` | Index day counter |
/// | 6 | Settlement calendar | Canonical calendar name | Index calendar |
/// | 7 | Fixed accrual calendar | Canonical calendar name | Index calendar |
/// | 8 | Overnight accrual calendar | Canonical calendar name | Index calendar |
/// | 9 | Payment calendar | Canonical calendar name | Index calendar |
/// | 10 | Accrual adjustment | Accrual business-day convention, e.g. `Following` | `ModifiedFollowing` |
/// | 11 | Termination adjustment | Termination business-day convention | Accrual convention |
/// | 12 | Payment adjustment | Payment business-day convention | `Following` |
/// | 13 | Schedule generation | e.g. `Backward`, `Forward` | `Backward` |
/// | 14 | End of month | `true` or `false` | Inferred from the effective date |
/// | 15 | Forward start | `Period`, e.g. `1M` | `0D` (start at spot) |
/// | 16 | First coupon date | ISO date, e.g. `2027-01-05` | No explicit front stub |
/// | 17 | Penultimate coupon date | ISO date, e.g. `2030-01-05` | No explicit back stub |
/// | 18 | Overnight averaging | `Compound` or `Simple` | `Compound` |
/// | 19 | Lookback days | Nonnegative business-day observation count | `0` |
/// | 20 | Lockout days | Nonnegative business-day observation count | `0` |
/// | 21 | Observation shift | `true` or `false` | `false` |
/// | 22 | Daily spread compounding | `true` or `false` | `false` |
/// | 23 | Calibration pillar | `LastRelevantDate`, `MaturityDate`, `CustomDate(YYYY-MM-DD)` | `LastRelevantDate` |
///
/// Calendar values use names such as `UnitedStates(Sofr)` or `TARGET`.
/// `OIS_USD_SOFR_2Y_Annual_Annual_2_2_Actual360` specifies annual payments,
/// two settlement days, a two-business-day payment lag and ACT/360 fixed coupons.
/// `CustomDate(2027-01-06)` selects an explicit calibration pillar; that date must
/// lie between the contract's effective and last relevant dates.
///
/// # Constructing details
///
/// Parse a standard identifier with `str::parse()` or `QuoteDetails::from_str()`.
/// `QuoteDetails::parse(identifier, separator)` accepts a different separator.
/// For a structured data feed, `QuoteDetails::new` and the `with_*` setters let
/// you supply the terms directly.
///
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteDetails {
    identifier: String,
    instrument: QuoteInstrument,
    #[serde(default)]
    market_index: Option<MarketIndex>,
    #[serde(default)]
    option_strategy: Option<OptionStrategy>,
    #[serde(default)]
    bond_calibration_strategy: Option<BondCalibrationStrategy>,
    #[serde(default)]
    fx_forward_calibration_strategy: Option<FxForwardCalibrationStrategy>,
    #[serde(default)]
    vol_type: Option<VolatilityType>,
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
    strike: Option<Strike>,
    #[serde(default)]
    maturity: Option<Date>,
    #[serde(default)]
    tenor: Option<Period>,
    #[serde(default)]
    vol_shift: Option<f64>,
    /// Primary instrument currency.
    #[serde(default)]
    currency: Option<Currency>,
    /// Secondary market index (e.g. receive-leg index on a basis swap).
    #[serde(default)]
    secondary_market_index: Option<MarketIndex>,
    /// Option expiry tenor (swaptions, caplets).
    #[serde(default)]
    option_expiry: Option<Period>,
    /// Futures / convexity-adjustment IMM contract code (e.g. "H6").
    #[serde(default)]
    contract_code: Option<String>,
    /// Underlying index tenor (caplet/floorlet frequency).
    #[serde(default)]
    index_tenor: Option<Period>,
    /// Pay (or fixed / domestic) leg frequency.
    #[serde(default)]
    pay_leg_frequency: Option<Frequency>,
    /// Receive (or floating / foreign) leg frequency.
    #[serde(default)]
    receive_leg_frequency: Option<Frequency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compounding: Option<Compounding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frequency: Option<Frequency>,
}

impl QuoteDetails {
    /// Creates a new quote details container with required fields.
    #[must_use]
    pub const fn new(identifier: String, instrument: QuoteInstrument) -> Self {
        Self {
            identifier,
            instrument,
            market_index: None,
            option_strategy: None,
            bond_calibration_strategy: None,
            fx_forward_calibration_strategy: None,
            vol_type: None,
            rate: None,
            price: None,
            coupon_rate: None,
            pay_currency: None,
            receive_currency: None,
            strike: None,
            maturity: None,
            tenor: None,
            vol_shift: None,
            currency: None,
            secondary_market_index: None,
            option_expiry: None,
            contract_code: None,
            index_tenor: None,
            pay_leg_frequency: None,
            receive_leg_frequency: None,
            compounding: None,
            frequency: None,
        }
    }

    // -----------------------------------------------------------------------
    // Getters
    // -----------------------------------------------------------------------

    /// Returns the quote identifier.
    #[must_use]
    pub fn identifier(&self) -> String {
        self.identifier.clone()
    }

    /// Returns the primary market index.
    #[must_use]
    pub const fn market_index(&self) -> Option<&MarketIndex> {
        self.market_index.as_ref()
    }

    /// Returns the instrument type.
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }

    /// Returns the option strategy, if present.
    #[must_use]
    pub const fn option_strategy(&self) -> Option<OptionStrategy> {
        self.option_strategy
    }

    /// Returns the fixed-rate bond calibration strategy, if specified.
    #[must_use]
    pub const fn bond_calibration_strategy(&self) -> Option<BondCalibrationStrategy> {
        self.bond_calibration_strategy
    }

    /// Returns the FX forward calibration strategy, if specified.
    #[must_use]
    pub const fn fx_forward_calibration_strategy(&self) -> Option<FxForwardCalibrationStrategy> {
        self.fx_forward_calibration_strategy
    }

    /// Returns the volatility type, if present.
    #[must_use]
    pub const fn vol_type(&self) -> Option<&VolatilityType> {
        self.vol_type.as_ref()
    }

    /// Returns the rate, if present.
    #[must_use]
    pub const fn rate(&self) -> Option<f64> {
        self.rate
    }

    /// Returns the price, if present.
    #[must_use]
    pub const fn price(&self) -> Option<f64> {
        self.price
    }

    /// Returns the coupon rate, if present.
    #[must_use]
    pub const fn coupon_rate(&self) -> Option<f64> {
        self.coupon_rate
    }

    /// Returns the pay / base currency, if present.
    #[must_use]
    pub const fn pay_currency(&self) -> Option<Currency> {
        self.pay_currency
    }

    /// Returns the receive / quote currency, if present.
    #[must_use]
    pub const fn receive_currency(&self) -> Option<Currency> {
        self.receive_currency
    }

    /// Returns the strike, if present.
    #[must_use]
    pub const fn strike(&self) -> Option<Strike> {
        self.strike
    }

    /// Returns the vol shift, if present.
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

    /// Returns the compounding convention of a direct zero-rate quote.
    #[must_use]
    pub const fn compounding(&self) -> Option<Compounding> {
        self.compounding
    }

    /// Returns the compounding frequency of a direct zero-rate quote.
    #[must_use]
    pub const fn frequency(&self) -> Option<Frequency> {
        self.frequency
    }

    /// Returns the primary instrument currency, if present.
    #[must_use]
    pub const fn currency(&self) -> Option<Currency> {
        self.currency
    }

    /// Returns the secondary market index (e.g. receive-leg index on a basis swap).
    #[must_use]
    pub const fn secondary_market_index(&self) -> Option<&MarketIndex> {
        self.secondary_market_index.as_ref()
    }

    /// Returns the option expiry tenor, if present.
    #[must_use]
    pub const fn option_expiry(&self) -> Option<Period> {
        self.option_expiry
    }

    /// Returns the futures contract code (IMM code), if present.
    #[must_use]
    pub fn contract_code(&self) -> Option<&str> {
        self.contract_code.as_deref()
    }

    /// Returns the underlying index tenor, if present.
    #[must_use]
    pub const fn index_tenor(&self) -> Option<Period> {
        self.index_tenor
    }

    /// Returns the pay (or fixed / domestic) leg frequency, if present.
    #[must_use]
    pub const fn pay_leg_frequency(&self) -> Option<Frequency> {
        self.pay_leg_frequency
    }

    /// Returns the receive (or floating / foreign) leg frequency, if present.
    #[must_use]
    pub const fn receive_leg_frequency(&self) -> Option<Frequency> {
        self.receive_leg_frequency
    }

    // -----------------------------------------------------------------------
    // Builder setters
    // -----------------------------------------------------------------------

    /// Sets the option strategy.
    #[must_use]
    pub const fn with_option_strategy(mut self, s: OptionStrategy) -> Self {
        self.option_strategy = Some(s);
        self
    }
    /// Sets the fixed-rate bond calibration strategy.
    #[must_use]
    pub const fn with_bond_calibration_strategy(mut self, s: BondCalibrationStrategy) -> Self {
        self.bond_calibration_strategy = Some(s);
        self
    }
    /// Sets the FX forward calibration strategy.
    #[must_use]
    pub const fn with_fx_forward_calibration_strategy(
        mut self,
        s: FxForwardCalibrationStrategy,
    ) -> Self {
        self.fx_forward_calibration_strategy = Some(s);
        self
    }
    /// Sets the volatility type.
    #[must_use]
    pub const fn with_vol_type(mut self, v: VolatilityType) -> Self {
        self.vol_type = Some(v);
        self
    }
    /// Sets the rate.
    #[must_use]
    pub const fn with_rate(mut self, r: f64) -> Self {
        self.rate = Some(r);
        self
    }
    /// Sets the price.
    #[must_use]
    pub const fn with_price(mut self, p: f64) -> Self {
        self.price = Some(p);
        self
    }
    /// Sets the coupon rate.
    #[must_use]
    pub const fn with_coupon_rate(mut self, r: f64) -> Self {
        self.coupon_rate = Some(r);
        self
    }
    /// Sets the pay / base currency.
    #[must_use]
    pub const fn with_pay_currency(mut self, c: Currency) -> Self {
        self.pay_currency = Some(c);
        self
    }
    /// Sets the receive / quote currency.
    #[must_use]
    pub const fn with_receive_currency(mut self, c: Currency) -> Self {
        self.receive_currency = Some(c);
        self
    }
    /// Sets the strike.
    #[must_use]
    pub const fn with_strike(mut self, s: Strike) -> Self {
        self.strike = Some(s);
        self
    }

    /// Sets the maturity.
    #[must_use]
    pub const fn with_maturity(mut self, d: Date) -> Self {
        self.maturity = Some(d);
        self
    }
    /// Sets the tenor.
    #[must_use]
    pub const fn with_tenor(mut self, p: Period) -> Self {
        self.tenor = Some(p);
        self
    }

    /// Sets the compounding convention of a direct zero-rate quote.
    #[must_use]
    pub const fn with_compounding(mut self, compounding: Compounding) -> Self {
        self.compounding = Some(compounding);
        self
    }

    /// Sets the compounding frequency of a direct zero-rate quote.
    #[must_use]
    pub const fn with_frequency(mut self, frequency: Frequency) -> Self {
        self.frequency = Some(frequency);
        self
    }
    /// Sets the vol shift.
    #[must_use]
    pub const fn with_vol_shift(mut self, s: f64) -> Self {
        self.vol_shift = Some(s);
        self
    }
    /// Sets the primary instrument currency.
    #[must_use]
    pub const fn with_currency(mut self, c: Currency) -> Self {
        self.currency = Some(c);
        self
    }
    /// Sets the secondary market index.
    #[must_use]
    pub fn with_secondary_market_index(mut self, idx: MarketIndex) -> Self {
        self.secondary_market_index = Some(idx);
        self
    }
    /// Sets the option expiry tenor.
    #[must_use]
    pub const fn with_option_expiry(mut self, p: Period) -> Self {
        self.option_expiry = Some(p);
        self
    }
    /// Sets the futures contract code.
    #[must_use]
    pub fn with_contract_code(mut self, code: String) -> Self {
        self.contract_code = Some(code);
        self
    }
    /// Sets the underlying index tenor.
    #[must_use]
    pub const fn with_index_tenor(mut self, p: Period) -> Self {
        self.index_tenor = Some(p);
        self
    }

    /// Sets the pay (or fixed / domestic) leg frequency.
    #[must_use]
    pub const fn with_pay_leg_frequency(mut self, f: Frequency) -> Self {
        self.pay_leg_frequency = Some(f);
        self
    }

    /// Sets the receive (or floating / foreign) leg frequency.
    #[must_use]
    pub const fn with_receive_leg_frequency(mut self, f: Frequency) -> Self {
        self.receive_leg_frequency = Some(f);
        self
    }

    /// Sets the primary market index.
    #[must_use]
    pub fn with_market_index(mut self, idx: MarketIndex) -> Self {
        self.market_index = Some(idx);
        self
    }

    // -----------------------------------------------------------------------
    // Identifier parsing helpers
    // -----------------------------------------------------------------------

    /// Tries to parse one or two optional [`Frequency`] values starting at
    /// `parts[start]`.
    ///
    /// Returns `(pay_freq, recv_freq, next_index)` where `next_index` is the
    /// position of the first part that was *not* consumed as a frequency.
    fn try_parse_frequencies(
        parts: &[&str],
        start: usize,
    ) -> (Option<Frequency>, Option<Frequency>, usize) {
        let pay: Option<Frequency> = parts.get(start).and_then(|s| s.parse().ok());
        pay.map_or((None, None, start), |p| {
            let recv: Option<Frequency> = parts.get(start + 1).and_then(|s| s.parse().ok());
            recv.map_or_else(
                || (Some(p), None, start + 1),
                |r| (Some(p), Some(r), start + 2),
            )
        })
    }

    /// Parses `OIS_CCY_Index_Tenor` followed by optional positional contract fields.
    /// See the [complete OIS parameter reference](QuoteDetails#ois-parameters).
    /// For example, `OIS_USD_SOFR_1Y___0_3` keeps the default frequencies,
    /// with zero settlement days and a three-business-day payment lag.
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_ois(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "OIS identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;

        let pay_freq = parts
            .get(4)
            .filter(|value| !value.is_empty())
            .map(|value| value.parse::<Frequency>())
            .transpose()?;
        let recv_freq = parts
            .get(5)
            .filter(|value| !value.is_empty())
            .map(|value| value.parse::<Frequency>())
            .transpose()?;

        let mut det = Self::new(id.to_string(), QuoteInstrument::OIS)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor);
        if let Some(f) = pay_freq {
            det = det.with_pay_leg_frequency(f);
        }
        if let Some(f) = recv_freq {
            det = det.with_receive_leg_frequency(f);
        }
        det.ois_conventions()?;
        Ok(det)
    }

    /// Resolves OIS contract parameters exclusively from the quote identifier.
    /// Omitted parameters retain the index preset.
    ///
    /// # Errors
    /// Rejects excess, malformed, or invalid positional contract parameters.
    pub fn ois_conventions(&self) -> Result<OisConventions> {
        if self.instrument != QuoteInstrument::OIS {
            return Err(QSError::InvalidValueErr("Not an OIS quote".into()));
        }
        let index = self
            .market_index
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("OIS index".into()))?;
        let separator = self
            .identifier
            .get(3..)
            .and_then(|suffix| suffix.chars().next())
            .unwrap_or('_');
        let parts: Vec<_> = self.identifier.split(separator).collect();
        OisConventions::for_index(index).with_quote_fields(parts.get(4..).unwrap_or_default())
    }

    /// `Swap_CCY_Index_Tenor[_FixedFreq[_FloatingFreq]]`
    ///
    /// # Errors
    /// Rejects missing required fields, invalid frequencies and excess fields.
    pub fn parse_swap(id: &str, parts: &[&str]) -> Result<Self> {
        if !(4..=6).contains(&parts.len()) {
            return Err(QSError::InvalidValueErr(format!(
                "Swap identifier requires four to six fields: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index: MarketIndex = parts[2].parse()?;
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;
        let fixed_frequency = parts
            .get(4)
            .filter(|value| !value.is_empty())
            .map(|value| value.parse::<Frequency>())
            .transpose()?;
        let floating_frequency = parts
            .get(5)
            .filter(|value| !value.is_empty())
            .map(|value| value.parse::<Frequency>())
            .transpose()?;
        let mut details = Self::new(id.to_string(), QuoteInstrument::Swap)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor);
        if let Some(frequency) = fixed_frequency {
            details = details.with_pay_leg_frequency(frequency);
        }
        if let Some(frequency) = floating_frequency {
            details = details.with_receive_leg_frequency(frequency);
        }
        Ok(details)
    }

    /// `{Instrument}_CCY_{Index}_{Tenor}`
    ///
    /// Fixed-rate deposit quotes are rates; no alternative calibration
    /// strategies are supported.
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fixed_rate_deposit(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() != 4 {
            return Err(QSError::InvalidValueErr(format!(
                "FixedRateDeposit identifier must contain exactly four fields: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;
        Ok(Self::new(id.to_string(), QuoteInstrument::FixedRateDeposit)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor))
    }

    /// `{Instrument}_{CCY}_{Index}_{Tenor}[_{PayFreq}]_{CouponRate}[_{Strategy}]`
    ///
    /// The strategy defaults to `AnchorYield`, for example
    /// `FixedRateBond_USD_UST_5Y_Semiannual_0.04_AnchorYield`.
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fixed_rate_bond(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "FixedRateBond identifier too short: {id}"
            )));
        }
        let currency = parts
            .get(1)
            .ok_or_else(|| {
                QSError::InvalidValueErr(format!("FixedRateBond identifier too short: {id}"))
            })?
            .to_ascii_uppercase()
            .parse::<Currency>()?;
        let index = parts
            .get(2)
            .ok_or_else(|| {
                QSError::InvalidValueErr(format!("FixedRateBond identifier too short: {id}"))
            })?
            .parse::<MarketIndex>()?;
        let tenor = Period::from_str(
            &parts
                .get(3)
                .ok_or_else(|| {
                    QSError::InvalidValueErr(format!("FixedRateBond identifier too short: {id}"))
                })?
                .to_ascii_uppercase(),
        )?;
        let mut next = 4;
        let freq = parts.get(next).and_then(|s| s.parse::<Frequency>().ok());
        if freq.is_some() {
            next += 1;
        }
        let coupon_rate = parts.get(next).and_then(|s| s.parse::<f64>().ok());
        if coupon_rate.is_some() {
            next += 1;
        }
        let strategy = parts
            .get(next)
            .map(|s| s.parse::<BondCalibrationStrategy>())
            .transpose()?;
        if parts.len() > next + usize::from(strategy.is_some()) {
            return Err(QSError::InvalidValueErr(format!(
                "Unexpected fields in fixed-rate bond identifier: {id}"
            )));
        }
        let mut quote = Self::new(id.to_string(), QuoteInstrument::FixedRateBond)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor);

        if let Some(f) = freq {
            quote = quote.with_pay_leg_frequency(f);
        }
        if let Some(c) = coupon_rate {
            quote = quote.with_coupon_rate(c);
        }
        if let Some(strategy) = strategy {
            quote = quote.with_bond_calibration_strategy(strategy);
        }
        Ok(quote)
    }

    /// `{Instrument}_{Entity}_{CCY}_{Tenor}` — e.g. `Cds_CORPORATE-A_USD_5Y`
    ///
    /// The quote level is the CDS par spread (decimal, e.g. `0.0125`).
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_cds(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "Cds identifier too short: {id}"
            )));
        }
        let entity = parts[1].to_string();
        let currency: Currency = parts[2].to_ascii_uppercase().parse()?;
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;
        Ok(Self::new(id.to_string(), QuoteInstrument::Cds)
            .with_market_index(MarketIndex::Credit(entity))
            .with_currency(currency)
            .with_tenor(tenor))
    }

    /// `{Instrument}_CCY_{PayIndex}_{RecvIndex}_{Tenor}[_{PayFreq}_{RecvFreq}]`
    /// e.g. `BasisSwap_USD_SOFR_TermSOFR3m_1Y` or
    /// `BasisSwap_USD_SOFR_TermSOFR3m_1Y_Quarterly_Quarterly`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_basis_swap(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 5 {
            return Err(QSError::InvalidValueErr(format!(
                "BasisSwap identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let pay_index = parts[2].parse::<MarketIndex>()?;
        let recv_index = parts[3].parse::<MarketIndex>()?;
        let tenor = Period::from_str(&parts[4].to_ascii_uppercase())?;

        let (pay_freq, recv_freq, _) = Self::try_parse_frequencies(parts, 5);

        let mut det = Self::new(id.to_string(), QuoteInstrument::BasisSwap)
            .with_market_index(pay_index)
            .with_currency(currency)
            .with_secondary_market_index(recv_index)
            .with_tenor(tenor);
        if let Some(f) = pay_freq {
            det = det.with_pay_leg_frequency(f);
        }
        if let Some(f) = recv_freq {
            det = det.with_receive_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_DomesticCCY_{FloatingIndex}_{ForeignCCY}_{Tenor}[_{DomFreq}_{ForFreq}]`
    /// e.g. `FixFloatCrossCurrencySwap_USD_ICP_CLP_1Y` or
    /// `FixFloatCrossCurrencySwap_USD_ICP_CLP_1Y_Semiannual_Quarterly`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fix_float_cross_currency_swap(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 5 {
            return Err(QSError::InvalidValueErr(format!(
                "FixFloatCrossCurrencySwap identifier too short: {id}"
            )));
        }
        let domestic_currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let floating_index = parts[2].parse::<MarketIndex>()?;
        let foreign_currency: Currency = parts[3].to_ascii_uppercase().parse()?;
        let tenor = Period::from_str(&parts[4].to_ascii_uppercase())?;

        let (dom_freq, for_freq, _) = Self::try_parse_frequencies(parts, 5);

        let mut det = Self::new(id.to_string(), QuoteInstrument::FixFloatCrossCurrencySwap)
            .with_market_index(floating_index)
            .with_currency(domestic_currency)
            .with_pay_currency(domestic_currency)
            .with_receive_currency(foreign_currency)
            .with_tenor(tenor);
        if let Some(f) = dom_freq {
            det = det.with_pay_leg_frequency(f);
        }
        if let Some(f) = for_freq {
            det = det.with_receive_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_{DomCCY}_{DomIndex}_{ForIndex}_{ForCCY}_{Tenor}[_{DomFreq}_{ForFreq}]`
    /// e.g. `FloatFloatCrossCurrencySwap_CLP_ICP_SOFR_USD_1Y` or
    /// `FloatFloatCrossCurrencySwap_CLP_ICP_SOFR_USD_1Y_Quarterly_Quarterly`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_float_float_cross_currency_swap(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 6 {
            return Err(QSError::InvalidValueErr(format!(
                "FloatFloatCrossCurrencySwap identifier too short: {id}"
            )));
        }
        let domestic_currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let dom_index = parts[2].parse::<MarketIndex>()?;
        let for_index = parts[3].parse::<MarketIndex>()?;
        let foreign_currency: Currency = parts[4].to_ascii_uppercase().parse()?;
        let tenor = Period::from_str(&parts[5].to_ascii_uppercase())?;

        let (dom_freq, for_freq, _) = Self::try_parse_frequencies(parts, 6);

        let mut det = Self::new(id.to_string(), QuoteInstrument::FloatFloatCrossCurrencySwap)
            .with_market_index(dom_index)
            .with_currency(domestic_currency)
            .with_pay_currency(domestic_currency)
            .with_receive_currency(foreign_currency)
            .with_secondary_market_index(for_index)
            .with_tenor(tenor);
        if let Some(f) = dom_freq {
            det = det.with_pay_leg_frequency(f);
        }
        if let Some(f) = for_freq {
            det = det.with_receive_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_CCY_{Index}_{Tenor}[_{Freq}]_{Strike}_{VolType}` (without strike value)
    ///
    /// `{Instrument}_CCY_{Index}_{Tenor}[_{Freq}]_{Strike}_{StrikeValue}_{VolType}` (with strike value)
    ///
    /// e.g. `CapFloor_USD_SOFR_1Y_Absolute_Black` or
    /// `CapFloor_USD_SOFR_1Y_Quarterly_Absolute_Black`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_cap_floor(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 6 {
            return Err(QSError::InvalidValueErr(format!(
                "CapFloor identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;

        // Try optional frequency after tenor
        let (freq, next) = parts
            .get(4)
            .and_then(|s| s.parse::<Frequency>().ok())
            .map_or((None, 4), |f| (Some(f), 5));

        let strike_base = parts[next].parse::<Strike>()?;

        // Try parsing next+1 as f64 (strike value). If it succeeds, the vol
        // type follows at next+2; otherwise next+1 is the vol type.
        let strike_idx = next + 1;
        let (strike, vol_idx) = parts
            .get(strike_idx)
            .and_then(|s| s.parse::<f64>().ok())
            .map_or((strike_base, strike_idx), |s| {
                let st = match strike_base {
                    Strike::Absolute(_) => Strike::Absolute(s),
                    Strike::Relative(_) => Strike::Relative(s),
                    Strike::Atm => Strike::Atm,
                };
                (st, strike_idx + 1)
            });
        let vol_type: VolatilityType = parts
            .get(vol_idx)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing vol type in: {id}")))?
            .parse()?;

        let mut det = Self::new(id.to_string(), QuoteInstrument::CapFloor)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor)
            .with_strike(strike)
            .with_vol_type(vol_type);
        if let Some(f) = freq {
            det = det.with_pay_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_CCY_{Index}_{IdxTenor}_{Expiry}_{Strike}_{StrikeValue}_{Strategy}_{VolType}`
    ///
    /// Or without explicit strike value: `.._{Strike}_{Strategy}_{VolType}`.
    ///
    /// e.g. `CapletFloorlet_USD_TermSOFR3m_3M_3M_Absolute_0.010_Straddle_Black`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_caplet_floorlet(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 8 {
            return Err(QSError::InvalidValueErr(format!(
                "CapletFloorlet identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let index_tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;
        let option_expiry = Period::from_str(&parts[4].to_ascii_uppercase())?;
        let strike_base = parts[5].parse::<Strike>()?;

        let (strike, next_idx) = parts[6].parse::<f64>().map_or((strike_base, 6), |s| {
            let st = match strike_base {
                Strike::Absolute(_) => Strike::Absolute(s),
                Strike::Relative(_) => Strike::Relative(s),
                Strike::Atm => Strike::Atm,
            };
            (st, 7)
        });

        let strategy: OptionStrategy = parts
            .get(next_idx)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing strategy in: {id}")))?
            .parse()?;
        let vol_type: VolatilityType = parts
            .get(next_idx + 1)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing vol type in: {id}")))?
            .parse()?;

        let det = Self::new(id.to_string(), QuoteInstrument::CapletFloorlet)
            .with_market_index(index)
            .with_currency(currency)
            .with_index_tenor(index_tenor)
            .with_option_expiry(option_expiry)
            .with_strike(strike)
            .with_option_strategy(strategy)
            .with_vol_type(vol_type);
        Ok(det)
    }

    /// `{Instrument}_CCY_{Index}_{IMMCode}` — e.g. `Future_USD_SOFR_H6`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_future(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "Future identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let code = parts[3].to_ascii_uppercase();
        Ok(Self::new(id.to_string(), QuoteInstrument::Future)
            .with_market_index(index)
            .with_currency(currency)
            .with_contract_code(code))
    }

    /// `{Instrument}_CCY_{Index}_{IMMCode}` — e.g. `ConvexityAdjustment_USD_SOFR_H6`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_convexity_adjustment(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "ConvexityAdjustment identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let code = parts[3].to_ascii_uppercase();
        Ok(
            Self::new(id.to_string(), QuoteInstrument::ConvexityAdjustment)
                .with_market_index(index)
                .with_currency(currency)
                .with_contract_code(code),
        )
    }

    /// Swaption identifier parser.
    ///
    /// `{Instrument}_CCY_{Index}_{Expiry}_{SwapTenor}[_{PayFreq}_{RecvFreq}]_{Strike}_{VolType}` (no strike value)
    /// `{Instrument}_CCY_{Index}_{Expiry}_{SwapTenor}[_{PayFreq}_{RecvFreq}]_{Strike}_{StrikeValue}_{VolType}` (with strike value)
    /// e.g. `Swaption_USD_SOFR_3M_2Y_Absolute_Black` or
    /// `Swaption_USD_SOFR_3M_2Y_Semiannual_Semiannual_Absolute_Black`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_swaption(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 7 {
            return Err(QSError::InvalidValueErr(format!(
                "Swaption identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let option_expiry = Period::from_str(&parts[3].to_ascii_uppercase())?;
        let swap_tenor = Period::from_str(&parts[4].to_ascii_uppercase())?;

        let (pay_freq, recv_freq, next) = Self::try_parse_frequencies(parts, 5);

        let strike_base = parts[next].parse::<Strike>()?;

        let strike_idx = next + 1;
        let (strike, vol_idx) = parts
            .get(strike_idx)
            .and_then(|s| s.parse::<f64>().ok())
            .map_or((strike_base, strike_idx), |s| {
                let st = match strike_base {
                    Strike::Absolute(_) => Strike::Absolute(s),
                    Strike::Relative(_) => Strike::Relative(s),
                    Strike::Atm => Strike::Atm,
                };
                (st, strike_idx + 1)
            });
        let vol_type: VolatilityType = parts
            .get(vol_idx)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing vol type in: {id}")))?
            .parse()?;

        let mut det = Self::new(id.to_string(), QuoteInstrument::EuropeanSwaption)
            .with_market_index(index)
            .with_currency(currency)
            .with_option_expiry(option_expiry)
            .with_tenor(swap_tenor)
            .with_strike(strike)
            .with_vol_type(vol_type);
        if let Some(f) = pay_freq {
            det = det.with_pay_leg_frequency(f);
        }
        if let Some(f) = recv_freq {
            det = det.with_receive_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_{CCYPAIR}_{Tenor}_{Strategy}` — e.g.
    /// `FxForward_EURUSD_1M_AnchorOutrightPrice`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fx_forward(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() != 4 {
            return Err(QSError::InvalidValueErr(format!(
                "FxForward identifier must contain a calibration strategy: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(&parts[2].to_ascii_uppercase())?;
        let strategy = parts[3].parse::<FxForwardCalibrationStrategy>()?;
        Ok(Self::new(id.to_string(), QuoteInstrument::FxForward)
            .with_pay_currency(base)
            .with_receive_currency(quote_ccy)
            .with_tenor(tenor)
            .with_fx_forward_calibration_strategy(strategy))
    }

    /// `{Instrument}_CCY_{Index}_{Expiry}_{StrikeKind}_{Strike}` — e.g. `EquityCall_USD_SPX_1Y_Absolute_5000`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_equity_call(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 6 {
            return Err(QSError::InvalidValueErr(format!(
                "Call identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = MarketIndex::Equity(parts[2].to_string());
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;
        let strike = parse_strike(id, parts[4], parts[5])?;
        Ok(Self::new(id.to_string(), QuoteInstrument::EquityCall)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor)
            .with_option_expiry(tenor)
            .with_strike(strike))
    }

    /// `{Instrument}_CCY_{Index}_{Expiry}_{StrikeKind}_{Strike}` — e.g. `EquityPut_USD_SPX_1Y_Absolute_5000`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_equity_put(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 6 {
            return Err(QSError::InvalidValueErr(format!(
                "Put identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].to_ascii_uppercase().parse()?;
        let index = MarketIndex::Equity(parts[2].to_string());
        let tenor = Period::from_str(&parts[3].to_ascii_uppercase())?;
        let strike = parse_strike(id, parts[4], parts[5])?;
        Ok(Self::new(id.to_string(), QuoteInstrument::EquityPut)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor)
            .with_option_expiry(tenor)
            .with_strike(strike))
    }

    /// `{Instrument}_{CCYPAIR}_{Expiry}_{StrikeKind}_{Strike}` — e.g. `FxCall_EURUSD_1Y_Absolute_1.10`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fx_call(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 5 {
            return Err(QSError::InvalidValueErr(format!(
                "FxCall identifier too short: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(&parts[2].to_ascii_uppercase())?;
        let strike = parse_strike(id, parts[3], parts[4])?;

        Ok(Self::new(id.to_string(), QuoteInstrument::FxCall)
            .with_market_index(MarketIndex::FxPair(FxPair::new(base, quote_ccy)?))
            .with_pay_currency(base)
            .with_receive_currency(quote_ccy)
            .with_tenor(tenor)
            .with_option_expiry(tenor)
            .with_strike(strike))
    }

    /// `{Instrument}_{CCYPAIR}_{Expiry}_{StrikeKind}_{Strike}` — e.g. `FxPut_EURUSD_1Y_Absolute_1.10`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fx_put(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 5 {
            return Err(QSError::InvalidValueErr(format!(
                "FxPut identifier too short: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(&parts[2].to_ascii_uppercase())?;
        let strike = parse_strike(id, parts[3], parts[4])?;

        Ok(Self::new(id.to_string(), QuoteInstrument::FxPut)
            .with_market_index(MarketIndex::FxPair(FxPair::new(base, quote_ccy)?))
            .with_pay_currency(base)
            .with_receive_currency(quote_ccy)
            .with_tenor(tenor)
            .with_option_expiry(tenor)
            .with_strike(strike))
    }

    fn parse_curve_pillar(id: &str, parts: &[&str]) -> Result<Self> {
        let zero_rate = parts[0].eq_ignore_ascii_case("ZeroRate");
        let max_parts = if zero_rate { 6 } else { 4 };
        if !(4..=max_parts).contains(&parts.len()) {
            return Err(QSError::InvalidValueErr(format!(
                "Invalid direct curve quote identifier: {id}"
            )));
        }
        let instrument = if zero_rate {
            QuoteInstrument::ZeroRate
        } else {
            QuoteInstrument::DiscountFactor
        };
        let mut details = Self::new(id.to_string(), instrument)
            .with_currency(parts[1].to_ascii_uppercase().parse()?)
            .with_market_index(parts[2].parse()?);
        details = if let Ok(date) = Date::from_str(parts[3], "%Y-%m-%d") {
            details.with_maturity(date)
        } else {
            details.with_tenor(Period::from_str(&parts[3].to_ascii_uppercase())?)
        };
        if zero_rate {
            details = details
                .with_compounding(parts.get(4).map_or(Ok(Compounding::Continuous), |value| {
                    Compounding::try_from((*value).to_string())
                })?)
                .with_frequency(
                    parts
                        .get(5)
                        .map_or(Ok(Frequency::Annual), |value| value.parse())?,
                );
        }
        Ok(details)
    }

    /// Parses a quote identifier using a custom separator.
    ///
    /// ## Errors
    /// Returns an error if the identifier cannot be parsed with the given separator.
    pub fn parse(s: &str, separator: char) -> Result<Self> {
        let parts: Vec<&str> = s.split(separator).collect();
        if parts.len() < 3 {
            return Err(QSError::InvalidValueErr(format!(
                "Identifier has fewer than 3 parts: {s}"
            )));
        }
        match parts[0].to_ascii_lowercase().as_str() {
            "discountfactor" | "zerorate" => Self::parse_curve_pillar(s, &parts),
            "ois" => Self::parse_ois(s, &parts),
            "swap" => Self::parse_swap(s, &parts),
            "fixedratedeposit" => Self::parse_fixed_rate_deposit(s, &parts),
            "fixedratebond" => Self::parse_fixed_rate_bond(s, &parts),
            "basisswap" => Self::parse_basis_swap(s, &parts),
            "fixfloatcrosscurrencyswap" => Self::parse_fix_float_cross_currency_swap(s, &parts),
            "capfloor" => Self::parse_cap_floor(s, &parts),
            "capletfloorlet" => Self::parse_caplet_floorlet(s, &parts),
            "future" => Self::parse_future(s, &parts),
            "convexityadjustment" => Self::parse_convexity_adjustment(s, &parts),
            "swaption" => Self::parse_swaption(s, &parts),
            "floatfloatcrosscurrencyswap" => Self::parse_float_float_cross_currency_swap(s, &parts),
            "fxforward" => Self::parse_fx_forward(s, &parts),
            "equitycall" => Self::parse_equity_call(s, &parts),
            "equityput" => Self::parse_equity_put(s, &parts),
            "fxcall" => Self::parse_fx_call(s, &parts),
            "fxput" => Self::parse_fx_put(s, &parts),
            "cds" => Self::parse_cds(s, &parts),
            other => Err(QSError::InvalidValueErr(format!(
                "Unknown instrument type in identifier: {other}"
            ))),
        }
    }
}

impl std::str::FromStr for QuoteDetails {
    type Err = QSError;

    /// Parses a quote identifier string (underscore-separated) into [`QuoteDetails`].
    ///
    /// The first `_`-delimited segment determines the instrument type (e.g.
    /// `FxForward`). Instrument names and typed fields are case insensitive.
    /// The original identifier and free-form entity names are preserved.
    ///
    /// # Errors
    /// Returns an error if the identifier cannot be parsed.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s, '_')
    }
}

/// Wraps every concrete instrument type that can be produced from a [`Quote`].
#[derive(Clone)]
pub enum CalibrationInstrumentType<T = f64>
where
    T: Scalar,
{
    /// A vanilla fixed-rate deposit.
    FixedRateDeposit(FixedRateDeposit<T>),
    /// A fixed-rate bond.
    FixedRateBond(FixedRateBond<T>, BondCalibrationStrategy),
    /// An overnight indexed swap with daily overnight coupon semantics.
    OIS(OvernightIndexSwap<T>),
    /// A regular fixed-vs-floating interest rate swap.
    Swap(Swap<T>),
    /// A floating-vs-floating basis swap.
    BasisSwap(BasisSwap<T>),
    /// A rate futures contract.
    RateFutures(RateFutures),
    /// An FX forward quoted as points or an outright.
    FxForward(FxForward, FxForwardCalibrationStrategy),
    /// A cross-currency swap (fixed domestic vs floating foreign).
    FixFloatCrossCurrencySwap(FixFloatCrossCurrencySwap<T>),
    /// A float-float cross-currency swap (both legs floating).
    FloatFloatCrossCurrencySwap(FloatFloatCrossCurrencySwap<T>),
    /// A European equity call option.
    EquityCall(EquityEuropeanOption),
    /// A European equity put option.
    EquityPut(EquityEuropeanOption),
    /// A European FX call option.
    FxCall(FxEuropeanOption),
    /// A European FX put option.
    FxPut(FxEuropeanOption),
    /// An interest rate cap or floor.
    CapFloor(CapFloor),
    /// A single caplet or floorlet.
    CapletFloorlet(CapletFloorlet),
    /// A European swaption (option on a swap).
    EuropeanSwaption(EuropeanSwaption<T>),
}

impl<T: Scalar> std::fmt::Debug for CalibrationInstrumentType<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixedRateDeposit(_) => {
                write!(f, "CalibrationInstrumentType::FixedRateDeposit")
            }
            Self::OIS(_) => write!(f, "CalibrationInstrumentType::OIS"),
            Self::Swap(_) => write!(f, "CalibrationInstrumentType::Swap"),
            Self::BasisSwap(_) => write!(f, "CalibrationInstrumentType::BasisSwap"),
            Self::RateFutures(_) => write!(f, "CalibrationInstrumentType::RateFutures"),
            Self::FxForward(_, _) => write!(f, "CalibrationInstrumentType::FxForward"),
            Self::FixFloatCrossCurrencySwap(_) => {
                write!(f, "CalibrationInstrumentType::FixFloatCrossCurrencySwap")
            }
            Self::FloatFloatCrossCurrencySwap(_) => {
                write!(f, "CalibrationInstrumentType::FloatFloatCrossCurrencySwap")
            }
            Self::FixedRateBond(_, _) => write!(f, "CalibrationInstrumentType::FixedRateBond"),
            Self::EquityCall(_) => write!(f, "CalibrationInstrumentType::EquityCall"),
            Self::EquityPut(_) => write!(f, "CalibrationInstrumentType::EquityPut"),
            Self::FxCall(_) => write!(f, "CalibrationInstrumentType::FxCall"),
            Self::FxPut(_) => write!(f, "CalibrationInstrumentType::FxPut"),
            Self::CapFloor(_) => write!(f, "CalibrationInstrumentType::CapFloor"),
            Self::CapletFloorlet(_) => write!(f, "CalibrationInstrumentType::CapletFloorlet"),
            Self::EuropeanSwaption(_) => write!(f, "CalibrationInstrumentType::EuropeanSwaption"),
        }
    }
}

impl<T> CalibrationInstrumentType<T>
where
    T: Scalar,
{
    /// Returns the final date that defines the calibration pillar for the instrument.
    ///
    /// # Errors
    /// Returns an error if the instrument type is not supported or if underlying instrument data is invalid.
    pub fn pillar_date(&self) -> Result<Date> {
        match self {
            Self::FixedRateDeposit(x) => Ok(x.leg().last_payment_date()),
            Self::FixedRateBond(x, _) => Ok(x.leg().last_payment_date()),
            Self::OIS(x) => Ok(x
                .fixed_leg()
                .last_payment_date()
                .max(x.floating_leg().last_payment_date())),
            Self::Swap(x) => Ok(x
                .fixed_leg()
                .last_payment_date()
                .max(x.floating_leg().last_payment_date())),
            Self::BasisSwap(x) => Ok(x
                .pay_leg()
                .last_payment_date()
                .max(x.receive_leg().last_payment_date())),
            Self::FixFloatCrossCurrencySwap(x) => Ok(x
                .domestic_leg()
                .last_payment_date()
                .max(x.foreign_leg().last_payment_date())),
            Self::FloatFloatCrossCurrencySwap(x) => Ok(x
                .domestic_leg()
                .last_payment_date()
                .max(x.foreign_leg().last_payment_date())),
            Self::RateFutures(x) => Ok(x.end_date()),
            Self::FxForward(x, _) => Ok(x.delivery_date()),
            Self::EquityCall(x) | Self::EquityPut(x) => Ok(x.expiry_date()),
            Self::FxCall(x) | Self::FxPut(x) => Ok(x.expiry_date()),
            Self::CapletFloorlet(x) => Ok(x.fixing_date()),
            Self::CapFloor(x) => x
                .last_fixing_date()
                .ok_or_else(|| QSError::ValueNotSetErr("cap/floor has no caplet/floorlets".into())),
            Self::EuropeanSwaption(x) => Ok(x.expiry_date()),
        }
    }
}

/// Contains the quote information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quote {
    details: QuoteDetails,
    levels: QuoteLevels,
}

impl Quote {
    /// Creates a new quote.
    #[must_use]
    pub const fn new(details: QuoteDetails, levels: QuoteLevels) -> Self {
        Self { details, levels }
    }

    /// Returns the quote details.
    #[must_use]
    pub const fn details(&self) -> &QuoteDetails {
        &self.details
    }

    /// Returns the quote levels.
    #[must_use]
    pub const fn levels(&self) -> &QuoteLevels {
        &self.levels
    }

    /// Builds a concrete financial instrument from this quote.
    ///
    /// # Arguments
    /// * `reference_date` – the as-of / valuation date. Tenors are rolled
    ///   from this date to determine maturity / delivery.
    /// * `level` – which price level to extract (`Mid`, `Bid`, `Ask`).
    /// * `fx_spot` – optional FX spot rate for cross-currency instruments.
    ///   When provided, the domestic notional is set to
    ///   `fx_spot × foreign_notional` so that notional exchanges are
    ///   balanced at inception.
    ///
    /// # Errors
    /// Returns an error when:
    /// * the quote level is unavailable,
    /// * required detail fields are missing,
    /// * the instrument type is not directly buildable (e.g. vol-only quotes), or
    /// * the underlying maker returns an error.
    pub fn build_instrument(
        &self,
        reference_date: Date,
        level: Level,
        fx_spot: Option<f64>,
    ) -> Result<CalibrationInstrumentType<f64>> {
        let value = self.levels.value(level)?;
        let notional = 1.0;
        match self.details.instrument() {
            QuoteInstrument::OIS => self.build_ois(value, reference_date, notional),
            QuoteInstrument::Swap => self.build_swap(value, reference_date, notional),
            QuoteInstrument::FixedRateDeposit => {
                self.build_fixed_rate_deposit(value, reference_date, notional)
            }
            QuoteInstrument::FixedRateBond => self.build_fixed_rate_bond(reference_date, notional),
            QuoteInstrument::BasisSwap => self.build_basis_swap(value, reference_date, notional),
            QuoteInstrument::Future => self.build_rate_futures(value, reference_date),
            QuoteInstrument::FxForward => self.build_fx_forward(value, reference_date),
            QuoteInstrument::FixFloatCrossCurrencySwap => {
                let domestic_notional = fx_spot.map_or(notional, |fx| notional * fx);
                self.build_fix_float_cross_currency_swap(
                    value,
                    reference_date,
                    domestic_notional,
                    notional,
                )
            }
            QuoteInstrument::FloatFloatCrossCurrencySwap => {
                let domestic_notional = fx_spot.map_or(notional, |fx| notional * fx);
                self.build_float_float_cross_currency_swap(
                    value,
                    reference_date,
                    domestic_notional,
                    notional,
                )
            }
            QuoteInstrument::EquityCall => self.build_equity_call(reference_date),
            QuoteInstrument::EquityPut => self.build_equity_put(reference_date),
            QuoteInstrument::CapFloor => self.build_cap_floor(value, reference_date, notional),
            QuoteInstrument::CapletFloorlet => self.build_caplet_floorlet(reference_date),
            QuoteInstrument::EuropeanSwaption => {
                self.build_swaption(value, reference_date, notional)
            }
            QuoteInstrument::FxCall => self.build_fx_call(reference_date),
            QuoteInstrument::FxPut => self.build_fx_put(reference_date),
            QuoteInstrument::ConvexityAdjustment => Err(QSError::NotImplementedErr(format!(
                "Cannot build instrument for {:?} — it is a vol / auxiliary quote type",
                QuoteInstrument::ConvexityAdjustment
            ))),
            QuoteInstrument::Cds => Err(QSError::NotImplementedErr(
                "CDS quotes are consumed by the credit curve bootstrapper, not the \
                 calibration-instrument builder"
                    .into(),
            )),
            QuoteInstrument::DiscountFactor | QuoteInstrument::ZeroRate => {
                Err(QSError::InvalidValueErr(
                    "Direct curve quotes are consumed by CurveConfiguration, not instrument builders"
                        .into(),
                ))
            }
        }
    }

    /// Resolves an OIS calibration node according to its quote identifier.
    ///
    /// # Errors
    /// Rejects custom nodes outside the instrument's relevant date range.
    pub fn calibration_pillar_date(&self, built: &CalibrationInstrumentType<f64>) -> Result<Date> {
        use crate::instruments::cashflows::cashflowtype::CashflowType;
        use crate::time::{
            calendars::traits::IsCalendar, enums::BusinessDayConvention, enums::TimeUnit,
            period::Period,
        };
        let mut latest = built.pillar_date()?;
        let CalibrationInstrumentType::OIS(swap) = built else {
            return Ok(latest);
        };
        if *self.details.instrument() != QuoteInstrument::OIS {
            return Ok(latest);
        }
        let mut earliest = latest;
        let mut maturity = Date::empty();
        for cashflow in swap
            .fixed_leg()
            .cashflows()
            .iter()
            .chain(swap.floating_leg().cashflows())
        {
            match cashflow {
                CashflowType::FixedRateCoupon(coupon) => {
                    earliest = earliest.min(coupon.accrual_start_date());
                    maturity = maturity.max(coupon.accrual_end_date());
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    earliest = earliest.min(coupon.accrual_start_date());
                    maturity = maturity.max(coupon.accrual_end_date());
                    let calendar = coupon.market_index().rate_index_details()?.calendar();
                    let value_end = calendar.adjust(
                        coupon.accrual_end_date(),
                        Some(BusinessDayConvention::Following),
                    );
                    let lookback = coupon
                        .overnight_conventions()
                        .map_or(0, OvernightCouponConventions::lookback_days);
                    let observation_end = calendar.advance(
                        value_end,
                        Period::new(-lookback, TimeUnit::Days),
                        None,
                        false,
                    );
                    latest = latest.max(observation_end).max(coupon.payment_date());
                }
                _ => {}
            }
        }
        let choice = self.details.ois_conventions()?.pillar();
        match choice {
            OisPillar::LastRelevantDate => Ok(latest),
            OisPillar::MaturityDate => Ok(maturity),
            OisPillar::CustomDate(date) if date >= earliest && date <= latest => Ok(date),
            OisPillar::CustomDate(_) => Err(QSError::InvalidValueErr(
                "Custom OIS pillar must lie between effective and last relevant dates".into(),
            )),
        }
    }

    /// Convenience method for getting the quote index.
    fn required_market_index(details: &QuoteDetails, context: &str) -> Result<MarketIndex> {
        details
            .market_index()
            .cloned()
            .ok_or_else(|| QSError::ValueNotSetErr(format!("Market index on {context}")))
    }

    /// OIS swap - mid value is the fixed rate.
    fn build_ois<T: Scalar + Default>(
        &self,
        rate: f64,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let currency = d
            .currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Currency on OIS quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on OIS quote".into()))?;

        let market_index = Self::required_market_index(d, "OIS quote")?;
        if market_index.rate_index_details()?.currency() != currency {
            return Err(QSError::InvalidValueErr(
                "OIS currency does not match its index".into(),
            ));
        }
        let conventions = d.ois_conventions()?;
        let mut builder =
            MakeOvernightIndexSwap::<T>::new(market_index, reference_date, tenor, rate)
                .with_identifier(d.identifier())
                .with_notional(notional)
                .with_conventions(conventions);
        if let Some(maturity) = d.maturity() {
            builder = builder.with_maturity_date(maturity);
        }
        let swap = builder.build()?;

        Ok(CalibrationInstrumentType::OIS(swap))
    }

    /// Builds a regular swap using single-fixing floating coupons and swap defaults.
    fn build_swap<T: Scalar + Default>(
        &self,
        rate: f64,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let details = &self.details;
        let currency = details
            .currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Currency on swap quote".into()))?;
        let tenor = details
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on swap quote".into()))?;
        let index = Self::required_market_index(details, "swap quote")?;
        let index_details = index.rate_index_details()?;
        if index_details.currency() != currency {
            return Err(QSError::InvalidValueErr(
                "Swap currency does not match its index".into(),
            ));
        }
        let mut builder = MakeSwap::<T>::default()
            .with_identifier(details.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(details.maturity().unwrap_or(reference_date + tenor))
            .with_fixed_rate(rate)
            .with_notional(notional)
            .with_rate_definition(index_details.rate_definition())
            .with_market_index(index)
            .with_currency(currency);
        if let Some(frequency) = details.pay_leg_frequency() {
            builder = builder.with_fixed_leg_frequency(frequency);
        }
        if let Some(frequency) = details.receive_leg_frequency() {
            builder = builder.with_floating_leg_frequency(frequency);
        }
        Ok(CalibrationInstrumentType::Swap(builder.build()?))
    }

    /// Builds a fixed-rate deposit from its rate quote.
    fn build_fixed_rate_deposit<T: Scalar + Default>(
        &self,
        quote_value: f64,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let currency = d
            .currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Currency on deposit quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on deposit quote".into()))?;

        let maturity = reference_date + tenor;
        let market_index = Self::required_market_index(d, "deposit quote")?;
        let rd = market_index.rate_index_details()?.rate_definition();

        let deposit = MakeFixedRateDeposit::<T>::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_rate(quote_value)
            .with_notional(notional)
            .with_rate_definition(rd)
            .with_currency(currency)
            .with_discount_index(Some(market_index))
            .build()?;

        Ok(CalibrationInstrumentType::FixedRateDeposit(deposit))
    }

    /// Builds a fixed-rate bond using the configured calibration strategy.
    /// The default strategy is yield anchoring.
    fn build_fixed_rate_bond<T: Scalar + Default>(
        &self,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let currency = d
            .currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Currency on fixed rate bond quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on fixed rate bond quote".into()))?;

        let maturity = reference_date + tenor;
        let market_index = Self::required_market_index(d, "fixed rate bond quote")?;
        let coupon_rate = d.coupon_rate().ok_or_else(|| {
            QSError::ValueNotSetErr("Coupon rate on fixed-rate bond quote".into())
        })?;
        let frequency = d.pay_leg_frequency().unwrap_or(Frequency::Semiannual);
        let rate_definition =
            RateDefinition::new(DayCounter::ActualActual, Compounding::Compounded, frequency);
        let strategy = d
            .bond_calibration_strategy()
            .unwrap_or(BondCalibrationStrategy::AnchorYield);

        let bond = MakeFixedRateBond::<T>::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_rate(coupon_rate)
            .with_notional(notional)
            .with_rate_definition(rate_definition)
            .with_currency(currency)
            .with_discount_index(market_index)
            .with_payment_frequency(frequency)
            .build()?;

        Ok(CalibrationInstrumentType::FixedRateBond(bond, strategy))
    }

    /// Basis Swap — mid value is the spread applied to the receive leg.
    fn build_basis_swap<T: Scalar + Default>(
        &self,
        spread: f64,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let currency = d
            .currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Currency on basis swap quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on basis swap quote".into()))?;
        let recv_index = d
            .secondary_market_index()
            .ok_or_else(|| {
                QSError::ValueNotSetErr("Secondary market index on basis swap quote".into())
            })?
            .clone();
        let pay_index = Self::required_market_index(d, "basis swap quote")?;

        let maturity = reference_date + tenor;

        let mut builder = MakeBasisSwap::<T>::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_notional(notional)
            .with_currency(currency)
            .with_pay_market_index(pay_index)
            .with_receive_market_index(recv_index)
            .with_pay_spread(spread);
        if let Some(f) = d.pay_leg_frequency() {
            builder = builder.with_pay_leg_frequency(f);
        }
        if let Some(f) = d.receive_leg_frequency() {
            builder = builder.with_receive_leg_frequency(f);
        }
        let basis_swap = builder.build()?;

        Ok(CalibrationInstrumentType::BasisSwap(basis_swap))
    }

    /// Rate Futures — mid value is the futures price, dates resolved from IMM code.
    fn build_rate_futures<T: Scalar + Default>(
        &self,
        price: f64,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let code = d
            .contract_code()
            .ok_or_else(|| QSError::ValueNotSetErr("Contract code on futures quote".into()))?;

        let start_date = IMM::date(code, reference_date);
        let end_date = IMM::next_date(start_date, true);
        let market_index = Self::required_market_index(d, "futures quote")?;
        let rd = market_index.rate_index_details()?.rate_definition();

        let futures = MakeRateFutures::default()
            .with_identifier(d.identifier())
            .with_market_index(market_index)
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_futures_price(price)
            .with_rate_definition(rd)
            .build()?;

        Ok(CalibrationInstrumentType::RateFutures(futures))
    }

    /// Builds an FX forward according to the quote's calibration strategy.
    fn build_fx_forward<T: Scalar + Default>(
        &self,
        quote_value: f64,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let base = d
            .pay_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Base currency on FX forward quote".into()))?;
        let quote_ccy = d
            .receive_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Quote currency on FX forward quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on FX forward quote".into()))?;

        let delivery_date = reference_date + tenor;

        let strategy = d.fx_forward_calibration_strategy().ok_or_else(|| {
            QSError::ValueNotSetErr("Calibration strategy on FX forward quote".into())
        })?;
        let mut builder = MakeFxForward::default()
            .with_identifier(d.identifier())
            .with_delivery_date(delivery_date)
            .with_base_currency(base)
            .with_quote_currency(quote_ccy);
        builder = match strategy {
            FxForwardCalibrationStrategy::AnchorForwardPoints => {
                builder.with_forward_points(quote_value)
            }
            FxForwardCalibrationStrategy::AnchorOutrightPrice => {
                builder.with_forward_rate(quote_value)
            }
        };
        Ok(CalibrationInstrumentType::FxForward(
            builder.build()?,
            strategy,
        ))
    }

    /// Cross-Currency Swap (fixed domestic vs floating foreign).
    /// Mid value is the fixed rate on the domestic leg.
    fn build_fix_float_cross_currency_swap<T: Scalar + Default>(
        &self,
        fixed_rate: f64,
        reference_date: Date,
        domestic_notional: f64,
        foreign_notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let domestic_ccy = d.pay_currency().ok_or_else(|| {
            QSError::ValueNotSetErr("Domestic currency on xccy swap quote".into())
        })?;
        let foreign_ccy = d
            .receive_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Foreign currency on xccy swap quote".into()))?;
        let floating_index = d
            .market_index()
            .ok_or_else(|| {
                QSError::ValueNotSetErr("Foreign market index on xccy swap quote".into())
            })?
            .clone();
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on xccy swap quote".into()))?;

        let maturity = reference_date + tenor;
        let rd = floating_index.rate_index_details()?.rate_definition();

        let mut builder = MakeFixFloatCrossCurrencySwap::<T>::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_domestic_notional(domestic_notional)
            .with_foreign_notional(foreign_notional)
            .with_fixed_rate(fixed_rate)
            .with_rate_definition(rd)
            .with_domestic_currency(domestic_ccy)
            .with_foreign_currency(foreign_ccy)
            .with_floating_index(floating_index);
        if let Some(f) = d.pay_leg_frequency() {
            builder = builder.with_domestic_leg_frequency(f);
        }
        if let Some(f) = d.receive_leg_frequency() {
            builder = builder.with_foreign_leg_frequency(f);
        }
        let xccy = builder.build()?;

        Ok(CalibrationInstrumentType::FixFloatCrossCurrencySwap(xccy))
    }

    /// Float-float cross-currency swap — mid value is the spread on the
    /// domestic floating leg.
    fn build_float_float_cross_currency_swap<T: Scalar + Default>(
        &self,
        domestic_spread: f64,
        reference_date: Date,
        domestic_notional: f64,
        foreign_notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let domestic_ccy = d.pay_currency().ok_or_else(|| {
            QSError::ValueNotSetErr("Domestic currency on ff-xccy swap quote".into())
        })?;
        let foreign_ccy = d.receive_currency().ok_or_else(|| {
            QSError::ValueNotSetErr("Foreign currency on ff-xccy swap quote".into())
        })?;
        let foreign_index = d
            .secondary_market_index()
            .ok_or_else(|| {
                QSError::ValueNotSetErr("Foreign market index on ff-xccy swap quote".into())
            })?
            .clone();
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on ff-xccy swap quote".into()))?;

        let maturity = reference_date + tenor;
        let domestic_index = Self::required_market_index(d, "ff-xccy swap quote")?;

        let mut builder = MakeFloatFloatCrossCurrencySwap::<T>::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_domestic_notional(domestic_notional)
            .with_foreign_notional(foreign_notional)
            .with_domestic_spread(domestic_spread)
            .with_domestic_currency(domestic_ccy)
            .with_foreign_currency(foreign_ccy)
            .with_domestic_market_index(domestic_index)
            .with_foreign_market_index(foreign_index);
        if let Some(f) = d.pay_leg_frequency() {
            builder = builder.with_domestic_leg_frequency(f);
        }
        if let Some(f) = d.receive_leg_frequency() {
            builder = builder.with_foreign_leg_frequency(f);
        }
        let xccy = builder.build()?;

        Ok(CalibrationInstrumentType::FloatFloatCrossCurrencySwap(xccy))
    }

    /// European Fx Call — strike and expiry from details.
    fn build_fx_call<T: Scalar + Default>(
        &self,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        self.build_fx_option(reference_date, EuroOptionType::Call)
    }

    /// European Fx Call — strike and expiry from details.
    fn build_fx_put<T: Scalar + Default>(
        &self,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        self.build_fx_option(reference_date, EuroOptionType::Put)
    }

    fn build_fx_option<T: Scalar + Default>(
        &self,
        reference_date: Date,
        option_type: EuroOptionType,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let strike = d
            .strike()
            .ok_or_else(|| QSError::ValueNotSetErr("Strike on FX option quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on FX option quote".into()))?;
        let base_currency = d
            .pay_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Base currency on FX option quote".into()))?;
        let quote_currency = d
            .receive_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Quote currency on FX option quote".into()))?;
        let pair = crate::indices::fxpair::FxPair::new(base_currency, quote_currency)?;
        let option = MakeFxEuropeanOption::default()
            .with_identifier(d.identifier())
            .with_expiry_date(reference_date + tenor)
            .with_strike_spec(strike)
            .with_option_type(option_type)
            .with_base_currency(base_currency)
            .with_quote_currency(quote_currency)
            .with_pair(pair)
            .build()?;

        Ok(match option_type {
            EuroOptionType::Call => CalibrationInstrumentType::FxCall(option),
            EuroOptionType::Put => CalibrationInstrumentType::FxPut(option),
        })
    }

    /// European equity Call — strike and expiry from details.
    fn build_equity_call<T: Scalar + Default>(
        &self,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        self.build_equity_option(reference_date, EuroOptionType::Call)
    }

    /// European equity Put — strike and expiry from details.
    fn build_equity_put<T: Scalar + Default>(
        &self,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        self.build_equity_option(reference_date, EuroOptionType::Put)
    }

    fn build_equity_option<T: Scalar + Default>(
        &self,
        reference_date: Date,
        option_type: EuroOptionType,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let strike = d
            .strike()
            .ok_or_else(|| QSError::ValueNotSetErr("Strike on equity option quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on equity option quote".into()))?;
        let currency = d
            .currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Currency on equity option quote".into()))?;
        let option = MakeEquityEuropeanOption::default()
            .with_identifier(d.identifier())
            .with_market_index(Self::required_market_index(d, "equity option quote")?)
            .with_expiry_date(reference_date + tenor)
            .with_strike_spec(strike)
            .with_option_type(option_type)
            .with_currency(currency)
            .build()?;

        Ok(match option_type {
            EuroOptionType::Call => CalibrationInstrumentType::EquityCall(option),
            EuroOptionType::Put => CalibrationInstrumentType::EquityPut(option),
        })
    }

    /// Builds a single `CapletFloorlet` from a vol quote.
    ///
    /// The quote value is the market vol; the strike, expiry, and index tenor
    /// are extracted from the parsed `QuoteDetails`.
    fn build_caplet_floorlet(&self, reference_date: Date) -> Result<CalibrationInstrumentType> {
        use crate::instruments::rates::capletfloorlet::{
            CapletFloorlet as CFL, CapletFloorletType,
        };

        let d = &self.details;
        let option_expiry = d
            .option_expiry()
            .ok_or_else(|| QSError::ValueNotSetErr("Option expiry on CapletFloorlet".into()))?;
        let index_tenor = d
            .index_tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Index tenor on CapletFloorlet".into()))?;
        let market_index = Self::required_market_index(d, "CapletFloorlet quote")?;

        let start = reference_date + option_expiry;
        let end = start + index_tenor;

        let strike = d.strike().unwrap_or(Strike::Atm);

        let cfl = CFL::new(
            d.identifier(),
            market_index,
            d.currency()
                .ok_or_else(|| QSError::ValueNotSetErr("Currency on CapletFloorlet".into()))?,
            start,
            start,
            end,
            end, // payment_date = end_date
            CapletFloorletType::Caplet,
            strike,
        );

        Ok(CalibrationInstrumentType::CapletFloorlet(cfl))
    }

    /// Interest rate Cap or Floor — strike and tenor from details.
    /// The quote value is used as the strike when the details don't carry one.
    fn build_cap_floor<T: Scalar + Default>(
        &self,
        value: f64,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on CapFloor quote".into()))?;
        let maturity = reference_date + tenor;
        let market_index = Self::required_market_index(d, "CapFloor quote")?;
        let rd = market_index.rate_index_details()?.rate_definition();

        // The quote value is treated as the strike. If the details already
        // carry a parsed strike, prefer that.
        let strike = d.strike().unwrap_or(Strike::Absolute(value)).resolve(0.0);

        // Default to Cap for the quote-driven builder.
        let cap_floor_type = CapFloorType::Cap;

        let mut builder = MakeCapFloor::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_strike(strike)
            .with_notional(notional)
            .with_rate_definition(rd)
            .with_market_index(market_index)
            .with_currency(
                d.currency()
                    .ok_or_else(|| QSError::ValueNotSetErr("Currency on CapFloor quote".into()))?,
            )
            .with_cap_floor_type(cap_floor_type);
        if let Some(f) = d.pay_leg_frequency() {
            builder = builder.with_frequency(f);
        }
        let cf = builder.build()?;

        Ok(CalibrationInstrumentType::CapFloor(cf))
    }

    /// European swaption — builds the underlying swap and wraps it.
    /// The quote value is used as the strike (fixed rate) when the details
    /// don't carry one.
    fn build_swaption<T: Scalar + Default>(
        &self,
        value: f64,
        reference_date: Date,
        notional: f64,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let option_expiry_period = d
            .option_expiry()
            .ok_or_else(|| QSError::ValueNotSetErr("Option expiry on Swaption quote".into()))?;
        let swap_tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Swap tenor on Swaption quote".into()))?;

        let expiry_date = reference_date + option_expiry_period;
        let swap_maturity = expiry_date + swap_tenor;
        let market_index = Self::required_market_index(d, "Swaption quote")?;
        let rd = market_index.rate_index_details()?.rate_definition();

        let strike = d.strike().unwrap_or(Strike::Absolute(value)).resolve(0.0);

        let mut builder = MakeSwaption::<T>::default()
            .with_identifier(d.identifier())
            .with_expiry(expiry_date)
            .with_start_date(expiry_date)
            .with_swap_tenor_date(swap_maturity)
            .with_strike(strike)
            .with_notional(notional)
            .with_rate_definition(rd)
            .with_market_index(market_index)
            .with_currency(
                d.currency()
                    .ok_or_else(|| QSError::ValueNotSetErr("Currency on Swaption quote".into()))?,
            );
        if let Some(f) = d.pay_leg_frequency() {
            builder = builder.with_fixed_leg_frequency(f);
        }
        if let Some(f) = d.receive_leg_frequency() {
            builder = builder.with_floating_leg_frequency(f);
        }
        let swaption = builder.build()?;

        Ok(CalibrationInstrumentType::EuropeanSwaption(swaption))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::enums::TimeUnit;

    fn ref_date() -> Date {
        Date::new(2026, 2, 24)
    }

    // -- FromStr round-trips ------------------------------------------------

    #[test]
    fn parse_ois_identifier() {
        let det: QuoteDetails = "OIS_USD_SOFR_1Y".parse().unwrap();
        assert_eq!(det.identifier(), "OIS_USD_SOFR_1Y");
        assert_eq!(*det.instrument(), QuoteInstrument::OIS);
        assert_eq!(det.currency(), Some(Currency::USD));
    }

    #[test]
    fn parse_deposit_identifier() {
        let det: QuoteDetails = "FixedRateDeposit_USD_SOFR_6M".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FixedRateDeposit);
    }

    #[test]
    fn deposit_rejects_non_rate_quote_fields() {
        assert!("FixedRateDeposit_USD_SOFR_6M_AnchorPrice"
            .parse::<QuoteDetails>()
            .is_err());
    }

    #[test]
    fn parse_price_anchored_bond_identifier() {
        let det: QuoteDetails = "FixedRateBond_USD_CORP_5Y_Semiannual_0.04_AnchorPrice"
            .parse()
            .unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FixedRateBond);
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.coupon_rate(), Some(0.04));
        assert_eq!(
            det.bond_calibration_strategy(),
            Some(BondCalibrationStrategy::AnchorPrice)
        );
    }

    #[test]
    fn calibration_strategy_names_must_use_anchor_prefix() {
        assert!("FixedRateBond_USD_CORP_5Y_Semiannual_0.04_Price"
            .parse::<QuoteDetails>()
            .is_err());
        assert!("FixedRateBond_USD_CORP_5Y_Semiannual_0.04_AnchorOAS"
            .parse::<QuoteDetails>()
            .is_err());
        assert!("FxForward_EURUSD_1M_ForwardPoints"
            .parse::<QuoteDetails>()
            .is_err());
    }

    #[test]
    fn legacy_fx_forward_instrument_names_are_rejected() {
        assert!("FxForwardPoints_EURUSD_1M".parse::<QuoteDetails>().is_err());
        assert!("FxOutrightForward_EURUSD_1M"
            .parse::<QuoteDetails>()
            .is_err());
    }

    #[test]
    fn parse_basis_swap_identifier() {
        let det: QuoteDetails = "BasisSwap_USD_SOFR_TermSOFR3m_1Y".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::BasisSwap);
        assert!(det.secondary_market_index().is_some());
    }

    #[test]
    fn parse_future_identifier() {
        let det: QuoteDetails = "Future_USD_SOFR_H6".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::Future);
        assert_eq!(det.contract_code(), Some("H6"));
    }

    #[test]
    fn parse_convexity_adjustment_identifier() {
        let det: QuoteDetails = "ConvexityAdjustment_USD_SOFR_M6".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::ConvexityAdjustment);
    }

    #[test]
    fn parse_cap_floor_identifier() {
        let det: QuoteDetails = "CapFloor_USD_SOFR_1Y_Absolute_Black".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::CapFloor);
        assert_eq!(det.strike(), Some(Strike::Absolute(0.0)));

        let det2: QuoteDetails = "CapFloor_USD_SOFR_1Y_Absolute_0.03_Black".parse().unwrap();
        assert_eq!(det2.strike(), Some(Strike::Absolute(0.03)));
    }

    #[test]
    fn parse_caplet_floorlet_identifier() {
        let det: QuoteDetails = "CapletFloorlet_USD_TermSOFR3m_3M_3M_Absolute_0.010_Straddle_Black"
            .parse()
            .unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::CapletFloorlet);
        assert_eq!(det.strike(), Some(Strike::Absolute(0.010)));
    }

    #[test]
    fn parse_swaption_identifier() {
        let det: QuoteDetails = "Swaption_USD_SOFR_3M_2Y_Absolute_Black".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EuropeanSwaption);
    }

    #[test]
    fn parse_outright_forward_identifier() {
        let det: QuoteDetails = "FxForward_EURUSD_1M_AnchorOutrightPrice".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FxForward);
        assert_eq!(det.pay_currency(), Some(Currency::EUR));
        assert_eq!(det.receive_currency(), Some(Currency::USD));
        assert_eq!(
            det.fx_forward_calibration_strategy(),
            Some(FxForwardCalibrationStrategy::AnchorOutrightPrice)
        );
    }

    #[test]
    fn parse_forward_points_identifier() {
        let det: QuoteDetails = "FxForward_EURUSD_1Y_AnchorForwardPoints".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FxForward);
        assert_eq!(
            det.fx_forward_calibration_strategy(),
            Some(FxForwardCalibrationStrategy::AnchorForwardPoints)
        );
    }

    #[test]
    fn parse_cross_currency_swap_identifier() {
        let det: QuoteDetails = "FixFloatCrossCurrencySwap_USD_ICP_CLP_1Y".parse().unwrap();
        assert_eq!(
            *det.instrument(),
            QuoteInstrument::FixFloatCrossCurrencySwap
        );
        assert_eq!(det.pay_currency(), Some(Currency::USD));
        assert_eq!(det.receive_currency(), Some(Currency::CLP));
    }

    #[test]
    fn parse_call_identifier() {
        let det: QuoteDetails = "EquityCall_USD_SPX_1Y_Absolute_5000".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EquityCall);
        assert_eq!(
            det.market_index(),
            Some(&MarketIndex::Equity("SPX".to_string()))
        );
        assert_eq!(det.option_expiry(), Some(Period::new(1, TimeUnit::Years)));
        assert_eq!(det.strike(), Some(Strike::Absolute(5000.0)));
    }

    #[test]
    fn parse_put_identifier() {
        let det: QuoteDetails = "EquityPut_USD_SPX_1Y_Relative_0.05".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EquityPut);
        assert_eq!(det.strike(), Some(Strike::Relative(0.05)));
    }

    #[test]
    fn parse_fx_call_identifier() {
        let det: QuoteDetails = "FxCall_EURUSD_1Y_Relative_0.05".parse().unwrap();
        assert!(matches!(det.market_index(), Some(MarketIndex::FxPair(_))));
        assert_eq!(det.option_expiry(), Some(Period::new(1, TimeUnit::Years)));
        assert_eq!(*det.instrument(), QuoteInstrument::FxCall);
        assert_eq!(det.pay_currency(), Some(Currency::EUR));
        assert_eq!(det.receive_currency(), Some(Currency::USD));
        assert_eq!(det.strike(), Some(Strike::Relative(0.05)));
    }

    #[test]
    fn parse_with_custom_separator() {
        let det = QuoteDetails::parse("EquityCall|USD|SPX|1Y|Absolute|5000", '|').unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EquityCall);
        assert_eq!(det.currency(), Some(Currency::USD));
        assert_eq!(det.strike(), Some(Strike::Absolute(5000.0)));
    }

    // -- build_instrument ---------------------------------------------------

    #[test]
    fn build_ois_swap() {
        let details: QuoteDetails = "OIS_USD_SOFR_1Y".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.0484));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        let CalibrationInstrumentType::OIS(swap) = inst else {
            panic!("expected OIS swap")
        };
        assert_eq!(swap.fixed_leg().cashflows().len(), 1);
        assert_eq!(swap.floating_leg().cashflows().len(), 1);
        let crate::instruments::cashflows::cashflowtype::CashflowType::FloatingRateCoupon(coupon) =
            &swap.floating_leg().cashflows()[0]
        else {
            panic!("expected overnight coupon")
        };
        assert!(coupon.overnight_conventions().is_some());
    }

    #[test]
    fn build_deposit() {
        let details: QuoteDetails = "FixedRateDeposit_USD_SOFR_6M".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.05));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::FixedRateDeposit(_)
        ));
    }

    #[test]
    fn build_price_anchored_bond() {
        let details: QuoteDetails = "FixedRateBond_USD_CORP_5Y_Semiannual_0.04_AnchorPrice"
            .parse()
            .unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(99.5));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::FixedRateBond(_, BondCalibrationStrategy::AnchorPrice)
        ));
    }

    #[test]
    fn build_basis_swap() {
        let details: QuoteDetails = "BasisSwap_USD_SOFR_TermSOFR3m_1Y".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.0003));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::BasisSwap(_)));
    }

    #[test]
    fn build_rate_futures() {
        let details: QuoteDetails = "Future_USD_SOFR_H6".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(94.75));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::RateFutures(_)));
    }

    #[test]
    fn build_fx_forward() {
        let details: QuoteDetails = "FxForward_EURUSD_1M_AnchorOutrightPrice".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(1.08));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::FxForward(
                _,
                FxForwardCalibrationStrategy::AnchorOutrightPrice
            )
        ));
    }

    #[test]
    fn build_cross_currency_swap() {
        let details: QuoteDetails = "FixFloatCrossCurrencySwap_USD_ICP_CLP_1Y".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.05));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::FixFloatCrossCurrencySwap(_)
        ));
    }

    #[test]
    fn build_call_option() {
        let details: QuoteDetails = "EquityCall_USD_SPX_1Y_Absolute_5000".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(150.0));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::EquityCall(option)
                if option.strike() == Strike::Absolute(5000.0)
        ));
    }

    #[test]
    fn build_put_option() {
        let details: QuoteDetails = "EquityPut_USD_SPX_1Y_Relative_0.05".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(100.0));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::EquityPut(option)
                if option.strike() == Strike::Relative(0.05)
        ));
    }

    #[test]
    fn build_fx_options() {
        let call = Quote::new(
            "FxCall_EURUSD_1Y_Absolute_1.10".parse().unwrap(),
            QuoteLevels::with_mid(0.12),
        )
        .build_instrument(ref_date(), Level::Mid, None)
        .unwrap();
        let put = Quote::new(
            "FxPut_EURUSD_1Y_Relative_0.05".parse().unwrap(),
            QuoteLevels::with_mid(0.11),
        )
        .build_instrument(ref_date(), Level::Mid, None)
        .unwrap();

        assert!(matches!(
            call,
            CalibrationInstrumentType::FxCall(option)
                if option.strike() == Strike::Absolute(1.10)
        ));
        assert!(matches!(
            put,
            CalibrationInstrumentType::FxPut(option)
                if option.strike() == Strike::Relative(0.05)
        ));
    }

    #[test]
    fn build_swaption() {
        let details: QuoteDetails = "Swaption_USD_SOFR_3M_2Y_Absolute_0.04_Black"
            .parse()
            .unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.33));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(
            inst,
            CalibrationInstrumentType::EuropeanSwaption(_)
        ));
    }

    #[test]
    fn build_cap_floor() {
        let details: QuoteDetails = "CapFloor_USD_SOFR_1Y_Absolute_0.03_Black".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.005));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::CapFloor(_)));
    }

    #[test]
    fn vol_quote_builds_caplet_floorlet() {
        let details: QuoteDetails =
            "CapletFloorlet_USD_TermSOFR3m_3M_3M_Absolute_0.010_Straddle_Black"
                .parse()
                .unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.33));
        let result = quote.build_instrument(ref_date(), Level::Mid, None);
        assert!(result.is_ok());
        let inst = result.unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::CapletFloorlet(_)));
    }

    // -- frequency parsing --------------------------------------------------

    #[test]
    fn parse_ois_with_frequencies() {
        let det: QuoteDetails = "OIS_USD_SOFR_1Y_Semiannual_Quarterly".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::OIS);
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.receive_leg_frequency(), Some(Frequency::Quarterly));
    }

    #[test]
    fn parse_ois_with_single_frequency() {
        let det: QuoteDetails = "OIS_USD_SOFR_1Y_Annual".parse().unwrap();
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Annual));
        assert_eq!(det.receive_leg_frequency(), None);
    }

    #[test]
    fn parse_ois_without_frequency_still_works() {
        let det: QuoteDetails = "OIS_USD_SOFR_1Y".parse().unwrap();
        assert_eq!(det.pay_leg_frequency(), None);
        assert_eq!(det.receive_leg_frequency(), None);
    }

    #[test]
    fn parse_regular_swap_quote_fields() {
        for (identifier, fixed, floating) in [
            ("Swap_USD_TermSOFR3m_5Y", None, None),
            (
                "Swap_USD_TermSOFR3m_5Y_Annual",
                Some(Frequency::Annual),
                None,
            ),
            (
                "Swap_USD_TermSOFR3m_5Y_Semiannual_Quarterly",
                Some(Frequency::Semiannual),
                Some(Frequency::Quarterly),
            ),
            (
                "Swap_USD_TermSOFR3m_5Y__Quarterly",
                None,
                Some(Frequency::Quarterly),
            ),
        ] {
            let details: QuoteDetails = identifier.parse().unwrap();
            assert_eq!(*details.instrument(), QuoteInstrument::Swap);
            assert_eq!(details.pay_leg_frequency(), fixed);
            assert_eq!(details.receive_leg_frequency(), floating);
            assert!(details.ois_conventions().is_err());
            let restored: QuoteDetails =
                serde_json::from_value(serde_json::to_value(&details).unwrap()).unwrap();
            assert_eq!(*restored.instrument(), QuoteInstrument::Swap);
        }
        for identifier in [
            "Swap_USD_SOFR",
            "Swap_USD_SOFR_1Y_garbage",
            "Swap_USD_SOFR_1Y_Annual_Quarterly_2",
            "Swap_USD_SOFR_1Y_Annual_bad",
        ] {
            assert!(identifier.parse::<QuoteDetails>().is_err());
        }
    }

    #[test]
    fn build_regular_swap_keeps_single_fixing_coupons() {
        use crate::instruments::cashflows::cashflowtype::CashflowType;

        let reference = ref_date();
        for (identifier, fixed_count, floating_count) in [
            ("Swap_USD_TermSOFR3m_1Y", 2, 4),
            ("Swap_USD_TermSOFR3m_1Y_Annual_Semiannual", 1, 2),
            ("Swap_USD_SOFR_1Y", 2, 4),
        ] {
            let details: QuoteDetails = identifier.parse().unwrap();
            let quote = Quote::new(
                details,
                QuoteLevels::new(Some(0.04), Some(0.03), Some(0.05)),
            );
            let built = quote.build_instrument(reference, Level::Ask, None).unwrap();
            let CalibrationInstrumentType::Swap(swap) = &built else {
                panic!("expected regular swap")
            };
            let fixed: Vec<_> = swap
                .fixed_leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::FixedRateCoupon(coupon) = cashflow {
                        Some(coupon)
                    } else {
                        None
                    }
                })
                .collect();
            let floating: Vec<_> = swap
                .floating_leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::FloatingRateCoupon(coupon) = cashflow {
                        Some(coupon)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(fixed.len(), fixed_count);
            assert_eq!(floating.len(), floating_count);
            assert_eq!(fixed[0].accrual_start_date(), reference);
            assert!((fixed[0].rate().rate() - 0.05).abs() < 1e-12);
            assert!(floating
                .iter()
                .all(|coupon| coupon.overnight_conventions().is_none()));
            assert_eq!(
                quote.calibration_pillar_date(&built).unwrap(),
                built.pillar_date().unwrap()
            );
        }
        let mismatched = Quote::new(
            "Swap_EUR_TermSOFR3m_1Y".parse().unwrap(),
            QuoteLevels::with_mid(0.04),
        );
        assert!(mismatched
            .build_instrument(reference, Level::Mid, None)
            .is_err());
        let term_ois = Quote::new(
            "OIS_USD_TermSOFR3m_1Y".parse().unwrap(),
            QuoteLevels::with_mid(0.04),
        );
        assert!(term_ois
            .build_instrument(reference, Level::Mid, None)
            .is_err());
    }

    #[test]
    fn ois_quote_parameters_preserve_index_defaults() {
        let default: QuoteDetails = "OIS_USD_SOFR_1Y".parse().unwrap();
        assert_eq!(default.ois_conventions().unwrap().settlement_days(), 2);
        assert_eq!(default.ois_conventions().unwrap().payment_lag(), 2);
        let overridden: QuoteDetails = "OIS_USD_SOFR_1Y_Annual_Annual_0".parse().unwrap();
        let conventions = overridden.ois_conventions().unwrap();
        assert_eq!(conventions.settlement_days(), 0);
        assert_eq!(conventions.payment_lag(), 2);
        assert!(matches!(conventions.pillar(), OisPillar::LastRelevantDate));
        let serialized = serde_json::to_string(&overridden).unwrap();
        assert!(!serialized.contains("ois_conventions"));
        let restored: QuoteDetails = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.ois_conventions().unwrap().settlement_days(), 0);
        assert_eq!(restored.ois_conventions().unwrap().payment_lag(), 2);
    }

    #[test]
    fn ois_quote_optional_positions_do_not_shift_when_empty() {
        let details: QuoteDetails = "OIS_USD_SOFR_1Y___0_3".parse().unwrap();
        assert_eq!(details.pay_leg_frequency(), None);
        assert_eq!(details.receive_leg_frequency(), None);
        let conventions = details.ois_conventions().unwrap();
        assert_eq!(conventions.fixed_frequency(), Frequency::Annual);
        assert_eq!(conventions.overnight_frequency(), Frequency::Annual);
        assert_eq!(conventions.settlement_days(), 0);
        assert_eq!(conventions.payment_lag(), 3);
        let details: QuoteDetails = "OIS_USD_SOFR_1Y__Quarterly".parse().unwrap();
        assert_eq!(details.pay_leg_frequency(), None);
        assert_eq!(details.receive_leg_frequency(), Some(Frequency::Quarterly));
        assert_eq!(details.ois_conventions().unwrap().payment_lag(), 2);
        let excess = format!("OIS_USD_SOFR_1Y_{}", [""; 24].join("_"));
        assert!(excess.parse::<QuoteDetails>().is_err());
    }

    #[test]
    fn ois_quote_parameters_parse_contract_and_observations() {
        let details: QuoteDetails = concat!(
            "OIS_USD_SOFR_5Y_Semiannual_Quarterly_1_3_Actual365",
            "_UnitedStates(Sofr)_WeekendsOnly_UnitedStates(Sofr)_UnitedStates(Settlement)",
            "_Following_Preceding_ModifiedFollowing_Forward_false_1M",
            "_2027-01-05_2030-01-05_Simple_2_1_true_true_CustomDate(2030-01-05)"
        )
        .parse()
        .unwrap();
        let conventions = details.ois_conventions().unwrap();
        assert_eq!(conventions.settlement_days(), 1);
        assert_eq!(conventions.payment_lag(), 3);
        assert_eq!(conventions.fixed_frequency(), Frequency::Semiannual);
        assert_eq!(conventions.overnight_frequency(), Frequency::Quarterly);
        assert_eq!(conventions.fixed_day_counter(), Some(DayCounter::Actual365));
        assert_eq!(conventions.end_of_month(), Some(false));
        assert_eq!(conventions.first_coupon_date(), Some(Date::new(2027, 1, 5)));
        assert_eq!(
            conventions.next_to_last_coupon_date(),
            Some(Date::new(2030, 1, 5))
        );
        assert_eq!(
            conventions.overnight().averaging(),
            crate::instruments::rates::oisconventions::OvernightAveraging::Simple
        );
        assert_eq!(conventions.overnight().lookback_days(), 2);
        assert_eq!(conventions.overnight().lockout_days(), 1);
        assert!(conventions.overnight().observation_shift());
        assert!(conventions.overnight().compound_spread_daily());
        assert_eq!(
            String::from(conventions.payment_calendar().unwrap().clone()),
            "UnitedStates(Settlement)"
        );
    }

    #[test]
    fn ois_quote_parameters_reject_ambiguous_or_invalid_inputs() {
        for suffix in [
            "SettlementDays=0",
            "Annual_Annual_PaymentLag=2",
            "Annual_Annual_-1",
            "Annual_Annual_2_-1",
            "Annual_Annual_2_abc",
            "Annual_Annual_2_2_UnitedStates(Sofr)",
            "Annual_Annual_2_2_Actual360_UnitedStates",
            "Annual_garbage",
            "garbage",
        ] {
            assert!(
                format!("OIS_USD_SOFR_1Y_{suffix}")
                    .parse::<QuoteDetails>()
                    .is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn ois_quote_parameters_follow_the_identifier_separator() {
        let identifier = "OIS|USD|SOFR|1Y|Annual|Annual|0|3";
        let details = QuoteDetails::parse(identifier, '|').unwrap();
        assert_eq!(details.identifier(), identifier);
        assert_eq!(details.ois_conventions().unwrap().settlement_days(), 0);
        assert_eq!(details.ois_conventions().unwrap().payment_lag(), 3);
        let serialized = serde_json::to_value(&details).unwrap();
        let restored: QuoteDetails = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(restored.ois_conventions().unwrap().payment_lag(), 3);
        let quote = Quote::new(restored, QuoteLevels::with_mid(0.04));
        let built = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(built, CalibrationInstrumentType::OIS(_)));
        let mut conflicting = serialized;
        conflicting["ois_conventions"] = serde_json::json!({"payment_lag": 0});
        assert!(serde_json::from_value::<QuoteDetails>(conflicting).is_err());
    }

    #[test]
    fn parse_basis_swap_with_frequencies() {
        let det: QuoteDetails = "BasisSwap_USD_SOFR_TermSOFR3m_1Y_Quarterly_Monthly"
            .parse()
            .unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::BasisSwap);
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Quarterly));
        assert_eq!(det.receive_leg_frequency(), Some(Frequency::Monthly));
    }

    #[test]
    fn parse_fix_float_xccy_with_frequencies() {
        let det: QuoteDetails = "FixFloatCrossCurrencySwap_USD_ICP_CLP_1Y_Semiannual_Quarterly"
            .parse()
            .unwrap();
        assert_eq!(
            *det.instrument(),
            QuoteInstrument::FixFloatCrossCurrencySwap
        );
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.receive_leg_frequency(), Some(Frequency::Quarterly));
    }

    #[test]
    fn parse_float_float_xccy_with_frequencies() {
        let det: QuoteDetails =
            "FloatFloatCrossCurrencySwap_CLP_ICP_SOFR_USD_1Y_Quarterly_Quarterly"
                .parse()
                .unwrap();
        assert_eq!(
            *det.instrument(),
            QuoteInstrument::FloatFloatCrossCurrencySwap
        );
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Quarterly));
        assert_eq!(det.receive_leg_frequency(), Some(Frequency::Quarterly));
    }

    #[test]
    fn parse_swaption_with_frequencies() {
        let det: QuoteDetails = "Swaption_USD_SOFR_3M_2Y_Semiannual_Semiannual_Absolute_0.04_Black"
            .parse()
            .unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EuropeanSwaption);
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.receive_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.strike(), Some(Strike::Absolute(0.04)));
    }

    #[test]
    fn parse_swaption_without_frequencies_still_works() {
        let det: QuoteDetails = "Swaption_USD_SOFR_3M_2Y_Absolute_Black".parse().unwrap();
        assert_eq!(det.pay_leg_frequency(), None);
        assert_eq!(det.receive_leg_frequency(), None);
    }

    #[test]
    fn parse_cap_floor_with_frequency() {
        let det: QuoteDetails = "CapFloor_USD_SOFR_1Y_Quarterly_Absolute_0.03_Black"
            .parse()
            .unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::CapFloor);
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Quarterly));
        assert_eq!(det.strike(), Some(Strike::Absolute(0.03)));
    }

    #[test]
    fn parse_cap_floor_without_frequency_still_works() {
        let det: QuoteDetails = "CapFloor_USD_SOFR_1Y_Absolute_Black".parse().unwrap();
        assert_eq!(det.pay_leg_frequency(), None);
    }
}

#[cfg(test)]
mod case_insensitive_tests {
    use super::*;

    #[test]
    fn products_and_typed_fields_accept_lower_upper_and_mixed_case() {
        for identifier in [
            "OIS_USD_SOFR_1Y_Annual_Annual_2_0_Actual360_TARGET_TARGET_TARGET_TARGET_ModifiedFollowing_ModifiedFollowing_ModifiedFollowing_Backward_FALSE_0D",
            "Swap_USD_SOFR_5Y_Annual_Quarterly",
            "FixedRateDeposit_USD_SOFR_3M",
            "FixedRateBond_USD_SOFR_5Y_Semiannual_0.04_AnchorPrice",
            "BasisSwap_USD_SOFR_TermSOFR3m_5Y_Quarterly_Quarterly",
            "FixFloatCrossCurrencySwap_USD_SOFR_EUR_5Y_Annual_Annual",
            "FloatFloatCrossCurrencySwap_USD_SOFR_ESTR_EUR_5Y_Quarterly_Quarterly",
            "CapFloor_USD_SOFR_1Y_Quarterly_Absolute_0.03_Black",
            "CapletFloorlet_USD_TermSOFR3m_3M_3M_Absolute_0.01_Straddle_Normal",
            "Swaption_USD_SOFR_3M_2Y_Semiannual_Semiannual_Absolute_0.04_Black",
            "FxForward_EURUSD_1Y_AnchorOutrightPrice",
            "EquityCall_USD_SPX_1Y_Absolute_5000",
            "FxPut_EURUSD_1Y_Absolute_1.10",
            "Cds_Acme_USD_5Y",
            "DiscountFactor_USD_SOFR_1Y",
            "ZeroRate_USD_SOFR_1Y_Continuous_Annual",
        ] {
            let canonical = identifier.parse::<QuoteDetails>().unwrap_or_else(|error| panic!("{identifier}: {error}"));
            for variant in [identifier.to_ascii_lowercase(), identifier.to_ascii_uppercase()] {
                let parsed = variant.parse::<QuoteDetails>().unwrap_or_else(|error| panic!("{variant}: {error}"));
                assert_eq!(parsed.identifier(), variant);
                assert_eq!(parsed.instrument(), canonical.instrument());
            }
        }
        let custom = QuoteDetails::parse("oIs|uSd|sOfR|1y|aNnUaL", '|').unwrap();
        custom.ois_conventions().unwrap();
        let equity: QuoteDetails = "equitycall_usd_MyTicker_1y_absolute_10".parse().unwrap();
        assert_eq!(
            equity.market_index(),
            Some(MarketIndex::Equity("MyTicker".into())).as_ref()
        );
    }
}
