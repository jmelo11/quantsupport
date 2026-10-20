//! Python-facing enums mirroring the library's Rust enums.
//!
//! Fieldless library enums are mirrored one-to-one through the
//! `mirror_enum!` macro: the exhaustive `match` in both `From`
//! conversions guarantees at compile time that the Python enum stays in
//! sync with the library. [`MarketIndex`] (which has data-carrying
//! variants) is a thin wrapper with constants and constructors instead.
//!
//! Financial input converters also accept serialized names such as `"USD"`,
//! `"SOFR"`, or `"Actual360"`. Typed calendar constructors take their market enums.

// Variant names intentionally mirror the library / ISO conventions.
#![allow(clippy::upper_case_acronyms, clippy::enum_variant_names)]

use pyo3::prelude::*;
use quantsupport::prelude::{
    BusinessDayConvention as QsBusinessDayConvention, CapFloorType as QsCapFloorType,
    CapletFloorletType as QsCapletFloorletType, Compounding as QsCompounding,
    Currency as QsCurrency, DateGenerationRule as QsDateGenerationRule, DayCounter as QsDayCounter,
    EuroOptionType as QsEuroOptionType, Frequency as QsFrequency, FxPair,
    Interpolator as QsInterpolator, Level as QsLevel, MarketIndex as QsMarketIndex,
    PaymentStructure as QsPaymentStructure, Request as QsRequest, ScenarioType as QsScenarioType,
    Side as QsSide, SmileType as QsSmileType, TimeUnit as QsTimeUnit,
    VolatilityType as QsVolatilityType,
};

use quantsupport::time::calendars::{
    australia::Market as QsAustraliaMarket, brazil::Market as QsBrazilMarket,
    canada::Market as QsCanadaMarket, china::Market as QsChinaMarket,
    jointcalendar::JointCalendarRule as QsJointCalendarRule,
    newzealand::Market as QsNewZealandMarket, southkorea::Market as QsSouthKoreaMarket,
    unitedstates::Market as QsUnitedStatesMarket,
};

use crate::common::conversion::{extract_currency, extract_date, qs_err};
use crate::QuantSupportError;

/// Defines a Python enum mirroring a fieldless library enum, with
/// bidirectional (compile-time exhaustive) conversions and a `parse`
/// constructor.
macro_rules! mirror_enum {
    ($(#[$meta:meta])* $name:ident, $qs:ty, $pyname:literal, [$($variant:ident),+ $(,)?]) => {
        $(#[$meta])*
        #[pyclass(module = "quantsupport", name = $pyname, eq, frozen, hash, from_py_object)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl From<$name> for $qs {
            fn from(v: $name) -> Self {
                match v {
                    $($name::$variant => <$qs>::$variant),+
                }
            }
        }

        impl From<$qs> for $name {
            fn from(v: $qs) -> Self {
                match v {
                    $(<$qs>::$variant => Self::$variant),+
                }
            }
        }

        impl $name {
            /// Parses the enum from its variant name (case-insensitive).
            pub fn from_name(s: &str) -> PyResult<Self> {
                $(
                    if s.eq_ignore_ascii_case(stringify!($variant)) {
                        return Ok(Self::$variant);
                    }
                )+
                Err(QuantSupportError::new_err(format!(
                    concat!("invalid ", $pyname, " '{}' (expected one of: {})"),
                    s,
                    [$(stringify!($variant)),+].join(", ")
                )))
            }
        }
    };
}

mirror_enum!(
    /// Market quote level used for construction and calibration.
    Level,
    QsLevel,
    "Level",
    [Mid, Bid, Ask]
);

#[pymethods]
impl Level {
    /// Parses a quote level from its name (case-insensitive).
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("Level.{self:?}")
    }
}

mirror_enum!(
    /// ISO currency.
    Currency,
    QsCurrency,
    "Currency",
    [
        USD, EUR, JPY, ZAR, CLP, CLF, CHF, BRL, COP, MXN, AUD, CAD, CNY, GBP, NZD, NOK, SEK, PEN,
        CNH, INR, TWD, HKD, KRW, DKK, IDR,
    ]
);

#[pymethods]
impl Currency {
    /// Parses a currency from its alphabetic code (e.g. `"USD"`).
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    /// Alphabetic ISO 4217 code.
    #[getter]
    fn code(&self) -> &'static str {
        QsCurrency::from(*self).as_str()
    }

    /// Display name (e.g. `"US Dollar"`).
    #[getter]
    fn name(&self) -> &'static str {
        QsCurrency::from(*self).name()
    }

    /// Currency symbol (e.g. `"$"`).
    #[getter]
    fn symbol(&self) -> &'static str {
        QsCurrency::from(*self).symbol()
    }

    /// Decimal precision.
    #[getter]
    fn precision(&self) -> u8 {
        QsCurrency::from(*self).precision()
    }

    /// Numeric ISO 4217 code.
    #[getter]
    fn numeric_code(&self) -> u16 {
        QsCurrency::from(*self).numeric_code()
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("Currency.{self:?}")
    }
}

mirror_enum!(
    /// Direction of a trade's cashflows.
    Side,
    QsSide,
    "Side",
    [LongReceive, PayShort]
);

#[pymethods]
impl Side {
    /// Parses a side; accepts `"LongReceive"`/`"Long"`/`"Receive"` and
    /// `"PayShort"`/`"Pay"`/`"Short"`.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        if ["longreceive", "long", "receive"].contains(&s.to_ascii_lowercase().as_str()) {
            Ok(Self::LongReceive)
        } else if ["payshort", "pay", "short"].contains(&s.to_ascii_lowercase().as_str()) {
            Ok(Self::PayShort)
        } else {
            Err(QuantSupportError::new_err(format!(
                "invalid Side '{s}' (expected 'LongReceive' or 'PayShort')"
            )))
        }
    }

    /// Cashflow sign: +1 for `LongReceive`, -1 for `PayShort`.
    fn sign(&self) -> f64 {
        QsSide::from(*self).sign()
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("Side.{self:?}")
    }
}

mirror_enum!(
    /// Interest compounding convention.
    Compounding,
    QsCompounding,
    "Compounding",
    [
        Simple,
        Compounded,
        Continuous,
        SimpleThenCompounded,
        CompoundedThenSimple
    ]
);

#[pymethods]
impl Compounding {
    /// Parses a compounding convention from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("Compounding.{self:?}")
    }
}

mirror_enum!(
    /// Payment / coupon frequency.
    Frequency,
    QsFrequency,
    "Frequency",
    [
        NoFrequency,
        Once,
        Annual,
        Semiannual,
        EveryFourthMonth,
        Quarterly,
        Bimonthly,
        Monthly,
        EveryFourthWeek,
        Biweekly,
        Weekly,
        Daily,
        OtherFrequency,
    ]
);

#[pymethods]
impl Frequency {
    /// Parses a frequency from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("Frequency.{self:?}")
    }
}

mirror_enum!(
    /// Day count convention.
    DayCounter,
    QsDayCounter,
    "DayCounter",
    [
        Actual360,
        Actual365,
        Thirty360,
        Thirty360US,
        ActualActual,
        Business252
    ]
);

#[pymethods]
impl DayCounter {
    /// Parses a day counter from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    /// Number of days between two dates under this convention.
    fn day_count(&self, start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<i64> {
        Ok(QsDayCounter::from(*self).day_count(extract_date(start)?, extract_date(end)?))
    }

    /// Year fraction between two dates under this convention.
    fn year_fraction(&self, start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<f64> {
        Ok(QsDayCounter::from(*self).year_fraction(extract_date(start)?, extract_date(end)?))
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("DayCounter.{self:?}")
    }
}

mirror_enum!(
    /// Calendar time unit.
    TimeUnit,
    QsTimeUnit,
    "TimeUnit",
    [Days, Weeks, Months, Years]
);

#[pymethods]
impl TimeUnit {
    /// Parses a time unit from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("TimeUnit.{self:?}")
    }
}

mirror_enum!(
    /// Business day adjustment convention.
    BusinessDayConvention,
    QsBusinessDayConvention,
    "BusinessDayConvention",
    [
        Following,
        ModifiedFollowing,
        HalfMonthModifiedFollowing,
        Preceding,
        ModifiedPreceding,
        Unadjusted,
        Nearest,
    ]
);

#[pymethods]
impl BusinessDayConvention {
    /// Parses a convention from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("BusinessDayConvention.{self:?}")
    }
}

/// Evaluation request with an optional zero-based contractual target.
#[pyclass(module = "quantsupport", eq, frozen, hash, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Request {
    inner: QsRequest,
}

impl From<Request> for QsRequest {
    fn from(value: Request) -> Self {
        value.inner
    }
}
impl From<QsRequest> for Request {
    fn from(inner: QsRequest) -> Self {
        Self { inner }
    }
}

#[pymethods]
#[allow(non_upper_case_globals, non_snake_case)]
impl Request {
    #[classattr]
    pub const Value: Self = Self {
        inner: QsRequest::Value,
    };
    #[classattr]
    pub const YieldToMaturity: Self = Self {
        inner: QsRequest::YieldToMaturity,
    };
    #[classattr]
    pub const ModifiedDuration: Self = Self {
        inner: QsRequest::ModifiedDuration,
    };
    #[classattr]
    pub const Sensitivities: Self = Self {
        inner: QsRequest::Sensitivities,
    };
    #[classattr]
    pub const ZSpread: Self = Self {
        inner: QsRequest::ZSpread,
    };
    #[classattr]
    pub const OAS: Self = Self {
        inner: QsRequest::OAS,
    };
    #[classattr]
    pub const Cashflows: Self = Self {
        inner: QsRequest::Cashflows,
    };

    /// Selects a zero-based swap leg or loan coupon/spread segment.
    #[staticmethod]
    #[pyo3(signature = (index=0))]
    pub const fn FairRate(index: usize) -> Self {
        Self {
            inner: QsRequest::FairRate(index),
        }
    }

    /// Zero-based contractual target for FairRate; None for other requests.
    #[getter]
    pub const fn index(&self) -> Option<usize> {
        match self.inner {
            QsRequest::FairRate(index) => Some(index),
            _ => None,
        }
    }

    /// Parses a request, including FairRate(1). Bare FairRate selects target zero.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        let name = s.trim().to_ascii_lowercase().replace('_', "");
        match name.as_str() {
            "value" | "npv" => Ok(Self::Value),
            "sensitivities" => Ok(Self::Sensitivities),
            "cashflows" => Ok(Self::Cashflows),
            "fairrate" => Ok(Self::FairRate(0)),
            "yieldtomaturity" | "ytm" => Ok(Self::YieldToMaturity),
            "modifiedduration" => Ok(Self::ModifiedDuration),
            "zspread" => Ok(Self::ZSpread),
            "oas" => Ok(Self::OAS),
            _ => {
                if let Some(index) = name
                    .strip_prefix("fairrate(")
                    .and_then(|v| v.strip_suffix(')'))
                {
                    if let Ok(index) = index.trim().parse::<usize>() {
                        return Ok(Self::FairRate(index));
                    }
                }
                Err(QuantSupportError::new_err(format!(
                    "invalid Request '{s}'; use FairRate(nonnegative_index) for a targeted quote"
                )))
            }
        }
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.inner)
    }
    fn __repr__(&self) -> String {
        format!("Request.{:?}", self.inner)
    }
}

impl Request {
    pub fn from_name(s: &str) -> PyResult<Self> {
        Self::parse(s)
    }
}

mirror_enum!(
    /// Quotation convention of a volatility.
    VolatilityType,
    QsVolatilityType,
    "VolatilityType",
    [Black, Normal]
);

#[pymethods]
impl VolatilityType {
    /// Parses a volatility type from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("VolatilityType.{self:?}")
    }
}

mirror_enum!(
    /// Smile axis of a volatility surface or cube.
    SmileType,
    QsSmileType,
    "SmileType",
    [Strike, Delta, LogMoneyness]
);

#[pymethods]
impl SmileType {
    /// Parses a smile type from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("SmileType.{self:?}")
    }
}

/// A market index: rate indices (SOFR, ESTR, ...), equity underlyings, FX
/// pairs, collateral curves and custom indices.
///
/// Plain rate indices are available as class constants
/// (`MarketIndex.SOFR`); parameterised variants are built with
/// `MarketIndex.equity`, `MarketIndex.fx_pair`,
/// `MarketIndex.collateral` and `MarketIndex.other`.
#[pyclass(
    module = "quantsupport",
    name = "MarketIndex",
    eq,
    frozen,
    hash,
    from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MarketIndex {
    pub inner: QsMarketIndex,
}

/// Declares a class constant per fieldless `MarketIndex` variant.
macro_rules! market_index_constants {
    ($($variant:ident),+ $(,)?) => {
        #[pymethods]
        impl MarketIndex {
            $(
                #[classattr]
                #[allow(non_snake_case)]
                fn $variant() -> Self {
                    Self { inner: QsMarketIndex::$variant }
                }
            )+

            /// Parses a plain rate index from its name (e.g. `"SOFR"`),
            /// case-insensitively.
            #[staticmethod]
            fn parse(s: &str) -> PyResult<Self> {
                $(
                    if s.eq_ignore_ascii_case(stringify!($variant)) {
                        return Ok(Self { inner: QsMarketIndex::$variant });
                    }
                )+
                Err(QuantSupportError::new_err(format!(
                    "invalid MarketIndex '{}' (expected one of: {})",
                    s,
                    [$(stringify!($variant)),+].join(", ")
                )))
            }
        }
    };
}

market_index_constants!(
    FedFunds,
    CDI,
    DESTR,
    KOFR,
    ZARONIA,
    BBSW1m,
    BBSW2m,
    BBSW3m,
    BBSW4m,
    BBSW6m,
    BKBM1m,
    BKBM2m,
    BKBM3m,
    BKBM4m,
    BKBM6m,
    NIBOR1m,
    NIBOR3m,
    NIBOR6m,
    NIBOR12m,
    SHIBOR1d,
    SHIBOR1w,
    SHIBOR2w,
    SHIBOR1m,
    SHIBOR3m,
    SHIBOR6m,
    SHIBOR12m,
    EURIBOR1w,
    SOFR,
    SOFRCompounded,
    TermSOFR1m,
    TermSOFR3m,
    TermSOFR6m,
    TermSOFR12m,
    ESTR,
    EURIBOR1m,
    EURIBOR3m,
    EURIBOR6m,
    EURIBOR12m,
    SONIA,
    TONAR,
    TIBOR3m,
    TIBOR6m,
    SARON,
    CORRA,
    AONIA,
    NZONIA,
    NOWA,
    SWESTR,
    ICP,
    VIX,
);

#[pymethods]
impl MarketIndex {
    /// An equity underlying identified by name.
    #[staticmethod]
    fn equity(name: &str) -> Self {
        Self {
            inner: QsMarketIndex::Equity(name.to_string()),
        }
    }

    /// A directed FX spot pair (1 base = x quote).
    #[staticmethod]
    fn fx_pair(base: &Bound<'_, PyAny>, quote: &Bound<'_, PyAny>) -> PyResult<Self> {
        let pair =
            FxPair::new(extract_currency(base)?, extract_currency(quote)?).map_err(qs_err)?;
        Ok(Self {
            inner: QsMarketIndex::FxPair(pair),
        })
    }

    /// Collateral discount curve for cashflows in `currency` posted under a
    /// CSA denominated in `collateral_currency`.
    #[staticmethod]
    fn collateral(
        currency: &Bound<'_, PyAny>,
        collateral_currency: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: QsMarketIndex::Collateral(
                extract_currency(currency)?,
                extract_currency(collateral_currency)?,
            ),
        })
    }

    /// Credit curve of a reference entity, identified by name.
    #[staticmethod]
    fn credit(name: &str) -> Self {
        Self {
            inner: QsMarketIndex::Credit(name.to_string()),
        }
    }

    /// A custom index identified by name.
    #[staticmethod]
    fn other(name: &str) -> Self {
        Self {
            inner: QsMarketIndex::Other(name.to_string()),
        }
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!("MarketIndex('{}')", self.inner)
    }
}

mirror_enum!(
    /// How a scenario shock is applied to a quote value.
    ScenarioType,
    QsScenarioType,
    "ScenarioType",
    [Absolute, Relative]
);

#[pymethods]
impl ScenarioType {
    /// Parses a scenario type from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("ScenarioType.{self:?}")
    }
}

mirror_enum!(
    /// European option payoff direction (equity and FX options).
    OptionType,
    QsEuroOptionType,
    "OptionType",
    [Call, Put]
);

#[pymethods]
impl OptionType {
    /// Parses an option type from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("OptionType.{self:?}")
    }
}

mirror_enum!(
    /// Cap or floor direction.
    CapFloorType,
    QsCapFloorType,
    "CapFloorType",
    [Cap, Floor]
);

#[pymethods]
impl CapFloorType {
    /// Parses a cap/floor type from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("CapFloorType.{self:?}")
    }
}

mirror_enum!(
    /// Caplet or floorlet payoff direction.
    CapletFloorletType,
    QsCapletFloorletType,
    "CapletFloorletType",
    [Caplet, Floorlet]
);

#[pymethods]
impl CapletFloorletType {
    /// Parses a caplet/floorlet type from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("CapletFloorletType.{self:?}")
    }
}

mirror_enum!(
    /// Amortization structure of a leg.
    PaymentStructure,
    QsPaymentStructure,
    "PaymentStructure",
    [Bullet, EqualPayments, EqualRedemptions, Zero, Other]
);

#[pymethods]
impl PaymentStructure {
    /// Parses a payment structure from its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }

    fn __str__(&self) -> String {
        format!("{self:?}")
    }

    fn __repr__(&self) -> String {
        format!("PaymentStructure.{self:?}")
    }
}

mirror_enum!(
    /// Schedule date-generation rule.
    DateGenerationRule,
    QsDateGenerationRule,
    "DateGenerationRule",
    [
        Backward,
        Forward,
        Zero,
        ThirdWednesday,
        ThirdWednesdayInclusive,
        Twentieth,
        TwentiethIMM,
        OldCDS,
        CDS,
        CDS2015
    ]
);
#[pymethods]
impl DateGenerationRule {
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
    fn __repr__(&self) -> String {
        format!("DateGenerationRule.{self:?}")
    }
}

mirror_enum!(
    /// Payer or receiver European swaption.
    SwaptionType,
    quantsupport::prelude::SwaptionType,
    "SwaptionType",
    [Payer, Receiver]
);

#[pymethods]
impl SwaptionType {
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }
    fn __repr__(&self) -> String {
        format!("SwaptionType.{self:?}")
    }
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

mirror_enum!(
    /// Interpolation rule for curve pillars.
    Interpolator,
    QsInterpolator,
    "Interpolator",
    [Linear, LogLinear, CubicSpline]
);

#[pymethods]
impl Interpolator {
    /// Parses an interpolation rule by its name.
    #[staticmethod]
    pub fn parse(s: &str) -> PyResult<Self> {
        Self::from_name(s)
    }
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
    fn __repr__(&self) -> String {
        format!("Interpolator.{self:?}")
    }
}

mirror_enum!(
    /// United States calendar market.
    UnitedStatesMarket,
    QsUnitedStatesMarket,
    "UnitedStatesMarket",
    [
        Settlement,
        LiborImpact,
        Nyse,
        GovernmentBond,
        Nerc,
        FederalReserve,
        Sofr
    ]
);
mirror_enum!(
    /// Brazil calendar market.
    BrazilMarket,
    QsBrazilMarket,
    "BrazilMarket",
    [Settlement, Exchange]
);
mirror_enum!(
    /// Business-day combination rule for a joint calendar.
    JointCalendarRule,
    QsJointCalendarRule,
    "JointCalendarRule",
    [JoinHolidays, JoinBusinessDays]
);

macro_rules! calendar_enum_methods {
    ($name:ident) => {
        #[pymethods]
        impl $name {
            #[staticmethod]
            fn parse(s: &str) -> PyResult<Self> {
                Self::from_name(s)
            }
            fn __str__(&self) -> String {
                format!("{self:?}")
            }
            fn __repr__(&self) -> String {
                format!("{}.{self:?}", stringify!($name))
            }
        }
    };
}
calendar_enum_methods!(UnitedStatesMarket);
calendar_enum_methods!(BrazilMarket);
calendar_enum_methods!(JointCalendarRule);

mirror_enum!(
    /// Australia calendar market.
    AustraliaMarket,
    QsAustraliaMarket,
    "AustraliaMarket",
    [Settlement, ASX]
);
calendar_enum_methods!(AustraliaMarket);
mirror_enum!(
    /// Canada calendar market.
    CanadaMarket,
    QsCanadaMarket,
    "CanadaMarket",
    [Settlement, TSX]
);
calendar_enum_methods!(CanadaMarket);
mirror_enum!(
    /// NewZealand calendar market.
    NewZealandMarket,
    QsNewZealandMarket,
    "NewZealandMarket",
    [Wellington, Auckland]
);
calendar_enum_methods!(NewZealandMarket);
mirror_enum!(
    /// SouthKorea calendar market.
    SouthKoreaMarket,
    QsSouthKoreaMarket,
    "SouthKoreaMarket",
    [Settlement, KRX]
);
calendar_enum_methods!(SouthKoreaMarket);
mirror_enum!(
    /// China calendar market.
    ChinaMarket,
    QsChinaMarket,
    "ChinaMarket",
    [SSE, IB]
);
calendar_enum_methods!(ChinaMarket);
