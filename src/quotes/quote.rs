use serde::{Deserialize, Serialize};

use crate::{
    ad::scalar::Scalar,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        equity::equityeuropeanoption::{EquityEuropeanOption, EuroOptionType},
        fixedincome::{
            fixedratedeposit::FixedRateDeposit, makefixedratedeposit::MakeFixedRateDeposit,
        },
        fx::{fxforward::FxForward, makefxforward::MakeFxForward},
        rates::{
            basisswap::BasisSwap,
            capfloor::{CapFloor, CapFloorType},
            crosscurrencyswap::FixFloatCrossCurrencySwap,
            floatfloatcrosscurrencyswap::FloatFloatCrossCurrencySwap,
            makebasisswap::MakeBasisSwap,
            makecapfloor::MakeCapFloor,
            makefixfloatcrosscurrencyswap::MakeFixFloatCrossCurrencySwap,
            makefloatfloatcrosscurrencyswap::MakeFloatFloatCrossCurrencySwap,
            makeratefutures::MakeRateFutures,
            makeswap::MakeSwap,
            makeswaption::MakeSwaption,
            ratefutures::RateFutures,
            swap::Swap,
            swaption::Swaption,
        },
    },
    time::{date::Date, enums::Frequency, imm::IMM, period::Period},
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::VolatilityType,
};

/// Splits a 6-character FX pair string (e.g. `"EURUSD"`) into two currencies.
fn parse_fx_pair(pair: &str) -> Result<(Currency, Currency)> {
    if pair.len() < 6 {
        return Err(QSError::InvalidValueErr(format!(
            "Invalid FX currency pair: {pair}"
        )));
    }
    let base: Currency = pair[..3].parse()?;
    let quote_ccy: Currency = pair[3..6].parse()?;
    Ok((base, quote_ccy))
}

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

/// Quote levels associated with an instrument identifier. When multiple levels are provided the `mid` is preferred, otherwise `bid/ask`
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

/// Represents if the strike is quoted in absolute or relative values.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum StrikeType {
    /// Absolute strike.
    Absolute,
    /// Relative (moneyness) strike.
    Relative,
}

impl std::str::FromStr for StrikeType {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Absolute" => Ok(Self::Absolute),
            "Relative" => Ok(Self::Relative),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown strike type: {s}"
            ))),
        }
    }
}

/// Represents the type of instruments that can be handled by the quoting system.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuoteInstrument {
    /// Deposit instrument.
    FixedRateDeposit,
    /// Basis swap instrument.
    BasisSwap,
    /// OIS swap instrument.
    OIS,
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
    /// FX forward points.
    FxForwardPoints,
    /// FX outright forward instrument.
    FxOutrightForward,
    /// Future instrument.
    Future,
    /// Convexity adjustment.
    ConvexityAdjustment,
    /// Caplet or floorlet instrument.
    CapletFloorlet,
    /// Swaption instrument.
    Swaption,
    /// Cap/Floor (requires stripping).
    CapFloor,
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

impl std::str::FromStr for OptionStrategy {
    type Err = QSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Straddle" => Ok(Self::Straddle),
            "Strangle" => Ok(Self::Strangle),
            "RiskReversal" => Ok(Self::RiskReversal),
            "Butterfly" => Ok(Self::Butterfly),
            _ => Err(QSError::InvalidValueErr(format!(
                "Unknown option strategy: {s}"
            ))),
        }
    }
}

/// A [`QuoteDetails`] contains all details related to a particular quote.
///
/// Instances can be built manually via [`QuoteDetails::new`] + builder setters,
/// or parsed from an identifier string via the [`std::str::FromStr`] trait.
///
/// # Identifier format
///
/// Each identifier is an underscore-separated string whose first segment
/// determines the instrument type. The table below shows the positional
/// parameters for every supported product. Square brackets denote optional
/// segments.
///
/// | Product | Pos 0 | Pos 1 | Pos 2 | Pos 3 | Pos 4 | Pos 5 | Pos 6 | Pos 7 | Pos 8 |
/// |---|---|---|---|---|---|---|---|---|---|
/// | OIS | `OIS` | CCY | Index | Tenor | \[PayFreq\] | \[RecvFreq\] | | | |
/// | FixedRateDeposit | `FixedRateDeposit` | CCY | Index | Tenor | | | | | |
/// | BasisSwap | `BasisSwap` | CCY | PayIndex | RecvIndex | Tenor | \[PayFreq\] | \[RecvFreq\] | | |
/// | FixFloatCrossCurrencySwap | `FixFloatCrossCurrencySwap` | DomCCY | FloatIndex | ForCCY | Tenor | \[DomFreq\] | \[ForFreq\] | | |
/// | FloatFloatCrossCurrencySwap | `FloatFloatCrossCurrencySwap` | DomCCY | DomIndex | ForIndex | ForCCY | Tenor | \[DomFreq\] | \[ForFreq\] | |
/// | CapFloor | `CapFloor` | CCY | Index | Tenor | \[Freq\] | StrikeType | \[Strike\] | VolType | |
/// | CapletFloorlet | `CapletFloorlet` | CCY | Index | IdxTenor | Expiry | StrikeType | \[Strike\] | Strategy | VolType |
/// | Future | `Future` | CCY | Index | IMMCode | | | | | |
/// | ConvexityAdjustment | `ConvexityAdjustment` | CCY | Index | IMMCode | | | | | |
/// | Swaption | `Swaption` | CCY | Index | Expiry | SwapTenor | \[PayFreq\] | \[RecvFreq\] | StrikeType | \[Strike\] VolType |
/// | FxOutrightForward | `FxOutrightForward` | CCYPAIR | Tenor | | | | | | |
/// | FxForwardPoints | `FxForwardPoints` | CCYPAIR | Tenor | | | | | | |
/// | EquityCall | `EquityCall` | CCY | Index | Tenor | Strike | | | | |
/// | EquityPut | `EquityPut` | CCY | Index | Tenor | Strike | | | | |
/// | FxCall | `FxCall` | CCYPAIR | Tenor | Strike | | | | | |
/// | FxPut | `FxPut` | CCYPAIR | Tenor | Strike | | | | | |
///
/// **Frequency values**: `Annual`, `Semiannual`, `Quarterly`, `Monthly`,
/// `Bimonthly`, `Biweekly`, `Weekly`, `Daily`, `EveryFourthMonth`,
/// `EveryFourthWeek`, `Once`, `NoFrequency`.
///
/// # Examples
///
/// ```text
/// OIS_USD_SOFR_1Y
/// OIS_USD_SOFR_1Y_Semiannual_Semiannual
/// BasisSwap_USD_SOFR_TermSOFR3m_1Y_Quarterly_Quarterly
/// FixFloatCrossCurrencySwap_USD_ICP_CLP_1Y_Semiannual_Quarterly
/// Swaption_USD_SOFR_3M_2Y_Semiannual_Semiannual_Absolute_0.04_Black
/// CapFloor_USD_SOFR_1Y_Quarterly_Absolute_0.03_Black
/// EquityCall_USD_SPX_1Y_5000
/// FxCall_EURUSD_1Y_1.10
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuoteDetails {
    identifier: String,
    instrument: QuoteInstrument,
    #[serde(default)]
    market_index: Option<MarketIndex>,
    #[serde(default)]
    strategy: Option<OptionStrategy>,
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
    strike: Option<f64>,
    #[serde(default)]
    strike_type: Option<StrikeType>,
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
}

impl QuoteDetails {
    /// Creates a new quote details container with required fields.
    #[must_use]
    pub const fn new(identifier: String, instrument: QuoteInstrument) -> Self {
        Self {
            identifier,
            instrument,
            market_index: None,
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
            currency: None,
            secondary_market_index: None,
            option_expiry: None,
            contract_code: None,
            index_tenor: None,
            pay_leg_frequency: None,
            receive_leg_frequency: None,
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
    pub const fn strategy(&self) -> Option<OptionStrategy> {
        self.strategy
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
    pub const fn strike(&self) -> Option<f64> {
        self.strike
    }

    /// Returns the strike type, if present.
    #[must_use]
    pub const fn strike_type(&self) -> Option<StrikeType> {
        self.strike_type
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
    pub const fn with_strategy(mut self, s: OptionStrategy) -> Self {
        self.strategy = Some(s);
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
    pub const fn with_strike(mut self, s: f64) -> Self {
        self.strike = Some(s);
        self
    }
    /// Sets the strike type.
    #[must_use]
    pub const fn with_strike_type(mut self, t: StrikeType) -> Self {
        self.strike_type = Some(t);
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
        if let Some(p) = pay {
            let recv: Option<Frequency> = parts.get(start + 1).and_then(|s| s.parse().ok());
            if let Some(r) = recv {
                (Some(p), Some(r), start + 2)
            } else {
                (Some(p), None, start + 1)
            }
        } else {
            (None, None, start)
        }
    }

    /// `{Instrument}_CCY_{Index}_{Tenor}[_{PayFreq}[_{RecvFreq}]]`
    /// e.g. `OIS_USD_SOFR_1Y` or `OIS_USD_SOFR_1Y_Semiannual_Semiannual`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_ois(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "OIS identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(parts[3])?;

        let (pay_freq, recv_freq, _) = Self::try_parse_frequencies(parts, 4);

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
        Ok(det)
    }

    /// `{Instrument}_CCY_{Index}_{Tenor}` — e.g. `FixedRateDeposit_USD_SOFR_1Y`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fixed_rate_deposit(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "FixedRateDeposit identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(parts[3])?;
        Ok(Self::new(id.to_string(), QuoteInstrument::FixedRateDeposit)
            .with_market_index(index)
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
        let currency: Currency = parts[1].parse()?;
        let pay_index = parts[2].parse::<MarketIndex>()?;
        let recv_index = parts[3].parse::<MarketIndex>()?;
        let tenor = Period::from_str(parts[4])?;

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
        let domestic_currency: Currency = parts[1].parse()?;
        let floating_index = parts[2].parse::<MarketIndex>()?;
        let foreign_currency: Currency = parts[3].parse()?;
        let tenor = Period::from_str(parts[4])?;

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
        let domestic_currency: Currency = parts[1].parse()?;
        let dom_index = parts[2].parse::<MarketIndex>()?;
        let for_index = parts[3].parse::<MarketIndex>()?;
        let foreign_currency: Currency = parts[4].parse()?;
        let tenor = Period::from_str(parts[5])?;

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

    /// `{Instrument}_CCY_{Index}_{Tenor}[_{Freq}]_{StrikeType}_{VolType}` (without strike)
    ///
    /// `{Instrument}_CCY_{Index}_{Tenor}[_{Freq}]_{StrikeType}_{Strike}_{VolType}` (with strike)
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
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(parts[3])?;

        // Try optional frequency after tenor
        let (freq, next) = parts
            .get(4)
            .and_then(|s| s.parse::<Frequency>().ok())
            .map_or((None, 4), |f| (Some(f), 5));

        let strike_type = parts[next].parse::<StrikeType>()?;

        // Try parsing next+1 as f64 (strike value). If it succeeds, the vol
        // type follows at next+2; otherwise next+1 is the vol type.
        let strike_idx = next + 1;
        let (strike, vol_idx) = parts
            .get(strike_idx)
            .and_then(|s| s.parse::<f64>().ok())
            .map_or((None, strike_idx), |s| (Some(s), strike_idx + 1));
        let vol_type: VolatilityType = parts
            .get(vol_idx)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing vol type in: {id}")))?
            .parse()?;

        let mut det = Self::new(id.to_string(), QuoteInstrument::CapFloor)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor)
            .with_strike_type(strike_type)
            .with_vol_type(vol_type);
        if let Some(k) = strike {
            det = det.with_strike(k);
        }
        if let Some(f) = freq {
            det = det.with_pay_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_CCY_{Index}_{IdxTenor}_{Expiry}_{StrikeType}_{Strike}_{Strategy}_{VolType}`
    ///
    /// Or without explicit strike: `.._{StrikeType}_{Strategy}_{VolType}`.
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
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let index_tenor = Period::from_str(parts[3])?;
        let option_expiry = Period::from_str(parts[4])?;
        let strike_type = parts[5].parse::<StrikeType>()?;

        let (strike, next_idx) = parts[6].parse::<f64>().map_or((None, 6), |s| (Some(s), 7));

        let strategy: OptionStrategy = parts
            .get(next_idx)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing strategy in: {id}")))?
            .parse()?;
        let vol_type: VolatilityType = parts
            .get(next_idx + 1)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing vol type in: {id}")))?
            .parse()?;

        let mut det = Self::new(id.to_string(), QuoteInstrument::CapletFloorlet)
            .with_market_index(index)
            .with_currency(currency)
            .with_index_tenor(index_tenor)
            .with_option_expiry(option_expiry)
            .with_strike_type(strike_type)
            .with_strategy(strategy)
            .with_vol_type(vol_type);
        if let Some(k) = strike {
            det = det.with_strike(k);
        }
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
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let code = parts[3].to_string();
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
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let code = parts[3].to_string();
        Ok(
            Self::new(id.to_string(), QuoteInstrument::ConvexityAdjustment)
                .with_market_index(index)
                .with_currency(currency)
                .with_contract_code(code),
        )
    }

    /// Swaption identifier parser.
    ///
    /// `{Instrument}_CCY_{Index}_{Expiry}_{SwapTenor}[_{PayFreq}_{RecvFreq}]_{StrikeType}_{VolType}` (no strike)
    /// `{Instrument}_CCY_{Index}_{Expiry}_{SwapTenor}[_{PayFreq}_{RecvFreq}]_{StrikeType}_{Strike}_{VolType}` (with strike)
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
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let option_expiry = Period::from_str(parts[3])?;
        let swap_tenor = Period::from_str(parts[4])?;

        let (pay_freq, recv_freq, next) = Self::try_parse_frequencies(parts, 5);

        let strike_type = parts[next].parse::<StrikeType>()?;

        let strike_idx = next + 1;
        let (strike, vol_idx) = parts
            .get(strike_idx)
            .and_then(|s| s.parse::<f64>().ok())
            .map_or((None, strike_idx), |s| (Some(s), strike_idx + 1));
        let vol_type: VolatilityType = parts
            .get(vol_idx)
            .ok_or_else(|| QSError::InvalidValueErr(format!("Missing vol type in: {id}")))?
            .parse()?;

        let mut det = Self::new(id.to_string(), QuoteInstrument::Swaption)
            .with_market_index(index)
            .with_currency(currency)
            .with_option_expiry(option_expiry)
            .with_tenor(swap_tenor)
            .with_strike_type(strike_type)
            .with_vol_type(vol_type);
        if let Some(k) = strike {
            det = det.with_strike(k);
        }
        if let Some(f) = pay_freq {
            det = det.with_pay_leg_frequency(f);
        }
        if let Some(f) = recv_freq {
            det = det.with_receive_leg_frequency(f);
        }
        Ok(det)
    }

    /// `{Instrument}_{CCYPAIR}_{Tenor}` — e.g. `FxOutrightForward_EURUSD_1M`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_outright_forward(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 3 {
            return Err(QSError::InvalidValueErr(format!(
                "OutrightForward identifier too short: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(parts[2])?;
        Ok(
            Self::new(id.to_string(), QuoteInstrument::FxOutrightForward)
                .with_pay_currency(base)
                .with_receive_currency(quote_ccy)
                .with_tenor(tenor),
        )
    }

    /// `{Instrument}_{CCYPAIR}_{Tenor}` — e.g. `FxForwardPoints_EURUSD_1M`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_forward_points(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 3 {
            return Err(QSError::InvalidValueErr(format!(
                "ForwardPoints identifier too short: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(parts[2])?;
        Ok(Self::new(id.to_string(), QuoteInstrument::FxForwardPoints)
            .with_pay_currency(base)
            .with_receive_currency(quote_ccy)
            .with_tenor(tenor))
    }

    /// `{Instrument}_CCY_{Index}_{Expiry}_{Strike}` — e.g. `EquityCall_USD_SPX_1Y_5000`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_equity_call(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 5 {
            return Err(QSError::InvalidValueErr(format!(
                "Call identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(parts[3])?;
        let strike: f64 = parts[4]
            .parse()
            .map_err(|e| QSError::InvalidValueErr(format!("Bad strike in {id}: {e}")))?;
        Ok(Self::new(id.to_string(), QuoteInstrument::EquityCall)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor)
            .with_strike(strike))
    }

    /// `{Instrument}_CCY_{Index}_{Expiry}_{Strike}` — e.g. `EquityPut_USD_SPX_1Y_5000`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_equity_put(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 5 {
            return Err(QSError::InvalidValueErr(format!(
                "Put identifier too short: {id}"
            )));
        }
        let currency: Currency = parts[1].parse()?;
        let index = parts[2].parse::<MarketIndex>()?;
        let tenor = Period::from_str(parts[3])?;
        let strike: f64 = parts[4]
            .parse()
            .map_err(|e| QSError::InvalidValueErr(format!("Bad strike in {id}: {e}")))?;
        Ok(Self::new(id.to_string(), QuoteInstrument::EquityPut)
            .with_market_index(index)
            .with_currency(currency)
            .with_tenor(tenor)
            .with_strike(strike))
    }

    /// `{Instrument}_{CCYPAIR}_{Expiry}_{Strike}` — e.g. `FxCall_EURUSD_1Y_1.10`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fx_call(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "FxCall identifier too short: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(parts[2])?;
        let strike: f64 = parts[3]
            .parse()
            .map_err(|e| QSError::InvalidValueErr(format!("Bad strike in {id}: {e}")))?;

        Ok(Self::new(id.to_string(), QuoteInstrument::FxCall)
            .with_pay_currency(base)
            .with_receive_currency(quote_ccy)
            .with_tenor(tenor)
            .with_strike(strike))
    }

    /// `{Instrument}_{CCYPAIR}_{Expiry}_{Strike}` — e.g. `FxPut_EURUSD_1Y_1.10`
    ///
    /// # Errors
    /// Returns an error if the identifier is too short or fields cannot be parsed.
    pub fn parse_fx_put(id: &str, parts: &[&str]) -> Result<Self> {
        if parts.len() < 4 {
            return Err(QSError::InvalidValueErr(format!(
                "FxPut identifier too short: {id}"
            )));
        }
        let (base, quote_ccy) = parse_fx_pair(parts[1])?;
        let tenor = Period::from_str(parts[2])?;
        let strike: f64 = parts[3]
            .parse()
            .map_err(|e| QSError::InvalidValueErr(format!("Bad strike in {id}: {e}")))?;

        Ok(Self::new(id.to_string(), QuoteInstrument::FxPut)
            .with_pay_currency(base)
            .with_receive_currency(quote_ccy)
            .with_tenor(tenor)
            .with_strike(strike))
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

        match parts[0] {
            "OIS" => Self::parse_ois(s, &parts),
            "FixedRateDeposit" => Self::parse_fixed_rate_deposit(s, &parts),
            "BasisSwap" => Self::parse_basis_swap(s, &parts),
            "FixFloatCrossCurrencySwap" => Self::parse_fix_float_cross_currency_swap(s, &parts),
            "CapFloor" => Self::parse_cap_floor(s, &parts),
            "CapletFloorlet" => Self::parse_caplet_floorlet(s, &parts),
            "Future" => Self::parse_future(s, &parts),
            "ConvexityAdjustment" => Self::parse_convexity_adjustment(s, &parts),
            "Swaption" => Self::parse_swaption(s, &parts),
            "FxOutrightForward" | "OutrightForward" => Self::parse_outright_forward(s, &parts),
            "FloatFloatCrossCurrencySwap" => Self::parse_float_float_cross_currency_swap(s, &parts),
            "FxForwardPoints" | "ForwardPoints" => Self::parse_forward_points(s, &parts),
            "EquityCall" | "Call" => Self::parse_equity_call(s, &parts),
            "EquityPut" | "Put" => Self::parse_equity_put(s, &parts),
            "FxCall" => Self::parse_fx_call(s, &parts),
            "FxPut" => Self::parse_fx_put(s, &parts),
            other => Err(QSError::InvalidValueErr(format!(
                "Unknown instrument type in identifier: {other}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// FromStr – parse a quote identifier into a QuoteDetails
// ---------------------------------------------------------------------------

impl std::str::FromStr for QuoteDetails {
    type Err = QSError;

    /// Parses a quote identifier string (underscore-separated) into [`QuoteDetails`].
    ///
    /// The first `_`-delimited segment determines the instrument type and must
    /// match the exact [`QuoteInstrument`] variant name (or the FX-specific
    /// tags `OutrightForward`/`ForwardPoints`).
    ///
    /// # Errors
    /// Returns an error if the identifier cannot be parsed.
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s, '_')
    }
}

// ---------------------------------------------------------------------------
// CalibrationInstrumentType
// ---------------------------------------------------------------------------

/// Wraps every concrete instrument type that can be produced from a [`Quote`].
#[derive(Clone)]
pub enum CalibrationInstrumentType<T = f64>
where
    T: Scalar,
{
    /// A vanilla fixed-rate deposit.
    FixedRateDeposit(FixedRateDeposit<T>),
    /// A fixed-vs-floating interest rate swap (e.g. OIS).
    Swap(Swap<T>),
    /// A floating-vs-floating basis swap.
    BasisSwap(BasisSwap<T>),
    /// A rate futures contract.
    RateFutures(RateFutures),
    /// An FX outright forward.
    FxForward(FxForward),
    /// A cross-currency swap (fixed domestic vs floating foreign).
    FixFloatCrossCurrencySwap(FixFloatCrossCurrencySwap<T>),
    /// A float-float cross-currency swap (both legs floating).
    FloatFloatCrossCurrencySwap(FloatFloatCrossCurrencySwap<T>),
    /// A European equity call option.
    Call(EquityEuropeanOption),
    /// A European equity put option.
    Put(EquityEuropeanOption),
    /// An interest rate cap or floor.
    CapFloor(CapFloor),
    /// A swaption (option on a swap).
    Swaption(Swaption<T>),
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
            Self::FxForward(x) => Ok(x.delivery_date()),
            _ => Err(QSError::InvalidValueErr("Instrument not supported".into())),
        }
    }
}

// ---------------------------------------------------------------------------
// Quote
// ---------------------------------------------------------------------------

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
            QuoteInstrument::FixedRateDeposit => {
                self.build_fixed_rate_deposit(value, reference_date, notional)
            }
            QuoteInstrument::BasisSwap => self.build_basis_swap(value, reference_date, notional),
            QuoteInstrument::Future => self.build_rate_futures(value, reference_date),
            QuoteInstrument::FxOutrightForward => self.build_fx_forward(value, reference_date),
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
            QuoteInstrument::EquityCall => self.build_call(reference_date),
            QuoteInstrument::EquityPut => self.build_put(reference_date),
            QuoteInstrument::CapFloor => self.build_cap_floor(value, reference_date, notional),
            QuoteInstrument::Swaption => self.build_swaption(value, reference_date, notional),
            QuoteInstrument::FxForwardPoints => self.build_fx_forward_points(value, reference_date),
            QuoteInstrument::FxCall | QuoteInstrument::FxPut => Err(QSError::NotImplementedErr(
                "FX option instrument builders are not implemented yet".into(),
            )),
            other => Err(QSError::NotImplementedErr(format!(
                "Cannot build instrument for {other:?} — it is a vol / auxiliary quote type"
            ))),
        }
    }

    /// wtf?
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

        let maturity = reference_date + tenor;
        let market_index = Self::required_market_index(d, "OIS quote")?;
        let rd = market_index.rate_index_details()?.rate_definition();

        let mut builder = MakeSwap::<T>::default()
            .with_identifier(d.identifier())
            .with_start_date(reference_date)
            .with_maturity_date(maturity)
            .with_fixed_rate(rate)
            .with_notional(notional)
            .with_rate_definition(rd)
            .with_currency(currency)
            .with_market_index(market_index);
        if let Some(f) = d.pay_leg_frequency() {
            builder = builder.with_fixed_leg_frequency(f);
        }
        if let Some(f) = d.receive_leg_frequency() {
            builder = builder.with_floating_leg_frequency(f);
        }
        let swap = builder.build()?;

        Ok(CalibrationInstrumentType::Swap(swap))
    }

    /// Fixed Rate Deposit — mid value is the deposit rate.
    fn build_fixed_rate_deposit<T: Scalar + Default>(
        &self,
        rate: f64,
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
            .with_rate(rate)
            .with_notional(notional)
            .with_rate_definition(rd)
            .with_currency(currency)
            .with_discount_index(Some(market_index))
            .build()?;

        Ok(CalibrationInstrumentType::FixedRateDeposit(deposit))
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

    /// FX Forward — mid value is the outright forward rate.
    fn build_fx_forward<T: Scalar + Default>(
        &self,
        forward_rate: f64,
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

        let fwd = MakeFxForward::default()
            .with_identifier(d.identifier())
            .with_delivery_date(delivery_date)
            .with_forward_rate(forward_rate)
            .with_base_currency(base)
            .with_quote_currency(quote_ccy)
            .build()?;

        Ok(CalibrationInstrumentType::FxForward(fwd))
    }

    /// FX Forward Points — mid value is the forward points (absolute).
    ///
    /// Builds an [`FxForward`] with `forward_points` set. The bootstrap
    /// residual combines these with the FX spot (from the discount policy)
    /// to solve for discount factors via covered interest-rate parity.
    fn build_fx_forward_points<T: Scalar + Default>(
        &self,
        points: f64,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let base = d
            .pay_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Base currency on FX fwd pts quote".into()))?;
        let quote_ccy = d
            .receive_currency()
            .ok_or_else(|| QSError::ValueNotSetErr("Quote currency on FX fwd pts quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on FX fwd pts quote".into()))?;

        let delivery_date = reference_date + tenor;

        let fwd = MakeFxForward::default()
            .with_identifier(d.identifier())
            .with_delivery_date(delivery_date)
            .with_forward_points(points)
            .with_base_currency(base)
            .with_quote_currency(quote_ccy)
            .build()?;

        Ok(CalibrationInstrumentType::FxForward(fwd))
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

    /// European equity Call — strike and expiry from details.
    fn build_call<T: Scalar + Default>(
        &self,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let strike = d
            .strike()
            .ok_or_else(|| QSError::ValueNotSetErr("Strike on Call quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on Call quote".into()))?;
        let expiry = reference_date + tenor;

        let market_index = Self::required_market_index(d, "Call quote")?;
        let opt = EquityEuropeanOption::new(
            market_index,
            expiry,
            strike,
            EuroOptionType::Call,
            d.identifier(),
        );
        Ok(CalibrationInstrumentType::Call(opt))
    }

    /// European equity Put — strike and expiry from details.
    fn build_put<T: Scalar + Default>(
        &self,
        reference_date: Date,
    ) -> Result<CalibrationInstrumentType<T>> {
        let d = &self.details;
        let strike = d
            .strike()
            .ok_or_else(|| QSError::ValueNotSetErr("Strike on Put quote".into()))?;
        let tenor = d
            .tenor()
            .ok_or_else(|| QSError::ValueNotSetErr("Tenor on Put quote".into()))?;
        let expiry = reference_date + tenor;

        let market_index = Self::required_market_index(d, "Put quote")?;
        let opt = EquityEuropeanOption::new(
            market_index,
            expiry,
            strike,
            EuroOptionType::Put,
            d.identifier(),
        );
        Ok(CalibrationInstrumentType::Put(opt))
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
        let strike = d.strike().unwrap_or(value);

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

        let strike = d.strike().unwrap_or(value);

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

        Ok(CalibrationInstrumentType::Swaption(swaption))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(det.strike().is_none());

        let det2: QuoteDetails = "CapFloor_USD_SOFR_1Y_Absolute_0.03_Black".parse().unwrap();
        assert_eq!(det2.strike(), Some(0.03));
    }

    #[test]
    fn parse_caplet_floorlet_identifier() {
        let det: QuoteDetails = "CapletFloorlet_USD_TermSOFR3m_3M_3M_Absolute_0.010_Straddle_Black"
            .parse()
            .unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::CapletFloorlet);
        assert_eq!(det.strike(), Some(0.010));
    }

    #[test]
    fn parse_swaption_identifier() {
        let det: QuoteDetails = "Swaption_USD_SOFR_3M_2Y_Absolute_Black".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::Swaption);
    }

    #[test]
    fn parse_outright_forward_identifier() {
        let det: QuoteDetails = "FxOutrightForward_EURUSD_1M".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FxOutrightForward);
        assert_eq!(det.pay_currency(), Some(Currency::EUR));
        assert_eq!(det.receive_currency(), Some(Currency::USD));
    }

    #[test]
    fn parse_forward_points_identifier() {
        let det: QuoteDetails = "FxForwardPoints_EURUSD_1Y".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FxForwardPoints);
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
        let det: QuoteDetails = "EquityCall_USD_SPX_1Y_5000".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EquityCall);
        assert_eq!(det.strike(), Some(5000.0));
    }

    #[test]
    fn parse_put_identifier() {
        let det: QuoteDetails = "EquityPut_USD_SPX_1Y_4500".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EquityPut);
        assert_eq!(det.strike(), Some(4500.0));
    }

    #[test]
    fn parse_fx_call_identifier() {
        let det: QuoteDetails = "FxCall_EURUSD_1Y_1.10".parse().unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::FxCall);
        assert_eq!(det.pay_currency(), Some(Currency::EUR));
        assert_eq!(det.receive_currency(), Some(Currency::USD));
    }

    #[test]
    fn parse_with_custom_separator() {
        let det = QuoteDetails::parse("EquityCall|USD|SPX|1Y|5000", '|').unwrap();
        assert_eq!(*det.instrument(), QuoteInstrument::EquityCall);
        assert_eq!(det.currency(), Some(Currency::USD));
        assert_eq!(det.strike(), Some(5000.0));
    }

    // -- build_instrument ---------------------------------------------------

    #[test]
    fn build_ois_swap() {
        let details: QuoteDetails = "OIS_USD_SOFR_1Y".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.0484));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::Swap(_)));
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
        let details: QuoteDetails = "FxOutrightForward_EURUSD_1M".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(1.08));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::FxForward(_)));
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
        let details: QuoteDetails = "EquityCall_USD_SPX_1Y_5000".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(150.0));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::Call(_)));
    }

    #[test]
    fn build_put_option() {
        let details: QuoteDetails = "EquityPut_USD_SPX_1Y_4500".parse().unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(100.0));
        let inst = quote
            .build_instrument(ref_date(), Level::Mid, None)
            .unwrap();
        assert!(matches!(inst, CalibrationInstrumentType::Put(_)));
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
        assert!(matches!(inst, CalibrationInstrumentType::Swaption(_)));
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
    fn vol_quote_returns_not_implemented() {
        // CapletFloorlet is a vol quote — no builder for it yet.
        let details: QuoteDetails =
            "CapletFloorlet_USD_TermSOFR3m_3M_3M_Absolute_0.010_Straddle_Black"
                .parse()
                .unwrap();
        let quote = Quote::new(details, QuoteLevels::with_mid(0.33));
        let result = quote.build_instrument(ref_date(), Level::Mid, None);
        assert!(result.is_err());
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
        assert_eq!(*det.instrument(), QuoteInstrument::Swaption);
        assert_eq!(det.pay_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.receive_leg_frequency(), Some(Frequency::Semiannual));
        assert_eq!(det.strike(), Some(0.04));
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
        assert_eq!(det.strike(), Some(0.03));
    }

    #[test]
    fn parse_cap_floor_without_frequency_still_works() {
        let det: QuoteDetails = "CapFloor_USD_SOFR_1Y_Absolute_Black".parse().unwrap();
        assert_eq!(det.pay_leg_frequency(), None);
    }
}
