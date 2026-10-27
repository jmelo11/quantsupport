//! Python instrument specifications, built with explicit position metadata.

use crate::{
    common::conversion::{
        extract_business_day_convention, extract_calendar, extract_cap_floor_type,
        extract_caplet_floorlet_type, extract_compounding, extract_currency, extract_date,
        extract_date_generation_rule, extract_day_counter, extract_frequency, extract_market_index,
        extract_option_type, extract_payment_structure, extract_strike, from_py, qs_err,
    },
    instruments::coupons::CouponDefinition,
    pricing::extension::{Instrument, Trade},
};
use pyo3::{prelude::*, types::PyType};
use quantsupport::prelude::{
    BasisSwap as QsBasisSwap, BasisSwapTrade, BusinessDayConvention, Calendar,
    CallScheduleEntry as QsCallScheduleEntry, CapFloor as QsCapFloor, CapFloorTrade, CapFloorType,
    CapletFloorlet as QsCapletFloorlet, CapletFloorletTrade, CapletFloorletType, CdsTrade,
    Compounding, ContingentClaim, CreditDefaultSwap as QsCreditDefaultSwap, Currency, Date,
    DateGenerationRule, DayCounter, DualFwd, EquityEuropeanOption as QsEquityEuropeanOption,
    EquityEuropeanOptionTrade, EuroOptionType, FixFloatCrossCurrencySwap as QsFixFloatXccy,
    FixFloatCrossCurrencySwapTrade, FixedRateBond as QsFixedRateBond, FixedRateBondTrade,
    FixedRateDeposit as QsFixedRateDeposit, FixedRateDepositTrade, FloatFloatCrossCurrencySwap,
    FloatFloatCrossCurrencySwapTrade, FloatingRateNote as QsFloatingRateNote,
    FloatingRateNoteTrade, Frequency, FxEuropeanOptionTrade, FxForwardTrade, IntoContingentClaims,
    LegsProvider, MakeBasisSwap, MakeCapFloor, MakeFixFloatCrossCurrencySwap, MakeFixedRateBond,
    MakeFixedRateDeposit, MakeFloatFloatCrossCurrencySwap, MakeFloatingRateNote,
    MakeFxEuropeanOption, MakeFxForward, MakeInstrument, MakeRateFutures, MakeSteppedCouponBond,
    MakeSwap, MarketIndex, PaymentStructure, RateDefinition, RateFutures as QsRateFutures,
    RateFuturesTrade, SteppedCouponBondTrade, Strike, Swap as QsSwap, SwapTrade,
};

macro_rules! product_dict_constructors {
    ($($name:ty),+ $(,)?) => {
        $(
            #[pymethods]
            impl $name {
                /// Builds a product using the same fields and defaults as its constructor.
                #[classmethod]
                fn from_dict<'py>(
                    cls: &Bound<'py, PyType>,
                    obj: &Bound<'py, PyAny>,
                ) -> PyResult<Bound<'py, PyAny>> {
                    cls.call((), Some(crate::common::conversion::expect_dict(obj, &format!("{}.from_dict", cls.name()?))?))
                }


            }
        )+
    };
}

product_dict_constructors!(
    EuropeanSwaption,
    EquityForward,
    Swap,
    CrossCurrencySwap,
    BasisSwap,
    FixFloatCrossCurrencySwap,
    FixedRateBond,
    SteppedCouponBond,
    FloatingRateNote,
    FixedRateDeposit,
    FxForwardPy,
    FxOptionPy,
    EquityOption,
    CreditDefaultSwapPy,
    CapFloorPy,
    CapletFloorletPy,
    RateFuturesPy,
);

/// A vanilla fixed-vs-floating interest rate swap.
#[pyclass(module = "quantsupport", name = "Swap", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub fixed_leg_frequency: Frequency,
    pub floating_leg_frequency: Frequency,
}

macro_rules! build_swap_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        let mut mk = MakeSwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_fixed_rate($self.fixed_rate)
            .with_notional($position.notional().unwrap_or($self.notional))
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_currency($self.currency)
            .with_market_index($self.market_index.clone())
            .with_side($position.side())
            .with_fixed_leg_frequency($self.fixed_leg_frequency)
            .with_floating_leg_frequency($self.floating_leg_frequency);
        if let Some(spread) = $self.spread {
            mk = mk.with_spread(spread);
        }
        mk.build().map_err(qs_err)
    }};
}

impl Swap {
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self, position: &Trade) -> PyResult<SwapTrade<DualFwd>> {
        let instrument: QsSwap<DualFwd> = build_swap_instrument!(self, position, DualFwd)?;
        Ok(SwapTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsSwap<f64> = build_swap_instrument!(self, position, f64)?;
        let trade = SwapTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
        );
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
        spread = None,
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        fixed_leg_frequency = None,
        floating_leg_frequency = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        fixed_rate: f64,
        currency: &Bound<'_, PyAny>,
        market_index: &Bound<'_, PyAny>,
        spread: Option<f64>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        fixed_leg_frequency: Option<&Bound<'_, PyAny>>,
        floating_leg_frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                notional,
                fixed_rate,
                spread,
                currency: extract_currency(currency)?,
                market_index: extract_market_index(market_index)?,
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
            }),
        )
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
    fn currency_py(&self) -> crate::common::enums::Currency {
        self.currency.into()
    }

    #[getter(market_index)]
    fn market_index_py(&self) -> crate::common::enums::MarketIndex {
        crate::common::enums::MarketIndex {
            inner: self.market_index.clone(),
        }
    }
}

/// A float-vs-float cross-currency swap.
#[pyclass(module = "quantsupport", name = "CrossCurrencySwap", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
    pub domestic_leg_frequency: Frequency,
    pub foreign_leg_frequency: Frequency,
}

macro_rules! build_xccy_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        MakeFloatFloatCrossCurrencySwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_domestic_notional(
                $position
                    .domestic_notional()
                    .unwrap_or($self.domestic_notional),
            )
            .with_foreign_notional(
                $position
                    .foreign_notional()
                    .unwrap_or($self.foreign_notional),
            )
            .with_domestic_spread($self.domestic_spread)
            .with_foreign_spread($self.foreign_spread)
            .with_domestic_currency($self.domestic_currency)
            .with_foreign_currency($self.foreign_currency)
            .with_domestic_market_index($self.domestic_market_index.clone())
            .with_foreign_market_index($self.foreign_market_index.clone())
            .with_side($position.side())
            .with_domestic_leg_frequency($self.domestic_leg_frequency)
            .with_foreign_leg_frequency($self.foreign_leg_frequency)
            .build()
            .map_err(qs_err)
    }};
}

impl CrossCurrencySwap {
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(
        &self,
        position: &Trade,
    ) -> PyResult<FloatFloatCrossCurrencySwapTrade<DualFwd>> {
        let instrument: FloatFloatCrossCurrencySwap<DualFwd> =
            build_xccy_instrument!(self, position, DualFwd)?;
        Ok(FloatFloatCrossCurrencySwapTrade::new(
            instrument,
            position.trade_date()?,
            position
                .domestic_notional()
                .unwrap_or(self.domestic_notional),
            position.foreign_notional().unwrap_or(self.foreign_notional),
            position.side(),
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: FloatFloatCrossCurrencySwap<f64> =
            build_xccy_instrument!(self, position, f64)?;
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
        domestic_spread = 0.0,
        foreign_spread = 0.0,
        domestic_leg_frequency = None,
        foreign_leg_frequency = None,
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
        domestic_spread: f64,
        foreign_spread: f64,
        domestic_leg_frequency: Option<&Bound<'_, PyAny>>,
        foreign_leg_frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
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
                domestic_leg_frequency: domestic_leg_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Semiannual),
                foreign_leg_frequency: foreign_leg_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Semiannual),
            }),
        )
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
    fn domestic_currency_py(&self) -> crate::common::enums::Currency {
        self.domestic_currency.into()
    }

    #[getter(foreign_currency)]
    fn foreign_currency_py(&self) -> crate::common::enums::Currency {
        self.foreign_currency.into()
    }

    #[getter(domestic_market_index)]
    fn domestic_market_index_py(&self) -> crate::common::enums::MarketIndex {
        crate::common::enums::MarketIndex {
            inner: self.domestic_market_index.clone(),
        }
    }

    #[getter(foreign_market_index)]
    fn foreign_market_index_py(&self) -> crate::common::enums::MarketIndex {
        crate::common::enums::MarketIndex {
            inner: self.foreign_market_index.clone(),
        }
    }
}

/// A float-vs-float single-currency basis swap.
#[pyclass(module = "quantsupport", name = "BasisSwap", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
    pub pay_leg_frequency: Frequency,
    pub receive_leg_frequency: Frequency,
}

macro_rules! build_basis_swap_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        let mut mk = MakeBasisSwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_notional($position.notional().unwrap_or($self.notional))
            .with_currency($self.currency)
            .with_pay_market_index($self.pay_market_index.clone())
            .with_receive_market_index($self.receive_market_index.clone())
            .with_side($position.side())
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
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self, position: &Trade) -> PyResult<BasisSwapTrade<DualFwd>> {
        let instrument: QsBasisSwap<DualFwd> =
            build_basis_swap_instrument!(self, position, DualFwd)?;
        Ok(BasisSwapTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsBasisSwap<f64> = build_basis_swap_instrument!(self, position, f64)?;
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
        pay_leg_frequency = None,
        receive_leg_frequency = None,
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
        pay_leg_frequency: Option<&Bound<'_, PyAny>>,
        receive_leg_frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                notional,
                currency: extract_currency(currency)?,
                pay_market_index: extract_market_index(pay_market_index)?,
                receive_market_index: extract_market_index(receive_market_index)?,
                pay_spread,
                receive_spread,
                pay_leg_frequency: pay_leg_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Quarterly),
                receive_leg_frequency: receive_leg_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Quarterly),
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A fixed-vs-float cross-currency swap.
#[pyclass(module = "quantsupport", name = "FixFloatCrossCurrencySwap", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub domestic_leg_frequency: Frequency,
    pub foreign_leg_frequency: Frequency,
}

macro_rules! build_fix_float_xccy_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        let mut mk = MakeFixFloatCrossCurrencySwap::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_domestic_notional(
                $position
                    .domestic_notional()
                    .unwrap_or($self.domestic_notional),
            )
            .with_foreign_notional(
                $position
                    .foreign_notional()
                    .unwrap_or($self.foreign_notional),
            )
            .with_fixed_rate($self.fixed_rate)
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_domestic_currency($self.domestic_currency)
            .with_foreign_currency($self.foreign_currency)
            .with_floating_index($self.floating_index.clone())
            .with_side($position.side())
            .with_domestic_leg_frequency($self.domestic_leg_frequency)
            .with_foreign_leg_frequency($self.foreign_leg_frequency);
        if let Some(spread) = $self.spread {
            mk = mk.with_spread(spread);
        }
        mk.build().map_err(qs_err)
    }};
}

impl FixFloatCrossCurrencySwap {
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(
        &self,
        position: &Trade,
    ) -> PyResult<FixFloatCrossCurrencySwapTrade<DualFwd>> {
        let instrument: QsFixFloatXccy<DualFwd> =
            build_fix_float_xccy_instrument!(self, position, DualFwd)?;
        Ok(FixFloatCrossCurrencySwapTrade::new(
            instrument,
            position.trade_date()?,
            position
                .domestic_notional()
                .unwrap_or(self.domestic_notional),
            position.foreign_notional().unwrap_or(self.foreign_notional),
            position.side(),
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFixFloatXccy<f64> =
            build_fix_float_xccy_instrument!(self, position, f64)?;
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
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        domestic_leg_frequency = None,
        foreign_leg_frequency = None,
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
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        domestic_leg_frequency: Option<&Bound<'_, PyAny>>,
        foreign_leg_frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                domestic_notional,
                foreign_notional,
                fixed_rate,
                spread,
                domestic_currency: extract_currency(domestic_currency)?,
                foreign_currency: extract_currency(foreign_currency)?,
                floating_index: extract_market_index(floating_index)?,
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
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// One dated issuer call in a callable bond's schedule.
#[pyclass(
    module = "quantsupport",
    name = "CallScheduleEntry",
    from_py_object,
    frozen
)]
#[derive(Clone)]
pub struct CallScheduleEntry {
    pub inner: QsCallScheduleEntry,
}

#[pymethods]
impl CallScheduleEntry {
    #[new]
    #[pyo3(signature = (date = None, strike = 100.0, start_date = None, end_date = None))]
    fn new(
        date: Option<&Bound<'_, PyAny>>,
        strike: f64,
        start_date: Option<&Bound<'_, PyAny>>,
        end_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        if date.is_some() && start_date.is_some() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "specify date or start_date, not both",
            ));
        }
        let start = date.or(start_date).ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("date or start_date is required")
        })?;
        let start = extract_date(start)?;
        let inner = match end_date {
            Some(end) => QsCallScheduleEntry::window(start, extract_date(end)?, strike),
            None => QsCallScheduleEntry::new(start, strike),
        };
        inner.validate().map_err(qs_err)?;
        Ok(Self { inner })
    }

    /// Builds a validated native call entry from a dict.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let inner: QsCallScheduleEntry = from_py(obj, "call schedule entry")?;
        inner.validate().map_err(qs_err)?;
        Ok(Self { inner })
    }

    /// Creates an inclusive issuer-call window.
    #[staticmethod]
    fn window(
        start_date: &Bound<'_, PyAny>,
        end_date: &Bound<'_, PyAny>,
        strike: f64,
    ) -> PyResult<Self> {
        Self::new(None, strike, Some(start_date), Some(end_date))
    }

    /// Returns the native JSON-compatible schema.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &self.inner).map_err(qs_err)
    }

    /// Inclusive first exercise date.
    #[getter]
    fn start_date(&self) -> crate::time::Date {
        self.date()
    }

    /// Inclusive final exercise date.
    #[getter]
    fn end_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.inner.end_date(),
        }
    }

    /// Whether a date is inside the inclusive exercise window.
    fn contains(&self, date: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.contains(extract_date(date)?))
    }

    /// Issuer call date.
    #[getter]
    fn date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.inner.date(),
        }
    }

    /// Call strike per bond units, excluding same-day coupon and amortization.
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike()
    }
}

/// A fixed-rate bullet (or amortizing) bond.
#[pyclass(module = "quantsupport", name = "FixedRateBond", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct FixedRateBond {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub rate: f64,
    pub currency: Currency,
    pub discount_index: Option<MarketIndex>,
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
    pub payment_frequency: Frequency,
    pub payment_structure: PaymentStructure,
    pub calendar: Option<Calendar>,
    pub business_day_convention: Option<BusinessDayConvention>,
    pub date_generation_rule: Option<DateGenerationRule>,
    pub end_of_month: Option<bool>,
    pub first_coupon_date: Option<Date>,
}

macro_rules! build_fixed_rate_bond_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        let mut builder = MakeFixedRateBond::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_rate($self.rate)
            .with_notional($position.notional().unwrap_or($self.notional))
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_currency($self.currency)
            .with_payment_frequency($self.payment_frequency)
            .with_payment_structure($self.payment_structure);
        if let Some(index) = &$self.discount_index {
            builder = builder.with_discount_index(index.clone());
        }
        if let Some(calendar) = &$self.calendar {
            builder = builder.with_calendar(calendar.clone());
        }
        if let Some(convention) = $self.business_day_convention {
            builder = builder.with_business_day_convention(convention);
        }
        if let Some(rule) = $self.date_generation_rule {
            builder = builder.with_date_generation_rule(rule);
        }
        if let Some(enabled) = $self.end_of_month {
            builder = builder.with_end_of_month(enabled);
        }
        if let Some(date) = $self.first_coupon_date {
            builder = builder.with_first_coupon_date(date);
        }
        builder.build().map_err(qs_err)
    }};
}

impl FixedRateBond {
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self, position: &Trade) -> PyResult<FixedRateBondTrade<DualFwd>> {
        let instrument: QsFixedRateBond<DualFwd> =
            build_fixed_rate_bond_instrument!(self, position, DualFwd)?;
        let mut trade = FixedRateBondTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
            position.bond_anchor()?,
        );
        if let Some(date) = position.settlement_date() {
            trade = trade.with_settlement_date(date);
        }
        if let Some(day_counter) = position.spread_day_counter() {
            trade = trade.with_spread_day_counter(day_counter);
        }
        Ok(trade)
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFixedRateBond<f64> =
            build_fixed_rate_bond_instrument!(self, position, f64)?;
        FixedRateBondTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
            position.bond_anchor()?,
        )
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
        discount_index = None,
        day_counter = None,
        compounding = None,
        rate_frequency = None,
        payment_frequency = None,
        payment_structure = None,
        calendar = None,
        business_day_convention = None,
        date_generation_rule = None,
        end_of_month = None,
        first_coupon_date = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        rate: f64,
        currency: &Bound<'_, PyAny>,
        discount_index: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        payment_structure: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        date_generation_rule: Option<&Bound<'_, PyAny>>,
        end_of_month: Option<bool>,
        first_coupon_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                notional,
                rate,
                currency: extract_currency(currency)?,
                discount_index: discount_index.map(extract_market_index).transpose()?,
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
                calendar: calendar.map(extract_calendar).transpose()?,
                business_day_convention: business_day_convention
                    .map(extract_business_day_convention)
                    .transpose()?,
                date_generation_rule: date_generation_rule
                    .map(extract_date_generation_rule)
                    .transpose()?,
                end_of_month,
                first_coupon_date: first_coupon_date.map(extract_date).transpose()?,
            }),
        )
    }

    /// Serializes bond contract terms for constructor round trips.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &serde_json::json!({
            "identifier": self.identifier, "start_date": self.start_date, "maturity_date": self.maturity_date,
            "notional": self.notional, "rate": self.rate, "currency": self.currency, "discount_index": self.discount_index,
            "day_counter": self.day_counter, "compounding": self.compounding,
            "rate_frequency": self.rate_frequency, "payment_frequency": self.payment_frequency,
            "payment_structure": format!("{:?}", crate::common::enums::PaymentStructure::from(self.payment_structure)),
            "calendar": self.calendar, "business_day_convention": self.business_day_convention, "date_generation_rule": self.date_generation_rule,
            "end_of_month": self.end_of_month, "first_coupon_date": self.first_coupon_date
        })).map_err(qs_err)
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A callable bond with positional coupon and outstanding-notional schedules.
#[pyclass(module = "quantsupport", name = "SteppedCouponBond", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct SteppedCouponBond {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub units: f64,
    pub currency: Currency,
    pub payment_frequency: Frequency,
    pub coupon_day_counter: DayCounter,
    pub coupons: Vec<CouponDefinition>,
    pub notional_percentages: Vec<f64>,
    pub call_schedule: Vec<CallScheduleEntry>,
    pub calendar: Option<Calendar>,
    pub business_day_convention: Option<BusinessDayConvention>,
    pub date_generation_rule: Option<DateGenerationRule>,
    pub end_of_month: Option<bool>,
    pub first_coupon_date: Option<Date>,
    pub next_to_last_coupon_date: Option<Date>,
    pub termination_date_convention: Option<BusinessDayConvention>,
}

impl SteppedCouponBond {
    /// Builds the AD-enabled native trade consumed by the callable engines.
    pub fn build_trade_dual(&self, position: &Trade) -> PyResult<SteppedCouponBondTrade<DualFwd>> {
        let mut builder = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier(self.identifier.clone())
            .with_start_date(self.start_date)
            .with_maturity_date(self.maturity_date)
            .with_notional(position.notional().unwrap_or(self.notional))
            .with_units(self.units)
            .with_currency(self.currency)
            .with_payment_frequency(self.payment_frequency)
            .with_coupon_day_counter(self.coupon_day_counter)
            .with_coupons(
                self.coupons
                    .iter()
                    .map(|coupon| coupon.inner.clone().into())
                    .collect(),
            )
            .with_notional_percentages(self.notional_percentages.clone())
            .with_call_schedule(self.call_schedule.iter().map(|call| call.inner).collect());
        if let Some(calendar) = &self.calendar {
            builder = builder.with_calendar(calendar.clone());
        }
        if let Some(convention) = self.business_day_convention {
            builder = builder.with_business_day_convention(convention);
        }
        if let Some(rule) = self.date_generation_rule {
            builder = builder.with_date_generation_rule(rule);
        }
        if let Some(enabled) = self.end_of_month {
            builder = builder.with_end_of_month(enabled);
        }
        if let Some(date) = self.first_coupon_date {
            builder = builder.with_first_coupon_date(date);
        }
        if let Some(date) = self.next_to_last_coupon_date {
            builder = builder.with_next_to_last_coupon_date(date);
        }
        if let Some(convention) = self.termination_date_convention {
            builder = builder.with_termination_date_convention(convention);
        }
        let instrument = builder.build().map_err(qs_err)?;
        let mut trade = SteppedCouponBondTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
            position.bond_anchor()?,
        );
        if let Some(date) = position.settlement_date() {
            trade = trade.with_settlement_date(date);
        }
        if let Some(day_counter) = position.spread_day_counter() {
            trade = trade.with_spread_day_counter(day_counter);
        }
        Ok(trade)
    }
}

#[pymethods]
impl SteppedCouponBond {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        identifier,
        start_date,
        maturity_date,
        notional,
        currency,
        coupons,
        notional_percentages,
        call_schedule = None,
        units = 100.0,
        payment_frequency = None,
        coupon_day_counter = None,
        calendar = None,
        business_day_convention = None,
        date_generation_rule = None,
        end_of_month = None,
        first_coupon_date = None,
        next_to_last_coupon_date = None,
        termination_date_convention = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        currency: &Bound<'_, PyAny>,
        coupons: &Bound<'_, PyAny>,
        notional_percentages: Vec<f64>,
        call_schedule: Option<Vec<Bound<'_, PyAny>>>,
        units: f64,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        coupon_day_counter: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        date_generation_rule: Option<&Bound<'_, PyAny>>,
        end_of_month: Option<bool>,
        first_coupon_date: Option<&Bound<'_, PyAny>>,
        next_to_last_coupon_date: Option<&Bound<'_, PyAny>>,
        termination_date_convention: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let coupons = CouponDefinition::list_from_py(coupons, "SteppedCouponBond.coupons")?;
        if coupons.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "coupons cannot be empty",
            ));
        }
        if notional_percentages.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "notional_percentages cannot be empty",
            ));
        }
        let call_schedule = call_schedule
            .unwrap_or_default()
            .iter()
            .map(|obj| {
                obj.extract::<CallScheduleEntry>()
                    .or_else(|_| CallScheduleEntry::from_dict(obj))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                notional,
                units,
                currency: extract_currency(currency)?,
                payment_frequency: payment_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Semiannual),
                coupon_day_counter: coupon_day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual365),
                coupons,
                notional_percentages,
                call_schedule,
                calendar: calendar.map(extract_calendar).transpose()?,
                business_day_convention: business_day_convention
                    .map(extract_business_day_convention)
                    .transpose()?,
                date_generation_rule: date_generation_rule
                    .map(extract_date_generation_rule)
                    .transpose()?,
                end_of_month,
                first_coupon_date: first_coupon_date.map(extract_date).transpose()?,
                next_to_last_coupon_date: next_to_last_coupon_date.map(extract_date).transpose()?,
                termination_date_convention: termination_date_convention
                    .map(extract_business_day_convention)
                    .transpose()?,
            }),
        )
    }

    /// Returns constructor fields with native nested coupon and call schemas.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = serde_json::json!({
            "identifier": self.identifier,
            "start_date": self.start_date,
            "maturity_date": self.maturity_date,
            "notional": self.notional,
            "units": self.units,
            "currency": self.currency,
            "payment_frequency": self.payment_frequency,
            "coupon_day_counter": self.coupon_day_counter,
            "coupons": self.coupons.iter().map(CouponDefinition::specification).collect::<PyResult<Vec<_>>>()?,
            "notional_percentages": self.notional_percentages,
            "call_schedule": self.call_schedule.iter().map(|call| call.inner).collect::<Vec<_>>(),
            "calendar": self.calendar,
            "business_day_convention": self.business_day_convention,
            "date_generation_rule": self.date_generation_rule,
            "end_of_month": self.end_of_month,
            "first_coupon_date": self.first_coupon_date,
            "next_to_last_coupon_date": self.next_to_last_coupon_date,
            "termination_date_convention": self.termination_date_convention,
        });
        pythonize::pythonize(py, &value).map_err(qs_err)
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A floating-rate note.
#[pyclass(module = "quantsupport", name = "FloatingRateNote", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct FloatingRateNote {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub spread: f64,
    pub forward_index: MarketIndex,
    pub currency: Currency,
    pub payment_frequency: Frequency,
    pub payment_structure: PaymentStructure,
}

macro_rules! build_frn_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        MakeFloatingRateNote::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_spread($self.spread)
            .with_notional($position.notional().unwrap_or($self.notional))
            .with_forward_index($self.forward_index.clone())
            .with_currency($self.currency)
            .with_side($position.side())
            .with_payment_frequency($self.payment_frequency)
            .with_payment_structure($self.payment_structure)
            .build()
            .map_err(qs_err)
    }};
}

impl FloatingRateNote {
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self, position: &Trade) -> PyResult<FloatingRateNoteTrade<DualFwd>> {
        let instrument: QsFloatingRateNote<DualFwd> =
            build_frn_instrument!(self, position, DualFwd)?;
        Ok(FloatingRateNoteTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFloatingRateNote<f64> = build_frn_instrument!(self, position, f64)?;
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
        payment_frequency = None,
        payment_structure = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        spread: f64,
        currency: &Bound<'_, PyAny>,
        forward_index: &Bound<'_, PyAny>,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        payment_structure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                notional,
                spread,
                forward_index: extract_market_index(forward_index)?,
                currency: extract_currency(currency)?,
                payment_frequency: payment_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Quarterly),
                payment_structure: payment_structure
                    .map(extract_payment_structure)
                    .transpose()?
                    .unwrap_or(PaymentStructure::Bullet),
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A fixed-rate deposit.
#[pyclass(module = "quantsupport", name = "FixedRateDeposit", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct FixedRateDeposit {
    pub identifier: String,
    pub start_date: Date,
    pub maturity_date: Date,
    pub notional: f64,
    pub rate: f64,
    pub currency: Currency,
    pub discount_index: Option<MarketIndex>,
    pub day_counter: DayCounter,
    pub compounding: Compounding,
    pub rate_frequency: Frequency,
}

macro_rules! build_deposit_instrument {
    ($self:expr, $position:expr, $t:ty) => {{
        MakeFixedRateDeposit::<$t>::default()
            .with_identifier($self.identifier.clone())
            .with_start_date($self.start_date)
            .with_maturity_date($self.maturity_date)
            .with_rate($self.rate)
            .with_notional($position.notional().unwrap_or($self.notional))
            .with_rate_definition(RateDefinition::new(
                $self.day_counter,
                $self.compounding,
                $self.rate_frequency,
            ))
            .with_discount_index($self.discount_index.clone())
            .with_currency($self.currency)
            .with_side($position.side())
            .build()
            .map_err(qs_err)
    }};
}

impl FixedRateDeposit {
    /// Builds an AD-enabled trade for pricing.
    pub fn build_trade_dual(&self, position: &Trade) -> PyResult<FixedRateDepositTrade<DualFwd>> {
        let instrument: QsFixedRateDeposit<DualFwd> =
            build_deposit_instrument!(self, position, DualFwd)?;
        Ok(FixedRateDepositTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
        ))
    }

    /// Decomposes the trade into contingent claims for XVA.
    pub fn claims(&self, position: &Trade) -> PyResult<Vec<ContingentClaim>> {
        let instrument: QsFixedRateDeposit<f64> = build_deposit_instrument!(self, position, f64)?;
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
        day_counter = None,
        compounding = None,
        rate_frequency = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        rate: f64,
        currency: &Bound<'_, PyAny>,
        discount_index: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                notional,
                rate,
                currency: extract_currency(currency)?,
                discount_index: discount_index.map(extract_market_index).transpose()?,
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
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A deliverable or non-deliverable FX forward.
#[pyclass(module = "quantsupport", name = "FxForward", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct FxForwardPy {
    pub identifier: String,
    pub delivery_date: Date,
    pub base_currency: Currency,
    pub quote_currency: Currency,
    pub forward_price: Option<f64>,
    pub forward_points: Option<f64>,
    pub day_counter: Option<DayCounter>,
    pub fixing_date: Option<Date>,
    pub settlement_currency: Option<Currency>,
}

impl FxForwardPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<FxForwardTrade> {
        let mut mk = MakeFxForward::default()
            .with_identifier(self.identifier.clone())
            .with_delivery_date(self.delivery_date)
            .with_base_currency(self.base_currency)
            .with_quote_currency(self.quote_currency)
            .with_side(position.side());
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
            position.trade_date()?,
            position.notional().unwrap_or(1.0),
            position.side(),
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
        forward_price = None,
        forward_points = None,
        day_counter = None,
        fixing_date = None,
        settlement_currency = None,
    ))]
    fn new(
        identifier: String,
        delivery_date: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        quote_currency: &Bound<'_, PyAny>,
        forward_price: Option<f64>,
        forward_points: Option<f64>,
        day_counter: Option<&Bound<'_, PyAny>>,
        fixing_date: Option<&Bound<'_, PyAny>>,
        settlement_currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                delivery_date: extract_date(delivery_date)?,
                base_currency: extract_currency(base_currency)?,
                quote_currency: extract_currency(quote_currency)?,
                forward_price,
                forward_points,
                day_counter: day_counter.map(extract_day_counter).transpose()?,
                fixing_date: fixing_date.map(extract_date).transpose()?,
                settlement_currency: settlement_currency.map(extract_currency).transpose()?,
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A European FX option.
#[pyclass(module = "quantsupport", name = "FxOption", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct FxOptionPy {
    pub identifier: String,
    pub expiry_date: Date,
    pub strike: f64,
    pub option_type: EuroOptionType,
    pub base_currency: Currency,
    pub quote_currency: Currency,
}

impl FxOptionPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<FxEuropeanOptionTrade> {
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
            position.trade_date()?,
            position.notional().unwrap_or(1.0),
            position.side(),
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
    ))]
    fn new(
        identifier: String,
        expiry_date: &Bound<'_, PyAny>,
        strike: f64,
        option_type: &Bound<'_, PyAny>,
        base_currency: &Bound<'_, PyAny>,
        quote_currency: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                expiry_date: extract_date(expiry_date)?,
                strike,
                option_type: extract_option_type(option_type)?,
                base_currency: extract_currency(base_currency)?,
                quote_currency: extract_currency(quote_currency)?,
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A European equity option.
#[pyclass(module = "quantsupport", name = "EquityOption", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct EquityOption {
    pub identifier: String,
    pub market_index: MarketIndex,
    pub expiry_date: Date,
    pub strike: Strike,
    pub option_type: EuroOptionType,
}

impl EquityOption {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<EquityEuropeanOptionTrade> {
        let instrument = QsEquityEuropeanOption::new(
            self.market_index.clone(),
            self.expiry_date,
            self.strike,
            self.option_type,
            self.identifier.clone(),
        );
        Ok(EquityEuropeanOptionTrade::new(
            instrument,
            position.notional().unwrap_or(1.0),
            position.trade_date()?,
            position.side(),
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
    ))]
    fn new(
        identifier: String,
        market_index: &Bound<'_, PyAny>,
        expiry_date: &Bound<'_, PyAny>,
        strike: &Bound<'_, PyAny>,
        option_type: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                market_index: extract_market_index(market_index)?,
                expiry_date: extract_date(expiry_date)?,
                strike: extract_strike(strike)?,
                option_type: extract_option_type(option_type)?,
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A single-name credit default swap.
#[pyclass(module = "quantsupport", name = "CreditDefaultSwap", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
    pub premium_frequency: Frequency,
    pub day_counter: DayCounter,
}

impl CreditDefaultSwapPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<CdsTrade> {
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
            position.trade_date()?,
            position.notional().unwrap_or(1.0),
            position.side(),
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
        spread,
        currency,
        credit_index,
        discount_index,
        recovery = 0.4,
        premium_frequency = None,
        day_counter = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        spread: f64,
        currency: &Bound<'_, PyAny>,
        credit_index: &Bound<'_, PyAny>,
        discount_index: &Bound<'_, PyAny>,
        recovery: f64,
        premium_frequency: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let credit_index = extract_market_index(credit_index)?;
        let credit_index = if matches!(credit_index, MarketIndex::Credit(_)) {
            credit_index
        } else {
            // Allow passing a plain name or non-credit index; coerce to Credit.
            MarketIndex::Credit(credit_index.to_string())
        };
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                credit_index,
                discount_index: extract_market_index(discount_index)?,
                currency: extract_currency(currency)?,
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                spread,
                recovery,
                premium_frequency: premium_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Quarterly),
                day_counter: day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual360),
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// An interest-rate cap or floor.
#[pyclass(module = "quantsupport", name = "CapFloor", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
    pub frequency: Frequency,
}

impl CapFloorPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<CapFloorTrade> {
        let instrument: QsCapFloor = MakeCapFloor::default()
            .with_identifier(self.identifier.clone())
            .with_start_date(self.start_date)
            .with_maturity_date(self.maturity_date)
            .with_strike(self.strike)
            .with_notional(position.notional().unwrap_or(self.notional))
            .with_market_index(self.market_index.clone())
            .with_currency(self.currency)
            .with_side(position.side())
            .with_cap_floor_type(self.cap_floor_type)
            .with_frequency(self.frequency)
            .build()
            .map_err(qs_err)?;
        Ok(CapFloorTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(self.notional),
            position.side(),
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
        frequency = None,
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
        frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                start_date: extract_date(start_date)?,
                maturity_date: extract_date(maturity_date)?,
                strike,
                notional,
                market_index: extract_market_index(market_index)?,
                currency: extract_currency(currency)?,
                cap_floor_type: extract_cap_floor_type(cap_floor_type)?,
                frequency: frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Quarterly),
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// A single caplet or floorlet.
#[pyclass(module = "quantsupport", name = "CapletFloorlet", extends = Instrument, from_py_object, subclass, dict, unsendable)]
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
}

impl CapletFloorletPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<CapletFloorletTrade> {
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
            position.trade_date()?,
            position.notional().unwrap_or(1.0),
            position.side(),
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
        currency,
        market_index,
        payoff_type,
    ))]
    fn new(
        identifier: String,
        fixing_date: &Bound<'_, PyAny>,
        start_accrual_date: &Bound<'_, PyAny>,
        end_accrual_date: &Bound<'_, PyAny>,
        payment_date: &Bound<'_, PyAny>,
        strike: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
        market_index: &Bound<'_, PyAny>,
        payoff_type: &Bound<'_, PyAny>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                market_index: extract_market_index(market_index)?,
                currency: extract_currency(currency)?,
                fixing_date: extract_date(fixing_date)?,
                start_accrual_date: extract_date(start_accrual_date)?,
                end_accrual_date: extract_date(end_accrual_date)?,
                payment_date: extract_date(payment_date)?,
                payoff_type: extract_caplet_floorlet_type(payoff_type)?,
                strike: extract_strike(strike)?,
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// An exchange-traded interest-rate futures position.
#[pyclass(module = "quantsupport", name = "RateFutures", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct RateFuturesPy {
    pub identifier: String,
    pub market_index: MarketIndex,
    pub start_date: Date,
    pub end_date: Date,
    pub futures_price: f64,
    pub contract_size: Option<f64>,
}

impl RateFuturesPy {
    /// Builds the trade for pricing.
    pub fn build_trade(&self, position: &Trade) -> PyResult<RateFuturesTrade> {
        let mut mk = MakeRateFutures::default()
            .with_identifier(self.identifier.clone())
            .with_market_index(self.market_index.clone())
            .with_start_date(self.start_date)
            .with_end_date(self.end_date)
            .with_futures_price(self.futures_price)
            .with_side(position.side());
        if let Some(size) = self.contract_size {
            mk = mk.with_contract_size(size);
        }
        let instrument: QsRateFutures = mk.build().map_err(qs_err)?;
        Ok(RateFuturesTrade::new(
            instrument,
            position.trade_date()?,
            position.num_contracts(),
            position.side(),
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
        contract_size = None,
    ))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        end_date: &Bound<'_, PyAny>,
        futures_price: f64,
        market_index: &Bound<'_, PyAny>,
        contract_size: Option<f64>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier: identifier.clone(),
                market_index: extract_market_index(market_index)?,
                start_date: extract_date(start_date)?,
                end_date: extract_date(end_date)?,
                futures_price,
                contract_size,
            }),
        )
    }

    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// Equity forward contract; its position size belongs to `Trade`.
#[pyclass(module = "quantsupport", name = "EquityForward", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct EquityForward {
    identifier: String,
    market_index: MarketIndex,
    delivery_date: Date,
    strike: f64,
    currency: Currency,
    day_counter: DayCounter,
}
impl EquityForward {
    pub fn build_trade(
        &self,
        position: &Trade,
    ) -> PyResult<quantsupport::prelude::EquityForwardTrade> {
        let instrument = quantsupport::prelude::MakeEquityForward::default()
            .with_identifier(self.identifier.clone())
            .with_market_index(self.market_index.clone())
            .with_delivery_date(self.delivery_date)
            .with_strike(self.strike)
            .with_currency(self.currency)
            .with_day_counter(self.day_counter)
            .build()
            .map_err(qs_err)?;
        Ok(quantsupport::prelude::EquityForwardTrade::new(
            instrument,
            position.trade_date()?,
            position.notional().unwrap_or(1.0),
            position.side(),
        ))
    }
}
#[pymethods]
impl EquityForward {
    #[new]
    #[pyo3(signature=(identifier, market_index, delivery_date, strike, currency, day_counter=None))]
    fn new(
        identifier: String,
        market_index: &Bound<'_, PyAny>,
        delivery_date: &Bound<'_, PyAny>,
        strike: f64,
        currency: &Bound<'_, PyAny>,
        day_counter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        if !strike.is_finite() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "strike must be finite",
            ));
        }
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier,
                market_index: extract_market_index(market_index)?,
                delivery_date: extract_date(delivery_date)?,
                strike,
                currency: extract_currency(currency)?,
                day_counter: day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual360),
            }),
        )
    }
    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
    #[getter]
    fn strike(&self) -> f64 {
        self.strike
    }
    #[getter]
    fn delivery_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.delivery_date,
        }
    }
}

/// European option to enter a payer or receiver fixed/floating swap at expiry.
#[pyclass(module = "quantsupport", name = "EuropeanSwaption", extends = Instrument, from_py_object, subclass, dict, unsendable)]
#[derive(Clone)]
pub struct EuropeanSwaption {
    identifier: String,
    expiry_date: Date,
    maturity_date: Date,
    strike: f64,
    notional: f64,
    currency: Currency,
    pub market_index: MarketIndex,
    swaption_type: quantsupport::prelude::SwaptionType,
    day_counter: DayCounter,
    compounding: Compounding,
    rate_frequency: Frequency,
    fixed_leg_frequency: Frequency,
    floating_leg_frequency: Frequency,
    calendar: Option<Calendar>,
}
impl EuropeanSwaption {
    pub fn build_trade_dual(
        &self,
        position: &Trade,
    ) -> PyResult<quantsupport::prelude::EuropeanSwaptionTrade<DualFwd>> {
        let amount = position.notional().unwrap_or(self.notional);
        // Unit underlying: the trade carries the position notional applied by the pricer.
        let mut builder = quantsupport::prelude::MakeSwaption::<DualFwd>::default()
            .with_identifier(self.identifier.clone())
            .with_expiry(self.expiry_date)
            .with_swap_tenor_date(self.maturity_date)
            .with_strike(self.strike)
            .with_notional(1.0)
            .with_currency(self.currency)
            .with_market_index(self.market_index.clone())
            .with_swaption_type(self.swaption_type)
            .with_rate_definition(RateDefinition::new(
                self.day_counter,
                self.compounding,
                self.rate_frequency,
            ))
            .with_fixed_leg_frequency(self.fixed_leg_frequency)
            .with_floating_leg_frequency(self.floating_leg_frequency);
        if let Some(calendar) = &self.calendar {
            builder = builder.with_calendar(calendar.clone());
        }
        let instrument = builder.build().map_err(qs_err)?;
        Ok(quantsupport::prelude::EuropeanSwaptionTrade::new(
            instrument,
            position.trade_date()?,
            amount,
            position.side(),
        ))
    }
}
#[pymethods]
impl EuropeanSwaption {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature=(identifier, expiry_date, maturity_date, strike, notional, currency, market_index,
        swaption_type=None, day_counter=None, compounding=None, rate_frequency=None,
        fixed_leg_frequency=None, floating_leg_frequency=None, calendar=None))]
    fn new(
        identifier: String,
        expiry_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        strike: f64,
        notional: f64,
        currency: &Bound<'_, PyAny>,
        market_index: &Bound<'_, PyAny>,
        swaption_type: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        rate_frequency: Option<&Bound<'_, PyAny>>,
        fixed_leg_frequency: Option<&Bound<'_, PyAny>>,
        floating_leg_frequency: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let expiry_date = extract_date(expiry_date)?;
        let maturity_date = extract_date(maturity_date)?;
        if !strike.is_finite()
            || !notional.is_finite()
            || notional < 0.0
            || expiry_date >= maturity_date
        {
            return Err(pyo3::exceptions::PyValueError::new_err("swaption requires finite strike, non-negative finite notional, and maturity after expiry"));
        }
        Ok(
            PyClassInitializer::from(Instrument::new(identifier.clone())).add_subclass(Self {
                identifier,
                expiry_date,
                maturity_date,
                strike,
                notional,
                currency: extract_currency(currency)?,
                market_index: extract_market_index(market_index)?,
                swaption_type: swaption_type
                    .map(crate::common::conversion::extract_swaption_type)
                    .transpose()?
                    .unwrap_or(quantsupport::prelude::SwaptionType::Payer),
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
                fixed_leg_frequency: fixed_leg_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Annual),
                floating_leg_frequency: floating_leg_frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Quarterly),
                calendar: calendar.map(extract_calendar).transpose()?,
            }),
        )
    }
    #[getter]
    fn identifier(&self) -> &str {
        &self.identifier
    }
    #[getter]
    fn notional(&self) -> f64 {
        self.notional
    }
    #[getter]
    fn strike(&self) -> f64 {
        self.strike
    }
    #[getter]
    fn swaption_type(&self) -> crate::common::enums::SwaptionType {
        self.swaption_type.into()
    }
    #[getter]
    fn expiry_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.expiry_date,
        }
    }
    #[getter]
    fn maturity_date(&self) -> crate::time::Date {
        crate::time::Date {
            inner: self.maturity_date,
        }
    }
}

// Contract terms use the same read-only property convention for every product.
macro_rules! contract_getters {
    ($class:ty, $this:ident, $py:ident; $( $field:ident : $ty:ty => $value:expr; )*) => {
        #[pymethods]
        impl $class {
            $(
                #[doc = concat!("Read-only contractual `", stringify!($field), "` input. Collections are returned as copies.")]
                #[getter]
                fn $field(&self, $py: Python<'_>) -> $ty {
                    let $this = self;
                    let _ = &$py;
                    $value
                }
            )*
        }
    };
}

contract_getters! { Swap, this, py;
    spread: Option<f64> => this.spread.clone().map(|value| value);
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
    compounding: crate::common::enums::Compounding => this.compounding.into();
    rate_frequency: crate::common::enums::Frequency => this.rate_frequency.into();
    fixed_leg_frequency: crate::common::enums::Frequency => this.fixed_leg_frequency.into();
    floating_leg_frequency: crate::common::enums::Frequency => this.floating_leg_frequency.into();
}

contract_getters! { CrossCurrencySwap, this, py;
    domestic_notional: f64 => this.domestic_notional;
    foreign_notional: f64 => this.foreign_notional;
    domestic_spread: f64 => this.domestic_spread;
    foreign_spread: f64 => this.foreign_spread;
    domestic_leg_frequency: crate::common::enums::Frequency => this.domestic_leg_frequency.into();
    foreign_leg_frequency: crate::common::enums::Frequency => this.foreign_leg_frequency.into();
}

contract_getters! { BasisSwap, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    notional: f64 => this.notional;
    currency: crate::common::enums::Currency => this.currency.into();
    pay_market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.pay_market_index.clone() };
    receive_market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.receive_market_index.clone() };
    pay_spread: Option<f64> => this.pay_spread.clone().map(|value| value);
    receive_spread: Option<f64> => this.receive_spread.clone().map(|value| value);
    pay_leg_frequency: crate::common::enums::Frequency => this.pay_leg_frequency.into();
    receive_leg_frequency: crate::common::enums::Frequency => this.receive_leg_frequency.into();
}

contract_getters! { FixFloatCrossCurrencySwap, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    domestic_notional: f64 => this.domestic_notional;
    foreign_notional: f64 => this.foreign_notional;
    fixed_rate: f64 => this.fixed_rate;
    spread: Option<f64> => this.spread.clone().map(|value| value);
    domestic_currency: crate::common::enums::Currency => this.domestic_currency.into();
    foreign_currency: crate::common::enums::Currency => this.foreign_currency.into();
    floating_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.floating_index.clone() };
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
    compounding: crate::common::enums::Compounding => this.compounding.into();
    rate_frequency: crate::common::enums::Frequency => this.rate_frequency.into();
    domestic_leg_frequency: crate::common::enums::Frequency => this.domestic_leg_frequency.into();
    foreign_leg_frequency: crate::common::enums::Frequency => this.foreign_leg_frequency.into();
}

contract_getters! { FixedRateBond, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    notional: f64 => this.notional;
    rate: f64 => this.rate;
    currency: crate::common::enums::Currency => this.currency.into();
    discount_index: Option<crate::common::enums::MarketIndex> => this.discount_index.clone().map(|value| crate::common::enums::MarketIndex { inner: value.clone() });
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
    compounding: crate::common::enums::Compounding => this.compounding.into();
    rate_frequency: crate::common::enums::Frequency => this.rate_frequency.into();
    payment_frequency: crate::common::enums::Frequency => this.payment_frequency.into();
    payment_structure: crate::common::enums::PaymentStructure => this.payment_structure.into();
    calendar: Option<crate::time::Calendar> => this.calendar.clone().map(|value| crate::time::Calendar { inner: value.clone() });
    business_day_convention: Option<crate::common::enums::BusinessDayConvention> => this.business_day_convention.clone().map(|value| value.into());
    date_generation_rule: Option<crate::common::enums::DateGenerationRule> => this.date_generation_rule.clone().map(|value| value.into());
    end_of_month: Option<bool> => this.end_of_month.clone().map(|value| value);
    first_coupon_date: Option<crate::time::Date> => this.first_coupon_date.clone().map(|value| crate::time::Date { inner: value });
}

contract_getters! { SteppedCouponBond, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    notional: f64 => this.notional;
    units: f64 => this.units;
    currency: crate::common::enums::Currency => this.currency.into();
    payment_frequency: crate::common::enums::Frequency => this.payment_frequency.into();
    coupon_day_counter: crate::common::enums::DayCounter => this.coupon_day_counter.into();
    coupons: PyResult<Vec<Py<PyAny>>> => this.coupons.iter().map(|coupon| coupon.to_python(py)).collect();
    notional_percentages: Vec<f64> => this.notional_percentages.clone();
    call_schedule: Vec<CallScheduleEntry> => this.call_schedule.clone();
    calendar: Option<crate::time::Calendar> => this.calendar.clone().map(|value| crate::time::Calendar { inner: value.clone() });
    business_day_convention: Option<crate::common::enums::BusinessDayConvention> => this.business_day_convention.clone().map(|value| value.into());
    date_generation_rule: Option<crate::common::enums::DateGenerationRule> => this.date_generation_rule.clone().map(|value| value.into());
    end_of_month: Option<bool> => this.end_of_month.clone().map(|value| value);
    first_coupon_date: Option<crate::time::Date> => this.first_coupon_date.clone().map(|value| crate::time::Date { inner: value });
    next_to_last_coupon_date: Option<crate::time::Date> => this.next_to_last_coupon_date.clone().map(|value| crate::time::Date { inner: value });
    termination_date_convention: Option<crate::common::enums::BusinessDayConvention> => this.termination_date_convention.clone().map(|value| value.into());
}

contract_getters! { FloatingRateNote, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    notional: f64 => this.notional;
    spread: f64 => this.spread;
    forward_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.forward_index.clone() };
    currency: crate::common::enums::Currency => this.currency.into();
    payment_frequency: crate::common::enums::Frequency => this.payment_frequency.into();
    payment_structure: crate::common::enums::PaymentStructure => this.payment_structure.into();
}

contract_getters! { FixedRateDeposit, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    notional: f64 => this.notional;
    rate: f64 => this.rate;
    currency: crate::common::enums::Currency => this.currency.into();
    discount_index: Option<crate::common::enums::MarketIndex> => this.discount_index.clone().map(|value| crate::common::enums::MarketIndex { inner: value.clone() });
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
    compounding: crate::common::enums::Compounding => this.compounding.into();
    rate_frequency: crate::common::enums::Frequency => this.rate_frequency.into();
}

contract_getters! { FxForwardPy, this, py;
    delivery_date: crate::time::Date => crate::time::Date { inner: this.delivery_date };
    base_currency: crate::common::enums::Currency => this.base_currency.into();
    quote_currency: crate::common::enums::Currency => this.quote_currency.into();
    forward_price: Option<f64> => this.forward_price.clone().map(|value| value);
    forward_points: Option<f64> => this.forward_points.clone().map(|value| value);
    day_counter: Option<crate::common::enums::DayCounter> => this.day_counter.clone().map(|value| value.into());
    fixing_date: Option<crate::time::Date> => this.fixing_date.clone().map(|value| crate::time::Date { inner: value });
    settlement_currency: Option<crate::common::enums::Currency> => this.settlement_currency.clone().map(|value| value.into());
}

contract_getters! { FxOptionPy, this, py;
    expiry_date: crate::time::Date => crate::time::Date { inner: this.expiry_date };
    strike: f64 => this.strike;
    option_type: crate::common::enums::OptionType => this.option_type.into();
    base_currency: crate::common::enums::Currency => this.base_currency.into();
    quote_currency: crate::common::enums::Currency => this.quote_currency.into();
}

contract_getters! { EquityOption, this, py;
    market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.market_index.clone() };
    expiry_date: crate::time::Date => crate::time::Date { inner: this.expiry_date };
    strike: PyResult<Py<PyAny>> => pythonize::pythonize(py, &this.strike).map(Bound::unbind).map_err(qs_err);
    option_type: crate::common::enums::OptionType => this.option_type.into();
}

contract_getters! { CreditDefaultSwapPy, this, py;
    credit_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.credit_index.clone() };
    discount_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.discount_index.clone() };
    currency: crate::common::enums::Currency => this.currency.into();
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    spread: f64 => this.spread;
    recovery: f64 => this.recovery;
    premium_frequency: crate::common::enums::Frequency => this.premium_frequency.into();
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
}

contract_getters! { CapFloorPy, this, py;
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    maturity_date: crate::time::Date => crate::time::Date { inner: this.maturity_date };
    strike: f64 => this.strike;
    notional: f64 => this.notional;
    market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.market_index.clone() };
    currency: crate::common::enums::Currency => this.currency.into();
    cap_floor_type: crate::common::enums::CapFloorType => this.cap_floor_type.into();
    frequency: crate::common::enums::Frequency => this.frequency.into();
}

contract_getters! { CapletFloorletPy, this, py;
    market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.market_index.clone() };
    currency: crate::common::enums::Currency => this.currency.into();
    fixing_date: crate::time::Date => crate::time::Date { inner: this.fixing_date };
    start_accrual_date: crate::time::Date => crate::time::Date { inner: this.start_accrual_date };
    end_accrual_date: crate::time::Date => crate::time::Date { inner: this.end_accrual_date };
    payment_date: crate::time::Date => crate::time::Date { inner: this.payment_date };
    payoff_type: crate::common::enums::CapletFloorletType => this.payoff_type.into();
    strike: PyResult<Py<PyAny>> => pythonize::pythonize(py, &this.strike).map(Bound::unbind).map_err(qs_err);
}

contract_getters! { RateFuturesPy, this, py;
    market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.market_index.clone() };
    start_date: crate::time::Date => crate::time::Date { inner: this.start_date };
    end_date: crate::time::Date => crate::time::Date { inner: this.end_date };
    futures_price: f64 => this.futures_price;
    contract_size: Option<f64> => this.contract_size.clone().map(|value| value);
}

contract_getters! { EquityForward, this, py;
    market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.market_index.clone() };
    currency: crate::common::enums::Currency => this.currency.into();
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
}

contract_getters! { EuropeanSwaption, this, py;
    currency: crate::common::enums::Currency => this.currency.into();
    market_index: crate::common::enums::MarketIndex => crate::common::enums::MarketIndex { inner: this.market_index.clone() };
    day_counter: crate::common::enums::DayCounter => this.day_counter.into();
    compounding: crate::common::enums::Compounding => this.compounding.into();
    rate_frequency: crate::common::enums::Frequency => this.rate_frequency.into();
    fixed_leg_frequency: crate::common::enums::Frequency => this.fixed_leg_frequency.into();
    floating_leg_frequency: crate::common::enums::Frequency => this.floating_leg_frequency.into();
    calendar: Option<crate::time::Calendar> => this.calendar.clone().map(|value| crate::time::Calendar { inner: value.clone() });
}

#[pymethods]
impl Swap {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "notional": self.notional,
                "fixed_rate": self.fixed_rate,
                "spread": self.spread,
                "currency": self.currency,
                "market_index": self.market_index,
                "day_counter": self.day_counter,
                "compounding": self.compounding,
                "rate_frequency": self.rate_frequency,
                "fixed_leg_frequency": self.fixed_leg_frequency,
                "floating_leg_frequency": self.floating_leg_frequency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl CrossCurrencySwap {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "domestic_notional": self.domestic_notional,
                "foreign_notional": self.foreign_notional,
                "domestic_spread": self.domestic_spread,
                "foreign_spread": self.foreign_spread,
                "domestic_currency": self.domestic_currency,
                "foreign_currency": self.foreign_currency,
                "domestic_market_index": self.domestic_market_index,
                "foreign_market_index": self.foreign_market_index,
                "domestic_leg_frequency": self.domestic_leg_frequency,
                "foreign_leg_frequency": self.foreign_leg_frequency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl BasisSwap {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "notional": self.notional,
                "currency": self.currency,
                "pay_market_index": self.pay_market_index,
                "receive_market_index": self.receive_market_index,
                "pay_spread": self.pay_spread,
                "receive_spread": self.receive_spread,
                "pay_leg_frequency": self.pay_leg_frequency,
                "receive_leg_frequency": self.receive_leg_frequency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl FixFloatCrossCurrencySwap {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "domestic_notional": self.domestic_notional,
                "foreign_notional": self.foreign_notional,
                "fixed_rate": self.fixed_rate,
                "spread": self.spread,
                "domestic_currency": self.domestic_currency,
                "foreign_currency": self.foreign_currency,
                "floating_index": self.floating_index,
                "day_counter": self.day_counter,
                "compounding": self.compounding,
                "rate_frequency": self.rate_frequency,
                "domestic_leg_frequency": self.domestic_leg_frequency,
                "foreign_leg_frequency": self.foreign_leg_frequency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl FloatingRateNote {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &serde_json::json!({
            "identifier": self.identifier,
            "start_date": self.start_date,
            "maturity_date": self.maturity_date,
            "notional": self.notional,
            "spread": self.spread,
            "forward_index": self.forward_index,
            "currency": self.currency,
            "payment_frequency": self.payment_frequency,
            "payment_structure": format!("{:?}", crate::common::enums::PaymentStructure::from(self.payment_structure)),
        })).map_err(qs_err)
    }
}

#[pymethods]
impl FixedRateDeposit {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "notional": self.notional,
                "rate": self.rate,
                "currency": self.currency,
                "discount_index": self.discount_index,
                "day_counter": self.day_counter,
                "compounding": self.compounding,
                "rate_frequency": self.rate_frequency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl FxForwardPy {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "delivery_date": self.delivery_date,
                "base_currency": self.base_currency,
                "quote_currency": self.quote_currency,
                "forward_price": self.forward_price,
                "forward_points": self.forward_points,
                "day_counter": self.day_counter,
                "fixing_date": self.fixing_date,
                "settlement_currency": self.settlement_currency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl FxOptionPy {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "expiry_date": self.expiry_date,
                "strike": self.strike,
                "option_type": self.option_type,
                "base_currency": self.base_currency,
                "quote_currency": self.quote_currency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl EquityOption {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "market_index": self.market_index,
                "expiry_date": self.expiry_date,
                "strike": self.strike,
                "option_type": self.option_type,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl CreditDefaultSwapPy {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "credit_index": self.credit_index,
                "discount_index": self.discount_index,
                "currency": self.currency,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "spread": self.spread,
                "recovery": self.recovery,
                "premium_frequency": self.premium_frequency,
                "day_counter": self.day_counter,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl CapFloorPy {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "start_date": self.start_date,
                "maturity_date": self.maturity_date,
                "strike": self.strike,
                "notional": self.notional,
                "market_index": self.market_index,
                "currency": self.currency,
                "cap_floor_type": self.cap_floor_type,
                "frequency": self.frequency,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl CapletFloorletPy {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "market_index": self.market_index,
                "currency": self.currency,
                "fixing_date": self.fixing_date,
                "start_accrual_date": self.start_accrual_date,
                "end_accrual_date": self.end_accrual_date,
                "payment_date": self.payment_date,
                "payoff_type": self.payoff_type,
                "strike": self.strike,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl RateFuturesPy {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "market_index": self.market_index,
                "start_date": self.start_date,
                "end_date": self.end_date,
                "futures_price": self.futures_price,
                "contract_size": self.contract_size,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl EquityForward {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "market_index": self.market_index,
                "delivery_date": self.delivery_date,
                "strike": self.strike,
                "currency": self.currency,
                "day_counter": self.day_counter,
            }),
        )
        .map_err(qs_err)
    }
}

#[pymethods]
impl EuropeanSwaption {
    /// Returns JSON-compatible constructor fields.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(
            py,
            &serde_json::json!({
                "identifier": self.identifier,
                "expiry_date": self.expiry_date,
                "maturity_date": self.maturity_date,
                "strike": self.strike,
                "notional": self.notional,
                "currency": self.currency,
                "market_index": self.market_index,
                "swaption_type": self.swaption_type,
                "day_counter": self.day_counter,
                "compounding": self.compounding,
                "rate_frequency": self.rate_frequency,
                "fixed_leg_frequency": self.fixed_leg_frequency,
                "floating_leg_frequency": self.floating_leg_frequency,
                "calendar": self.calendar,
            }),
        )
        .map_err(qs_err)
    }
}

crate::common::representation::impl_dict_protocol!(
    Swap,
    CrossCurrencySwap,
    BasisSwap,
    FixFloatCrossCurrencySwap,
    CallScheduleEntry,
    FixedRateBond,
    SteppedCouponBond,
    FloatingRateNote,
    FixedRateDeposit,
    FxForwardPy,
    FxOptionPy,
    EquityOption,
    CreditDefaultSwapPy,
    CapFloorPy,
    CapletFloorletPy,
    RateFuturesPy,
    EquityForward,
    EuropeanSwaption
);
