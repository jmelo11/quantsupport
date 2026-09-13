//! Python trade specifications.
//!
//! Each Python trade class stores plain parameters; the corresponding Rust
//! instrument is built on demand — as `DualFwd` for AD-enabled pricing or as
//! `f64` for XVA contingent-claim decomposition.

use pyo3::prelude::*;
use quantsupport::prelude::{
    BasisSwap as QsBasisSwap, BasisSwapTrade, CapFloor as QsCapFloor, CapFloorTrade, CapFloorType,
    CapletFloorlet as QsCapletFloorlet, CapletFloorletTrade, CapletFloorletType, CdsTrade,
    Compounding, ContingentClaim, CreditDefaultSwap as QsCreditDefaultSwap, Currency, Date,
    DayCounter, DualFwd, EquityEuropeanOption as QsEquityEuropeanOption, EquityEuropeanOptionTrade,
    EuroOptionType, FixFloatCrossCurrencySwap as QsFixFloatXccy, FixFloatCrossCurrencySwapTrade,
    FixedRateBond as QsFixedRateBond, FixedRateBondTrade, FixedRateDeposit as QsFixedRateDeposit,
    FixedRateDepositTrade, FloatFloatCrossCurrencySwap, FloatFloatCrossCurrencySwapTrade,
    FloatingRateNote as QsFloatingRateNote, FloatingRateNoteTrade, Frequency,
    FxEuropeanOptionTrade, FxForwardTrade, IntoContingentClaims, LegsProvider, MakeBasisSwap,
    MakeCapFloor, MakeFixFloatCrossCurrencySwap, MakeFixedRateBond, MakeFixedRateDeposit,
    MakeFloatFloatCrossCurrencySwap, MakeFloatingRateNote, MakeFxEuropeanOption, MakeFxForward,
    MakeRateFutures, MakeSwap, MarketIndex, PaymentStructure, RateDefinition,
    RateFutures as QsRateFutures, RateFuturesTrade, Side, Strike, Swap as QsSwap, SwapTrade,
};

use crate::conv::{
    extract_cap_floor_type, extract_caplet_floorlet_type, extract_compounding, extract_currency,
    extract_date, extract_day_counter, extract_frequency, extract_market_index,
    extract_option_type, extract_payment_structure, extract_side, extract_strike, qs_err,
};

/// A vanilla fixed-vs-floating interest rate swap.
#[pyclass(name = "Swap", from_py_object)]
#[derive(Clone)]
pub struct Swap {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub fixed_rate: f64,
    pub spread: Option<f64>,
    pub currency: Currency,
    pub market_index: MarketIndex,
    pub side: Side,
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub fixed_leg_frequency: Frequency,
    pub floating_leg_frequency: Frequency,
    pub trade_date: Option<Date>,
}

macro_rules! build_swap_instrument {
    ($self:expr, $t:ty) => {{
        let mut mk = MakeSwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_fixed_rate($self.fixed_rate)
            .with_notional($self.notional)
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_currency($self.currency)
            .with_market_index($self.market_index.clone())
            .with_side($self.side)
            .with_fixed_leg_frequency($self.fixed_leg_frequency)
            .with_floating_leg_frequency($self.floating_leg_frequency);
        if let Some(spread) = $self.spread {
            mk = mk.with_spread(spread);
        }
        mk.build().map_err(qs_err)
    }};
}

impl Swap {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<SwapTrade<DualFwd>> {
        let instrument: QsSwap<DualFwd> = build_swap_instrument!(self, DualFwd)?;
        Ok(SwapTrade::new(
            instrument,
            self.trade_date(),
            self.notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsSwap<f64> = build_swap_instrument!(self, f64)?;
        let trade = SwapTrade::new(instrument, self.trade_date(), self.notional, self.side);
        trade.into_contingent_claims().map_err(qs_err)
    }
}

#[pymethods]
impl Swap {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        fixed_rate,
        currency,
        market_index,
        side = None,
        spread = None,
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        fixed_leg_frequency = None,
        floating_leg_frequency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        fixed_rate: f64,
        currency: &Bound<'_, PyAny>,
        market_index: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        spread: Option<f64>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        fixed_leg_frequency: Option<&Bound<'_, PyAny>>,
        floating_leg_frequency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            notional,
            fixed_rate,
            spread,
            currency: extract_currency(currency)?,
            market_index: extract_market_index(market_index)?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            day_counter: day_counter
                .map(extract_day_counter)
                .transpose()?
                .unwrap_or(DayCounter::Actual360),
            compounding: compounding
                .map(extract_compounding)
                .transpose()?
                .unwrap_or(Compounding::Simple),
            rate_frequency: rate_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            fixed_leg_frequency: fixed_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            floating_leg_frequency: floating_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    #[getter(start_date)]
    fn start_date_py(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.start_date,
        }
    }

    #[getter(maturity_date)]
    fn maturity_date_py(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.maturity_date,
        }
    }

    #[getter(notional)]
    fn notional_py(&self) -> f64 {
        self.notional
    }

    #[getter(fixed_rate)]
    fn fixed_rate_py(&self) -> f64 {
        self.fixed_rate
    }

    #[getter(currency)]
    fn currency_py(&self) -> crate::enums::Currency {
        self.currency.into()
    }

    #[getter(market_index)]
    fn market_index_py(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.market_index.clone(),
        }
    }

    #[getter(side)]
    fn side_py(&self) -> crate::enums::Side {
        self.side.into()
    }

    fn __repr__(&self) -> String {
        format!(
            "Swap(identifier='{}', start_date='{}', maturity_date='{}', notional={}, fixed_rate={})",
            self.identifier, self.start_date, self.maturity_date, self.notional, self.fixed_rate
        )
    }
}

/// A float-vs-float cross-currency swap.
#[pyclass(name = "CrossCurrencySwap", from_py_object)]
#[derive(Clone)]
pub struct CrossCurrencySwap {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub domestic_notional: f64,
    pub foreign_notional: f64,
    pub domestic_spread: f64,
    pub foreign_spread: f64,
    pub domestic_currency: Currency,
    pub foreign_currency: Currency,
    pub domestic_market_index: MarketIndex,
    pub foreign_market_index: MarketIndex,
    pub side: Side,
    pub domestic_leg_frequency: Frequency,
    pub foreign_leg_frequency: Frequency,
    pub trade_date: Option<Date>,
}

macro_rules! build_xccy_instrument {
    ($self:expr, $t:ty) => {{
        MakeFloatFloatCrossCurrencySwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_domestic_notional($self.domestic_notional)
            .with_foreign_notional($self.foreign_notional)
            .with_domestic_spread($self.domestic_spread)
            .with_foreign_spread($self.foreign_spread)
            .with_domestic_currency($self.domestic_currency)
            .with_foreign_currency($self.foreign_currency)
            .with_domestic_market_index($self.domestic_market_index.clone())
            .with_foreign_market_index($self.foreign_market_index.clone())
            .with_side($self.side)
            .with_domestic_leg_frequency($self.domestic_leg_frequency)
            .with_foreign_leg_frequency($self.foreign_leg_frequency)
            .build()
            .map_err(qs_err)
    }};
}

impl CrossCurrencySwap {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<FloatFloatCrossCurrencySwapTrade<DualFwd>> {
        let instrument: FloatFloatCrossCurrencySwap<DualFwd> =
            build_xccy_instrument!(self, DualFwd)?;
        Ok(FloatFloatCrossCurrencySwapTrade::new(
            instrument,
            self.trade_date(),
            self.domestic_notional,
            self.foreign_notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: FloatFloatCrossCurrencySwap<f64> = build_xccy_instrument!(self, f64)?;
        instrument
            .legs()
            .to_vec()
            .into_contingent_claims(&self.identifier)
            .map_err(qs_err)
    }
}

#[pymethods]
impl CrossCurrencySwap {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        domestic_notional,
        foreign_notional,
        domestic_currency,
        foreign_currency,
        domestic_market_index,
        foreign_market_index,
        side = None,
        domestic_spread = 0.0,
        foreign_spread = 0.0,
        domestic_leg_frequency = None,
        foreign_leg_frequency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        domestic_notional: f64,
        foreign_notional: f64,
        domestic_currency: &Bound<'_, PyAny>,
        foreign_currency: &Bound<'_, PyAny>,
        domestic_market_index: &Bound<'_, PyAny>,
        foreign_market_index: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        domestic_spread: f64,
        foreign_spread: f64,
        domestic_leg_frequency: Option<&Bound<'_, PyAny>>,
        foreign_leg_frequency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            domestic_notional,
            foreign_notional,
            domestic_spread,
            foreign_spread,
            domestic_currency: extract_currency(domestic_currency)?,
            foreign_currency: extract_currency(foreign_currency)?,
            domestic_market_index: extract_market_index(domestic_market_index)?,
            foreign_market_index: extract_market_index(foreign_market_index)?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            domestic_leg_frequency: domestic_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            foreign_leg_frequency: foreign_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    #[getter(start_date)]
    fn start_date_py(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.start_date,
        }
    }

    #[getter(maturity_date)]
    fn maturity_date_py(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.maturity_date,
        }
    }

    #[getter(domestic_currency)]
    fn domestic_currency_py(&self) -> crate::enums::Currency {
        self.domestic_currency.into()
    }

    #[getter(foreign_currency)]
    fn foreign_currency_py(&self) -> crate::enums::Currency {
        self.foreign_currency.into()
    }

    #[getter(domestic_market_index)]
    fn domestic_market_index_py(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.domestic_market_index.clone(),
        }
    }

    #[getter(foreign_market_index)]
    fn foreign_market_index_py(&self) -> crate::enums::MarketIndex {
        crate::enums::MarketIndex {
            inner: self.foreign_market_index.clone(),
        }
    }

    #[getter(side)]
    fn side_py(&self) -> crate::enums::Side {
        self.side.into()
    }

    fn __repr__(&self) -> String {
        format!(
            "CrossCurrencySwap(identifier='{}', start_date='{}', maturity_date='{}')",
            self.identifier, self.start_date, self.maturity_date
        )
    }
}

/// A float-vs-float single-currency basis swap.
#[pyclass(name = "BasisSwap", from_py_object)]
#[derive(Clone)]
pub struct BasisSwap {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub currency: Currency,
    pub pay_market_index: MarketIndex,
    pub receive_market_index: MarketIndex,
    pub pay_spread: Option<f64>,
    pub receive_spread: Option<f64>,
    pub side: Side,
    pub pay_leg_frequency: Frequency,
    pub receive_leg_frequency: Frequency,
    pub trade_date: Option<Date>,
}

macro_rules! build_basis_swap_instrument {
    ($self:expr, $t:ty) => {{
        let mut mk = MakeBasisSwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_notional($self.notional)
            .with_currency($self.currency)
            .with_pay_market_index($self.pay_market_index.clone())
            .with_receive_market_index($self.receive_market_index.clone())
            .with_side($self.side)
            .with_pay_leg_frequency($self.pay_leg_frequency)
            .with_receive_leg_frequency($self.receive_leg_frequency);
        if let Some(s) = $self.pay_spread {
            mk = mk.with_pay_spread(s);
        }
        if let Some(s) = $self.receive_spread {
            mk = mk.with_receive_spread(s);
        }
        mk.build().map_err(qs_err)
    }};
}

impl BasisSwap {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<BasisSwapTrade<DualFwd>> {
        let instrument: QsBasisSwap<DualFwd> = build_basis_swap_instrument!(self, DualFwd)?;
        Ok(BasisSwapTrade::new(
            instrument,
            self.trade_date(),
            self.notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsBasisSwap<f64> = build_basis_swap_instrument!(self, f64)?;
        instrument
            .legs()
            .to_vec()
            .into_contingent_claims(&self.identifier)
            .map_err(qs_err)
    }
}

#[pymethods]
impl BasisSwap {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        currency,
        pay_market_index,
        receive_market_index,
        pay_spread = None,
        receive_spread = None,
        side = None,
        pay_leg_frequency = None,
        receive_leg_frequency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        currency: &Bound<'_, PyAny>,
        pay_market_index: &Bound<'_, PyAny>,
        receive_market_index: &Bound<'_, PyAny>,
        pay_spread: Option<f64>,
        receive_spread: Option<f64>,
        side: Option<&Bound<'_, PyAny>>,
        pay_leg_frequency: Option<&Bound<'_, PyAny>>,
        receive_leg_frequency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            notional,
            currency: extract_currency(currency)?,
            pay_market_index: extract_market_index(pay_market_index)?,
            receive_market_index: extract_market_index(receive_market_index)?,
            pay_spread,
            receive_spread,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            pay_leg_frequency: pay_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Quarterly),
            receive_leg_frequency: receive_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Quarterly),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "BasisSwap(identifier='{}', start_date='{}', maturity_date='{}', notional={})",
            self.identifier, self.start_date, self.maturity_date, self.notional
        )
    }
}

/// A fixed-vs-float cross-currency swap.
#[pyclass(name = "FixFloatCrossCurrencySwap", from_py_object)]
#[derive(Clone)]
pub struct FixFloatCrossCurrencySwap {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub domestic_notional: f64,
    pub foreign_notional: f64,
    pub fixed_rate: f64,
    pub spread: Option<f64>,
    pub domestic_currency: Currency,
    pub foreign_currency: Currency,
    pub floating_index: MarketIndex,
    pub side: Side,
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub domestic_leg_frequency: Frequency,
    pub foreign_leg_frequency: Frequency,
    pub trade_date: Option<Date>,
}

macro_rules! build_fix_float_xccy_instrument {
    ($self:expr, $t:ty) => {{
        let mut mk = MakeFixFloatCrossCurrencySwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_domestic_notional($self.domestic_notional)
            .with_foreign_notional($self.foreign_notional)
            .with_fixed_rate($self.fixed_rate)
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_domestic_currency($self.domestic_currency)
            .with_foreign_currency($self.foreign_currency)
            .with_floating_index($self.floating_index.clone())
            .with_side($self.side)
            .with_domestic_leg_frequency($self.domestic_leg_frequency)
            .with_foreign_leg_frequency($self.foreign_leg_frequency);
        if let Some(spread) = $self.spread {
            mk = mk.with_spread(spread);
        }
        mk.build().map_err(qs_err)
    }};
}

impl FixFloatCrossCurrencySwap {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<FixFloatCrossCurrencySwapTrade<DualFwd>> {
        let instrument: QsFixFloatXccy<DualFwd> = build_fix_float_xccy_instrument!(self, DualFwd)?;
        Ok(FixFloatCrossCurrencySwapTrade::new(
            instrument,
            self.trade_date(),
            self.domestic_notional,
            self.foreign_notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFixFloatXccy<f64> = build_fix_float_xccy_instrument!(self, f64)?;
        instrument
            .legs()
            .to_vec()
            .into_contingent_claims(&self.identifier)
            .map_err(qs_err)
    }
}

#[pymethods]
impl FixFloatCrossCurrencySwap {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        domestic_notional,
        foreign_notional,
        fixed_rate,
        domestic_currency,
        foreign_currency,
        floating_index,
        spread = None,
        side = None,
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        domestic_leg_frequency = None,
        foreign_leg_frequency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        domestic_notional: f64,
        foreign_notional: f64,
        fixed_rate: f64,
        domestic_currency: &Bound<'_, PyAny>,
        foreign_currency: &Bound<'_, PyAny>,
        floating_index: &Bound<'_, PyAny>,
        spread: Option<f64>,
        side: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        domestic_leg_frequency: Option<&Bound<'_, PyAny>>,
        foreign_leg_frequency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            domestic_notional,
            foreign_notional,
            fixed_rate,
            spread,
            domestic_currency: extract_currency(domestic_currency)?,
            foreign_currency: extract_currency(foreign_currency)?,
            floating_index: extract_market_index(floating_index)?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            day_counter: day_counter
                .map(extract_day_counter)
                .transpose()?
                .unwrap_or(DayCounter::Actual360),
            compounding: compounding
                .map(extract_compounding)
                .transpose()?
                .unwrap_or(Compounding::Simple),
            rate_frequency: rate_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            domestic_leg_frequency: domestic_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            foreign_leg_frequency: foreign_leg_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "FixFloatCrossCurrencySwap(identifier='{}', start_date='{}', maturity_date='{}')",
            self.identifier, self.start_date, self.maturity_date
        )
    }
}

/// A fixed-rate bullet (or amortizing) bond.
#[pyclass(name = "FixedRateBond", from_py_object)]
#[derive(Clone)]
pub struct FixedRateBond {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub rate: f64,
    pub currency: Currency,
    pub discount_index: MarketIndex,
    pub side: Side,
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub payment_frequency: Frequency,
    pub payment_structure: PaymentStructure,
    pub trade_date: Option<Date>,
}

macro_rules! build_fixed_rate_bond_instrument {
    ($self:expr, $t:ty) => {{
        MakeFixedRateBond::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_rate($self.rate)
            .with_notional($self.notional)
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_discount_index($self.discount_index.clone())
            .with_currency($self.currency)
            .with_side($self.side)
            .with_payment_frequency($self.payment_frequency)
            .with_payment_structure($self.payment_structure)
            .build()
            .map_err(qs_err)
    }};
}

impl FixedRateBond {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<FixedRateBondTrade<DualFwd>> {
        let instrument: QsFixedRateBond<DualFwd> =
            build_fixed_rate_bond_instrument!(self, DualFwd)?;
        Ok(FixedRateBondTrade::new(
            instrument,
            self.trade_date(),
            self.notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFixedRateBond<f64> = build_fixed_rate_bond_instrument!(self, f64)?;
        instrument
            .legs()
            .to_vec()
            .into_contingent_claims(&self.identifier)
            .map_err(qs_err)
    }
}

#[pymethods]
impl FixedRateBond {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        rate,
        currency,
        discount_index,
        side = None,
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        payment_frequency = None,
        payment_structure = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        rate: f64,
        currency: &Bound<'_, PyAny>,
        discount_index: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        payment_structure: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            notional,
            rate,
            currency: extract_currency(currency)?,
            discount_index: extract_market_index(discount_index)?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            day_counter: day_counter
                .map(extract_day_counter)
                .transpose()?
                .unwrap_or(DayCounter::Actual360),
            compounding: compounding
                .map(extract_compounding)
                .transpose()?
                .unwrap_or(Compounding::Simple),
            rate_frequency: rate_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            payment_frequency: payment_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Semiannual),
            payment_structure: payment_structure
                .map(extract_payment_structure)
                .transpose()?
                .unwrap_or(PaymentStructure::Bullet),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedRateBond(identifier='{}', start_date='{}', maturity_date='{}', notional={}, rate={})",
            self.identifier, self.start_date, self.maturity_date, self.notional, self.rate
        )
    }
}

/// A floating-rate note.
#[pyclass(name = "FloatingRateNote", from_py_object)]
#[derive(Clone)]
pub struct FloatingRateNote {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub spread: f64,
    pub forward_index: MarketIndex,
    pub currency: Currency,
    pub side: Side,
    pub payment_frequency: Frequency,
    pub payment_structure: PaymentStructure,
    pub trade_date: Option<Date>,
}

macro_rules! build_frn_instrument {
    ($self:expr, $t:ty) => {{
        MakeFloatingRateNote::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_spread($self.spread)
            .with_notional($self.notional)
            .with_forward_index($self.forward_index.clone())
            .with_currency($self.currency)
            .with_side($self.side)
            .with_payment_frequency($self.payment_frequency)
            .with_payment_structure($self.payment_structure)
            .build()
            .map_err(qs_err)
    }};
}

impl FloatingRateNote {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<FloatingRateNoteTrade<DualFwd>> {
        let instrument: QsFloatingRateNote<DualFwd> = build_frn_instrument!(self, DualFwd)?;
        Ok(FloatingRateNoteTrade::new(
            instrument,
            self.trade_date(),
            self.notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFloatingRateNote<f64> = build_frn_instrument!(self, f64)?;
        instrument
            .legs()
            .to_vec()
            .into_contingent_claims(&self.identifier)
            .map_err(qs_err)
    }
}

#[pymethods]
impl FloatingRateNote {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        spread,
        currency,
        forward_index,
        side = None,
        payment_frequency = None,
        payment_structure = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        spread: f64,
        currency: &Bound<'_, PyAny>,
        forward_index: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        payment_structure: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            notional,
            spread,
            forward_index: extract_market_index(forward_index)?,
            currency: extract_currency(currency)?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            payment_frequency: payment_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Quarterly),
            payment_structure: payment_structure
                .map(extract_payment_structure)
                .transpose()?
                .unwrap_or(PaymentStructure::Bullet),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "FloatingRateNote(identifier='{}', start_date='{}', maturity_date='{}', notional={})",
            self.identifier, self.start_date, self.maturity_date, self.notional
        )
    }
}

/// A fixed-rate deposit.
#[pyclass(name = "FixedRateDeposit", from_py_object)]
#[derive(Clone)]
pub struct FixedRateDeposit {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub rate: f64,
    pub currency: Currency,
    pub discount_index: Option<MarketIndex>,
    pub side: Side,
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub trade_date: Option<Date>,
}

macro_rules! build_deposit_instrument {
    ($self:expr, $t:ty) => {{
        MakeFixedRateDeposit::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_rate($self.rate)
            .with_notional($self.notional)
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_discount_index($self.discount_index.clone())
            .with_currency($self.currency)
            .with_side($self.side)
            .build()
            .map_err(qs_err)
    }};
}

impl FixedRateDeposit {
    fn trade_date(&self) -> Date {
        self.trade_date.unwrap_or(self.start_date)
    }

    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self) -> PyResult<FixedRateDepositTrade<DualFwd>> {
        let instrument: QsFixedRateDeposit<DualFwd> = build_deposit_instrument!(self, DualFwd)?;
        Ok(FixedRateDepositTrade::new(
            instrument,
            self.trade_date(),
            self.notional,
            self.side,
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFixedRateDeposit<f64> = build_deposit_instrument!(self, f64)?;
        instrument
            .legs()
            .to_vec()
            .into_contingent_claims(&self.identifier)
            .map_err(qs_err)
    }
}

#[pymethods]
impl FixedRateDeposit {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        rate,
        currency,
        discount_index = None,
        side = None,
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        rate: f64,
        currency: &Bound<'_, PyAny>,
        discount_index: Option<&Bound<'_, PyAny>>,
        side: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            notional,
            rate,
            currency: extract_currency(currency)?,
            discount_index: discount_index.map(extract_market_index).transpose()?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            day_counter: day_counter
                .map(extract_day_counter)
                .transpose()?
                .unwrap_or(DayCounter::Actual360),
            compounding: compounding
                .map(extract_compounding)
                .transpose()?
                .unwrap_or(Compounding::Simple),
            rate_frequency: rate_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Annual),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedRateDeposit(identifier='{}', start_date='{}', maturity_date='{}', notional={}, rate={})",
            self.identifier, self.start_date, self.maturity_date, self.notional, self.rate
        )
    }
}

/// A deliverable or non-deliverable FX forward.
#[pyclass(name = "FxForward", from_py_object)]
#[derive(Clone)]
pub struct FxForwardPy {
    pub identifier: String,
    pub delivery_date: Date,
    pub base_currency: Currency,
    pub quote_currency: Currency,
    pub notional: f64,
    pub forward_price: Option<f64>,
    pub forward_points: Option<f64>,
    pub side: Side,
    pub day_counter: Option<DayCounter>,
    pub fixing_date: Option<Date>,
    pub settlement_currency: Option<Currency>,
    pub trade_date: Option<Date>,
}

impl FxForwardPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<FxForwardTrade> {
        let mut mk = MakeFxForward::default()
            .with_identifier(self.identifier.clone())
            .with_delivery_date(self.delivery_date)
            .with_base_currency(self.base_currency)
            .with_quote_currency(self.quote_currency)
            .with_side(self.side);
        if let Some(price) = self.forward_price {
            mk = mk.with_forward_price(price);
        }
        if let Some(points) = self.forward_points {
            mk = mk.with_forward_points(points);
        }
        if let Some(dc) = self.day_counter {
            mk = mk.with_day_counter(dc);
        }
        mk = match (self.fixing_date, self.settlement_currency) {
            (Some(fixing), Some(settlement)) => mk.as_ndf(fixing, settlement),
            _ => mk.as_deliverable(),
        };
        let instrument = mk.build().map_err(qs_err)?;
        Ok(FxForwardTrade::new(
            instrument,
            self.trade_date.unwrap_or(self.delivery_date),
            self.notional,
            self.side,
        ))
    }
}

#[pymethods]
impl FxForwardPy {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        delivery_date,
        base_currency,
        quote_currency,
        notional,
        forward_price = None,
        forward_points = None,
        side = None,
        day_counter = None,
        fixing_date = None,
        settlement_currency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        delivery_date: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        quote_currency: &Bound<'_, PyAny>,
        notional: f64,
        forward_price: Option<f64>,
        forward_points: Option<f64>,
        side: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        fixing_date: Option<&Bound<'_, PyAny>>,
        settlement_currency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            delivery_date: extract_date(delivery_date)?,
            base_currency: extract_currency(base_currency)?,
            quote_currency: extract_currency(quote_currency)?,
            notional,
            forward_price,
            forward_points,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            day_counter: day_counter.map(extract_day_counter).transpose()?,
            fixing_date: fixing_date.map(extract_date).transpose()?,
            settlement_currency: settlement_currency.map(extract_currency).transpose()?,
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "FxForward(identifier='{}', delivery_date='{}', notional={})",
            self.identifier, self.delivery_date, self.notional
        )
    }
}

/// A European FX option.
#[pyclass(name = "FxOption", from_py_object)]
#[derive(Clone)]
pub struct FxOptionPy {
    pub identifier: String,
    pub expiry_date: Date,
    pub strike: f64,
    pub option_type: EuroOptionType,
    pub base_currency: Currency,
    pub quote_currency: Currency,
    pub notional: f64,
    pub side: Side,
    pub trade_date: Option<Date>,
}

impl FxOptionPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<FxEuropeanOptionTrade> {
        let instrument = MakeFxEuropeanOption::default()
            .with_identifier(self.identifier.clone())
            .with_expiry_date(self.expiry_date)
            .with_strike(self.strike)
            .with_option_type(self.option_type)
            .with_base_currency(self.base_currency)
            .with_quote_currency(self.quote_currency)
            .build()
            .map_err(qs_err)?;
        Ok(FxEuropeanOptionTrade::new(
            instrument,
            self.trade_date.unwrap_or(self.expiry_date),
            self.notional,
            self.side,
        ))
    }
}

#[pymethods]
impl FxOptionPy {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        expiry_date,
        strike,
        option_type,
        base_currency,
        quote_currency,
        notional,
        side = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        expiry_date: &Bound<'_, PyAny>,
        strike: f64,
        option_type: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        quote_currency: &Bound<'_, PyAny>,
        notional: f64,
        side: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            expiry_date: extract_date(expiry_date)?,
            strike,
            option_type: extract_option_type(option_type)?,
            base_currency: extract_currency(base_currency)?,
            quote_currency: extract_currency(quote_currency)?,
            notional,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "FxOption(identifier='{}', expiry_date='{}', strike={}, notional={})",
            self.identifier, self.expiry_date, self.strike, self.notional
        )
    }
}

/// A European equity option.
#[pyclass(name = "EquityOption", from_py_object)]
#[derive(Clone)]
pub struct EquityOption {
    pub identifier: String,
    pub market_index: MarketIndex,
    pub expiry_date: Date,
    pub strike: Strike,
    pub option_type: EuroOptionType,
    pub notional: f64,
    pub side: Side,
    pub trade_date: Option<Date>,
}

impl EquityOption {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<EquityEuropeanOptionTrade> {
        let instrument = QsEquityEuropeanOption::new(
            self.market_index.clone(),
            self.expiry_date,
            self.strike,
            self.option_type.clone(),
            self.identifier.clone(),
        );
        Ok(EquityEuropeanOptionTrade::new(
            instrument,
            self.notional,
            self.trade_date.unwrap_or(self.expiry_date),
            self.side,
        ))
    }
}

#[pymethods]
impl EquityOption {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        market_index,
        expiry_date,
        strike,
        option_type,
        notional,
        side = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        market_index: &Bound<'_, PyAny>,
        expiry_date: &Bound<'_, PyAny>,
        strike: &Bound<'_, PyAny>,
        option_type: &Bound<'_, PyAny>,
        notional: f64,
        side: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            market_index: extract_market_index(market_index)?,
            expiry_date: extract_date(expiry_date)?,
            strike: extract_strike(strike)?,
            option_type: extract_option_type(option_type)?,
            notional,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "EquityOption(identifier='{}', expiry_date='{}', notional={})",
            self.identifier, self.expiry_date, self.notional
        )
    }
}

/// A single-name credit default swap.
#[pyclass(name = "CreditDefaultSwap", from_py_object)]
#[derive(Clone)]
pub struct CreditDefaultSwapPy {
    pub identifier: String,
    pub credit_index: MarketIndex,
    pub discount_index: MarketIndex,
    pub currency: Currency,
    pub start_date: Date,
    pub maturity_date: Date,
    pub spread: f64,
    pub recovery: f64,
    pub notional: f64,
    pub premium_frequency: Frequency,
    pub day_counter: DayCounter,
    pub side: Side,
    pub trade_date: Option<Date>,
}

impl CreditDefaultSwapPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<CdsTrade> {
        let instrument = QsCreditDefaultSwap::new(
            self.identifier.clone(),
            self.credit_index.clone(),
            self.discount_index.clone(),
            self.currency,
            self.start_date,
            self.maturity_date,
            self.spread,
            self.recovery,
            self.premium_frequency,
            self.day_counter,
        )
        .map_err(qs_err)?;
        Ok(CdsTrade::new(
            instrument,
            self.trade_date.unwrap_or(self.start_date),
            self.notional,
            self.side,
        ))
    }
}

#[pymethods]
impl CreditDefaultSwapPy {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        spread,
        currency,
        credit_index,
        discount_index,
        recovery = 0.4,
        premium_frequency = None,
        day_counter = None,
        side = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        spread: f64,
        currency: &Bound<'_, PyAny>,
        credit_index: &Bound<'_, PyAny>,
        discount_index: &Bound<'_, PyAny>,
        recovery: f64,
        premium_frequency: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        side: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let credit_index = extract_market_index(credit_index)?;
        let credit_index = if matches!(credit_index, MarketIndex::Credit(_)) {
            credit_index
        } else {
            // Allow passing a plain name or non-credit index; coerce to Credit.
            MarketIndex::Credit(credit_index.to_string())
        };
        Ok(Self {
            identifier,
            credit_index,
            discount_index: extract_market_index(discount_index)?,
            currency: extract_currency(currency)?,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            spread,
            recovery,
            notional,
            premium_frequency: premium_frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Quarterly),
            day_counter: day_counter
                .map(extract_day_counter)
                .transpose()?
                .unwrap_or(DayCounter::Actual360),
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditDefaultSwap(identifier='{}', start_date='{}', maturity_date='{}', spread={}, notional={})",
            self.identifier, self.start_date, self.maturity_date, self.spread, self.notional
        )
    }
}

/// An interest-rate cap or floor.
#[pyclass(name = "CapFloor", from_py_object)]
#[derive(Clone)]
pub struct CapFloorPy {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub strike: f64,
    pub notional: f64,
    pub market_index: MarketIndex,
    pub currency: Currency,
    pub cap_floor_type: CapFloorType,
    pub side: Side,
    pub frequency: Frequency,
    pub trade_date: Option<Date>,
}

impl CapFloorPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<CapFloorTrade> {
        let instrument: QsCapFloor = MakeCapFloor::default()
            .with_identifier(self.identifier.clone())
            .with_start_date(self.start_date)
            .with_maturity_date(self.maturity_date)
            .with_strike(self.strike)
            .with_notional(self.notional)
            .with_market_index(self.market_index.clone())
            .with_currency(self.currency)
            .with_side(self.side)
            .with_cap_floor_type(self.cap_floor_type)
            .with_frequency(self.frequency)
            .build()
            .map_err(qs_err)?;
        Ok(CapFloorTrade::new(
            instrument,
            self.trade_date.unwrap_or(self.start_date),
            self.notional,
            self.side,
        ))
    }
}

#[pymethods]
impl CapFloorPy {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        strike,
        notional,
        currency,
        market_index,
        cap_floor_type,
        side = None,
        frequency = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        strike: f64,
        notional: f64,
        currency: &Bound<'_, PyAny>,
        market_index: &Bound<'_, PyAny>,
        cap_floor_type: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            start_date: extract_date(start_date)?,
            maturity_date: extract_date(maturity_date)?,
            strike,
            notional,
            market_index: extract_market_index(market_index)?,
            currency: extract_currency(currency)?,
            cap_floor_type: extract_cap_floor_type(cap_floor_type)?,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            frequency: frequency
                .map(extract_frequency)
                .transpose()?
                .unwrap_or(Frequency::Quarterly),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "CapFloor(identifier='{}', start_date='{}', maturity_date='{}', strike={}, notional={})",
            self.identifier, self.start_date, self.maturity_date, self.strike, self.notional
        )
    }
}

/// A single caplet or floorlet.
#[pyclass(name = "CapletFloorlet", from_py_object)]
#[derive(Clone)]
pub struct CapletFloorletPy {
    pub identifier: String,
    pub market_index: MarketIndex,
    pub currency: Currency,
    pub fixing_date: Date,
    pub start_accrual_date: Date,
    pub end_accrual_date: Date,
    pub payment_date: Date,
    pub payoff_type: CapletFloorletType,
    pub strike: Strike,
    pub notional: f64,
    pub side: Side,
    pub trade_date: Option<Date>,
}

impl CapletFloorletPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<CapletFloorletTrade> {
        let instrument = QsCapletFloorlet::new(
            self.identifier.clone(),
            self.market_index.clone(),
            self.currency,
            self.fixing_date,
            self.start_accrual_date,
            self.end_accrual_date,
            self.payment_date,
            self.payoff_type,
            self.strike,
        );
        Ok(CapletFloorletTrade::new(
            instrument,
            self.trade_date.unwrap_or(self.fixing_date),
            self.notional,
            self.side,
        ))
    }
}

#[pymethods]
impl CapletFloorletPy {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        fixing_date,
        start_accrual_date,
        end_accrual_date,
        payment_date,
        strike,
        notional,
        currency,
        market_index,
        payoff_type,
        side = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        fixing_date: &Bound<'_, PyAny>,
        start_accrual_date: &Bound<'_, PyAny>,
        end_accrual_date: &Bound<'_, PyAny>,
        payment_date: &Bound<'_, PyAny>,
        strike: &Bound<'_, PyAny>,
        notional: f64,
        currency: &Bound<'_, PyAny>,
        market_index: &Bound<'_, PyAny>,
        payoff_type: &Bound<'_, PyAny>,
        side: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            market_index: extract_market_index(market_index)?,
            currency: extract_currency(currency)?,
            fixing_date: extract_date(fixing_date)?,
            start_accrual_date: extract_date(start_accrual_date)?,
            end_accrual_date: extract_date(end_accrual_date)?,
            payment_date: extract_date(payment_date)?,
            payoff_type: extract_caplet_floorlet_type(payoff_type)?,
            strike: extract_strike(strike)?,
            notional,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "CapletFloorlet(identifier='{}', fixing_date='{}', notional={})",
            self.identifier, self.fixing_date, self.notional
        )
    }
}

/// An exchange-traded interest-rate futures position.
#[pyclass(name = "RateFutures", from_py_object)]
#[derive(Clone)]
pub struct RateFuturesPy {
    pub identifier: String,
    pub market_index: MarketIndex,
    pub start_date: Date,
    pub end_date: Date,
    pub futures_price: f64,
    pub contract_size: Option<f64>,
    pub num_contracts: f64,
    pub side: Side,
    pub trade_date: Option<Date>,
}

impl RateFuturesPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self) -> PyResult<RateFuturesTrade> {
        let mut mk = MakeRateFutures::default()
            .with_identifier(self.identifier.clone())
            .with_market_index(self.market_index.clone())
            .with_start_date(self.start_date)
            .with_end_date(self.end_date)
            .with_futures_price(self.futures_price)
            .with_side(self.side);
        if let Some(size) = self.contract_size {
            mk = mk.with_contract_size(size);
        }
        let instrument: QsRateFutures = mk.build().map_err(qs_err)?;
        Ok(RateFuturesTrade::new(
            instrument,
            self.trade_date.unwrap_or(self.start_date),
            self.num_contracts,
            self.side,
        ))
    }
}

#[pymethods]
impl RateFuturesPy {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        end_date,
        futures_price,
        market_index,
        num_contracts = 1.0,
        contract_size = None,
        side = None,
        trade_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        end_date: &Bound<'_, PyAny>,
        futures_price: f64,
        market_index: &Bound<'_, PyAny>,
        num_contracts: f64,
        contract_size: Option<f64>,
        side: Option<&Bound<'_, PyAny>>,
        trade_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            identifier,
            market_index: extract_market_index(market_index)?,
            start_date: extract_date(start_date)?,
            end_date: extract_date(end_date)?,
            futures_price,
            contract_size,
            num_contracts,
            side: side
                .map(extract_side)
                .transpose()?
                .unwrap_or(Side::LongReceive),
            trade_date: trade_date.map(extract_date).transpose()?,
        })
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn __repr__(&self) -> String {
        format!(
            "RateFutures(identifier='{}', start_date='{}', end_date='{}', futures_price={})",
            self.identifier, self.start_date, self.end_date, self.futures_price
        )
    }
}
