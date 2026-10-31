//! Native coupon cashflows and composable payoff operations.
use crate::common::conversion::{
    at_path, expect_dict, extract_compounding, extract_date, extract_day_counter,
    extract_frequency, extract_market_index, from_py, qs_err,
};
use crate::common::enums::{
    Compounding as PyCompounding, DayCounter as PyDayCounter, Frequency as PyFrequency, MarketIndex,
};
use crate::time::Date;
use pyo3::prelude::*;
use quantsupport::instruments::cashflows::{
    cashflow::Cashflow, coupons::{LinearCoupon, NonLinearCoupon},
    fixedratecoupon::FixedRateCoupon as QsFixed,
    floatingratecoupon::FloatingRateCoupon as QsFloating,
    optionembeddedcoupon::OptionEmbeddedCoupon as QsOption, payoffops::PayoffOps as QsPayoff,
};
use quantsupport::prelude::{
    CashflowType, Compounding, Date as QsDate, DayCounter, Frequency, InterestRate as QsRate,
};

/// An annual interest rate with the native day-count and compounding conventions.
///
/// Parameters
/// ----------
/// rate : float
///     Annual decimal rate, for example 0.06 for 6%.
/// day_counter : DayCounter, optional
///     Defaults to Actual360.
/// compounding : Compounding, optional
///     Defaults to Simple.
/// frequency : Frequency, optional
///     Defaults to Annual.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct InterestRate {
    pub inner: QsRate<f64>,
}
#[pymethods]
impl InterestRate {
    #[new]
    #[pyo3(signature = (rate, day_counter = None, compounding = None, frequency = None))]
    fn new(
        rate: f64,
        day_counter: Option<&Bound<'_, PyAny>>,
        compounding: Option<&Bound<'_, PyAny>>,
        frequency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        finite(rate)?;
        Ok(Self {
            inner: QsRate::new(
                rate,
                compounding
                    .map(extract_compounding)
                    .transpose()?
                    .unwrap_or(Compounding::Simple),
                frequency
                    .map(extract_frequency)
                    .transpose()?
                    .unwrap_or(Frequency::Annual),
                day_counter
                    .map(extract_day_counter)
                    .transpose()?
                    .unwrap_or(DayCounter::Actual360),
            ),
        })
    }
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate()
    }
    #[getter]
    fn day_counter(&self) -> PyDayCounter {
        self.inner.day_counter().into()
    }
    #[getter]
    fn compounding(&self) -> PyCompounding {
        self.inner.compounding().into()
    }
    #[getter]
    fn frequency(&self) -> PyFrequency {
        self.inner.frequency().into()
    }
    fn compound_factor(&self, start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<f64> {
        Ok(self
            .inner
            .compound_factor(extract_date(start)?, extract_date(end)?))
    }
    fn discount_factor(&self, start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<f64> {
        Ok(self
            .inner
            .discount_factor(extract_date(start)?, extract_date(end)?))
    }
    /// Returns the native interest-rate value and rate definition.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::common::representation::to_dict(py, &self.inner)
    }

    /// Restores the native interest-rate dictionary.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: from_py(obj, "interest rate")?,
        })
    }
}

fn finite(value: f64) -> PyResult<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(crate::common::errors::invalid_value(
            "Coupon rates, spreads, fixings, and payoff constants must be finite",
        ))
    }
}

/// The native payoff expression evaluated on one index fixing.
///
/// Index and Const are leaves. Max, Min, Times, Plus, and Minus compose them.
/// Arithmetic operators also construct expressions; min() and max() apply bounds.
/// OptionEmbeddedCoupon pays notional * accrual * (payoff.eval(fixing) + spread).
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct PayoffOps {
    pub inner: QsPayoff,
}
impl PayoffOps {
    fn extract(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(payoff) = obj.extract::<Self>() {
            return Ok(payoff);
        }
        if let Ok(value) = obj.extract::<f64>() {
            finite(value)?;
            return Ok(Self {
                inner: QsPayoff::Const(value),
            });
        }
        Ok(Self {
            inner: from_py(obj, "payoff operations")?,
        })
    }
    fn binary(
        &self,
        other: &Bound<'_, PyAny>,
        op: fn(Box<QsPayoff>, Box<QsPayoff>) -> QsPayoff,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: op(
                Box::new(self.inner.clone()),
                Box::new(Self::extract(other)?.inner),
            ),
        })
    }
}
#[pymethods]
#[allow(non_snake_case)]
impl PayoffOps {
    #[classattr]
    fn Index() -> Self {
        Self {
            inner: QsPayoff::Index,
        }
    }
    #[staticmethod]
    fn Const(value: f64) -> PyResult<Self> {
        finite(value)?;
        Ok(Self {
            inner: QsPayoff::Const(value),
        })
    }
    #[staticmethod]
    fn Max(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::extract(left)?.binary(right, QsPayoff::Max)
    }
    #[staticmethod]
    fn Min(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::extract(left)?.binary(right, QsPayoff::Min)
    }
    #[staticmethod]
    fn Times(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::extract(left)?.binary(right, QsPayoff::Times)
    }
    #[staticmethod]
    fn Plus(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::extract(left)?.binary(right, QsPayoff::Plus)
    }
    #[staticmethod]
    fn Minus(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::extract(left)?.binary(right, QsPayoff::Minus)
    }
    fn min(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.binary(other, QsPayoff::Min)
    }
    fn max(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.binary(other, QsPayoff::Max)
    }
    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.binary(other, QsPayoff::Plus)
    }
    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: QsPayoff::Plus(
                Box::new(Self::extract(other)?.inner),
                Box::new(self.inner.clone()),
            ),
        })
    }
    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.binary(other, QsPayoff::Minus)
    }
    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: QsPayoff::Minus(
                Box::new(Self::extract(other)?.inner),
                Box::new(self.inner.clone()),
            ),
        })
    }
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.binary(other, QsPayoff::Times)
    }
    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.binary(other, QsPayoff::Times)
    }
    /// Evaluates the native expression at a decimal index fixing.
    fn eval(&self, index_fixing: f64) -> PyResult<f64> {
        finite(index_fixing)?;
        self.inner.eval(index_fixing).map_err(qs_err)
    }
    /// Reconstructs the native expression tree from its serialized representation.
    #[staticmethod]
    fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::extract(obj)
    }
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::common::representation::to_dict(py, &self.inner)
    }
}

/// Native fixed-rate coupon. Omit dates for a schedule-builder template.
///
/// Parameters
/// ----------
/// rate : InterestRate
///     Fixed annual rate and conventions.
/// notional : float, optional
///     Coupon principal; defaults to a unit template. Builders supply the schedule balance.
/// accrual_start_date, accrual_end_date, payment_date : Date, optional
///     Supply all three to create a dated coupon. Builders supply generated dates.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct FixedRateCoupon {
    pub inner: QsFixed<f64>,
}
#[pymethods]
impl FixedRateCoupon {
    #[new]
    #[pyo3(signature = (rate, notional = 1.0, accrual_start_date = None, accrual_end_date = None, payment_date = None))]
    fn new(
        rate: &Bound<'_, PyAny>,
        notional: f64,
        accrual_start_date: Option<&Bound<'_, PyAny>>,
        accrual_end_date: Option<&Bound<'_, PyAny>>,
        payment_date: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let rate = if let Ok(rate) = rate.extract::<InterestRate>() {
            rate.inner
        } else if let Ok(rate) = rate.extract::<f64>() {
            QsRate::new(
                rate,
                Compounding::Simple,
                Frequency::Annual,
                DayCounter::Actual360,
            )
        } else {
            from_py(rate, "interest rate")?
        };
        finite(rate.rate())?;
        let (start, end, payment) =
            coupon_dates(notional, accrual_start_date, accrual_end_date, payment_date)?;
        Ok(Self {
            inner: QsFixed::new(notional, Box::new(rate), start, end, payment),
        })
    }
    #[getter]
    fn rate(&self) -> InterestRate {
        InterestRate {
            inner: *self.inner.rate(),
        }
    }
}

/// Native floating coupon paying the index fixing plus spread.
///
/// Parameters
/// ----------
/// index : MarketIndex
///     Index used by the pricer to resolve historical fixings and future forwards.
/// spread : float, optional
///     Decimal annual spread added to the index; defaults to zero.
/// day_counter : DayCounter, optional
///     Accrual basis; defaults to the index convention.
/// notional : float, optional
///     Coupon principal; defaults to a unit schedule template.
/// accrual_start_date, accrual_end_date, payment_date : Date, optional
///     Supply all three for a dated coupon. Builders supply generated dates.
/// fixing : float, optional
///     Resolved observation for a standalone dated coupon.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct FloatingRateCoupon {
    pub inner: QsFloating<f64>,
}
#[pymethods]
impl FloatingRateCoupon {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (index, spread = 0.0, day_counter = None, notional = 1.0,
        accrual_start_date = None, accrual_end_date = None, payment_date = None, fixing = None))]
    fn new(
        index: &Bound<'_, PyAny>,
        spread: f64,
        day_counter: Option<&Bound<'_, PyAny>>,
        notional: f64,
        accrual_start_date: Option<&Bound<'_, PyAny>>,
        accrual_end_date: Option<&Bound<'_, PyAny>>,
        payment_date: Option<&Bound<'_, PyAny>>,
        fixing: Option<f64>,
    ) -> PyResult<Self> {
        finite(spread)?;
        let (start, end, payment) =
            coupon_dates(notional, accrual_start_date, accrual_end_date, payment_date)?;
        let mut coupon = QsFloating::new(
            notional,
            spread,
            extract_market_index(index)?,
            start,
            end,
            payment,
        );
        if let Some(day_counter) = day_counter {
            coupon = coupon.with_day_counter(extract_day_counter(day_counter)?);
        }
        if let Some(fixing) = fixing {
            finite(fixing)?;
            coupon.set_fixing(fixing);
        }
        Ok(Self { inner: coupon })
    }
    #[getter]
    fn index(&self) -> MarketIndex {
        MarketIndex {
            inner: self.inner.market_index().clone(),
        }
    }
    #[getter]
    fn spread(&self) -> f64 {
        self.inner.spread()
    }
    #[getter]
    fn fixing(&self) -> Option<f64> {
        self.inner.fixing()
    }
    #[getter]
    fn day_counter(&self) -> PyDayCounter {
        self.inner.day_counter().into()
    }
}

/// Native coupon with a custom payoff expression.
///
/// Parameters
/// ----------
/// index : MarketIndex
///     Observation supplied to the payoff expression.
/// payoff : PayoffOps
///     Annual rate expression. Include a lending margin inside this expression
///     when a cap or floor should apply to the all-in rate.
/// spread : float, optional
///     Additional spread added after evaluating the expression; defaults to zero.
/// day_counter : DayCounter, optional
///     Accrual basis; defaults to the index convention.
/// notional : float, optional
///     Coupon principal; defaults to a unit schedule template.
/// accrual_start_date, accrual_end_date, payment_date : Date, optional
///     Supply all three for a dated coupon. Builders supply generated dates.
/// fixing : float, optional
///     Resolved observation for a standalone dated coupon.
#[pyclass(module = "quantsupport", frozen, from_py_object)]
#[derive(Clone)]
pub struct OptionEmbeddedCoupon {
    pub inner: QsOption<f64>,
}
#[pymethods]
impl OptionEmbeddedCoupon {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (index, payoff, spread = 0.0, day_counter = None, notional = 1.0,
        accrual_start_date = None, accrual_end_date = None, payment_date = None, fixing = None))]
    fn new(
        index: &Bound<'_, PyAny>,
        payoff: &Bound<'_, PyAny>,
        spread: f64,
        day_counter: Option<&Bound<'_, PyAny>>,
        notional: f64,
        accrual_start_date: Option<&Bound<'_, PyAny>>,
        accrual_end_date: Option<&Bound<'_, PyAny>>,
        payment_date: Option<&Bound<'_, PyAny>>,
        fixing: Option<f64>,
    ) -> PyResult<Self> {
        finite(spread)?;
        let (start, end, payment) =
            coupon_dates(notional, accrual_start_date, accrual_end_date, payment_date)?;
        let mut coupon = QsOption::new(
            notional,
            extract_market_index(index)?,
            spread,
            start,
            end,
            payment,
            PayoffOps::extract(payoff)?.inner,
        );
        if let Some(day_counter) = day_counter {
            coupon = coupon.with_day_counter(extract_day_counter(day_counter)?);
        }
        CashflowType::OptionEmbeddedCoupon(coupon.clone())
            .validate_coupon()
            .map_err(qs_err)?;
        if let Some(fixing) = fixing {
            finite(fixing)?;
            coupon = coupon.with_fixing(fixing);
        }
        Ok(Self { inner: coupon })
    }
    #[getter]
    fn index(&self) -> MarketIndex {
        MarketIndex {
            inner: self.inner.market_index().clone(),
        }
    }
    #[getter]
    fn spread(&self) -> f64 {
        self.inner.spread()
    }
    #[getter]
    fn fixing(&self) -> Option<f64> {
        self.inner.fixing()
    }
    #[getter]
    fn day_counter(&self) -> PyResult<PyDayCounter> {
        self.inner.day_counter().map(Into::into).map_err(qs_err)
    }
    #[getter]
    fn payoff(&self) -> PayoffOps {
        PayoffOps {
            inner: self.inner.payoff_ops().clone(),
        }
    }
}

fn coupon_dates(
    notional: f64,
    start: Option<&Bound<'_, PyAny>>,
    end: Option<&Bound<'_, PyAny>>,
    payment: Option<&Bound<'_, PyAny>>,
) -> PyResult<(QsDate, QsDate, QsDate)> {
    if !notional.is_finite() || notional <= 0.0 {
        return Err(crate::common::errors::invalid_value(
            "Coupon notional must be positive and finite",
        ));
    }
    match (start, end, payment) {
        (None, None, None) => {
            let date = QsDate::new(1970, 1, 1);
            Ok((date, date, date))
        }
        (Some(start), Some(end), Some(payment)) => {
            let dates = (
                extract_date(start)?,
                extract_date(end)?,
                extract_date(payment)?,
            );
            if dates.1 <= dates.0 || dates.2 < dates.1 {
                return Err(crate::common::errors::invalid_value(
                    "Coupon dates require start < end <= payment",
                ));
            }
            Ok(dates)
        }
        _ => Err(crate::common::errors::invalid_value(
            "Supply all three coupon dates, or omit them for a schedule template",
        )),
    }
}

macro_rules! coupon_methods {
    ($name:ident,$variant:ident) => {
        #[pymethods]
        impl $name {
            /// Loads a tagged coupon specification returned by to_dict().
            #[staticmethod]
            fn from_dict(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
                match CouponDefinition::from_py(obj)?.inner {
                    CashflowType::$variant(inner) => Ok(Self { inner }),
                    _ => Err(pyo3::exceptions::PyTypeError::new_err(concat!(
                        "Expected ",
                        stringify!($name)
                    ))),
                }
            }
            #[getter]
            fn notional(&self) -> f64 {
                self.inner.notional()
            }
            #[getter]
            fn accrual_start_date(&self) -> Date {
                Date {
                    inner: self.inner.accrual_start_date(),
                }
            }
            #[getter]
            fn accrual_end_date(&self) -> Date {
                Date {
                    inner: self.inner.accrual_end_date(),
                }
            }
            #[getter]
            fn payment_date(&self) -> Date {
                Date {
                    inner: self.inner.payment_date(),
                }
            }
            fn amount(&self) -> PyResult<f64> {
                self.inner.amount().map_err(qs_err)
            }
            /// Earned interest in the clipped contractual overlap; reversed dates error.
            /// Resolved floating observations are required for nonzero accrual.
            fn accrued_amount(&self, start: &Bound<'_, PyAny>, end: &Bound<'_, PyAny>) -> PyResult<f64> {
                self.inner.accrued_amount(extract_date(start)?, extract_date(end)?).map_err(qs_err)
            }
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                pythonize::pythonize(
                    py,
                    &CouponDefinition {
                        inner: CashflowType::$variant(self.inner.clone()),
                    }
                    .specification()?,
                )
                .map_err(qs_err)
            }
        }
    };
}
coupon_methods!(FixedRateCoupon, FixedRateCoupon);
coupon_methods!(FloatingRateCoupon, FloatingRateCoupon);
coupon_methods!(OptionEmbeddedCoupon, OptionEmbeddedCoupon);

/// Serialization adapter around the library's native cashflow enum.
#[derive(Clone)]
pub struct CouponDefinition {
    pub inner: CashflowType<f64>,
}
impl CouponDefinition {
    /// Read a flat coupon sequence and identify the failing entry in errors.
    pub fn list_from_py(obj: &Bound<'_, PyAny>, path: &str) -> PyResult<Vec<Self>> {
        let values = obj.extract::<Vec<Bound<'_, PyAny>>>().map_err(|cause| {
            let error = crate::common::errors::invalid_type(
                obj,
                "a flat sequence of coupon objects or tagged dicts",
                None,
            );
            error.set_cause(obj.py(), Some(cause));
            at_path(obj.py(), path, error)
        })?;
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                Self::from_py(value)
                    .map_err(|error| at_path(obj.py(), &format!("{path}[{index}]"), error))
            })
            .collect()
    }

    /// Returns an independent Python coupon with the native contractual terms.
    pub fn to_python(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match &self.inner {
            CashflowType::FixedRateCoupon(inner) => Py::new(
                py,
                FixedRateCoupon {
                    inner: inner.clone(),
                },
            )
            .map(Py::into_any),
            CashflowType::FloatingRateCoupon(inner) => Py::new(
                py,
                FloatingRateCoupon {
                    inner: inner.clone(),
                },
            )
            .map(Py::into_any),
            CashflowType::OptionEmbeddedCoupon(inner) => Py::new(
                py,
                OptionEmbeddedCoupon {
                    inner: inner.clone(),
                },
            )
            .map(Py::into_any),
            _ => Err(crate::common::errors::invalid_value(
                "Expected a contractual coupon",
            )),
        }
    }

    pub fn from_py(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(c) = obj.extract::<FixedRateCoupon>() {
            return Ok(Self {
                inner: CashflowType::FixedRateCoupon(c.inner),
            });
        }
        if let Ok(c) = obj.extract::<FloatingRateCoupon>() {
            return Ok(Self {
                inner: CashflowType::FloatingRateCoupon(c.inner),
            });
        }
        if let Ok(c) = obj.extract::<OptionEmbeddedCoupon>() {
            return Ok(Self {
                inner: CashflowType::OptionEmbeddedCoupon(c.inner),
            });
        }
        let spec = expect_dict(
            obj,
            "coupon (expected a coupon object or tagged dict; use a flat coupon list)",
        )?;
        if spec.len() != 1 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "A coupon specification requires one cashflow kind",
            ));
        }
        let (kind, terms) = spec
            .iter()
            .next()
            .ok_or_else(|| crate::common::errors::invalid_value("Missing coupon kind"))?;
        let kind: String = kind.extract()?;
        let terms = expect_dict(&terms, &format!("{kind} coupon terms"))
            .map_err(|error| crate::common::errors::context(obj.py(), error, None, Some(&kind)))?;
        let py = obj.py();
        // Compact legacy JSON remains readable; all new output preserves native payoff trees.
        if (kind == "Floating" || kind == "FloatingRateCoupon")
            && (terms.contains("floor")? || terms.contains("cap")?)
        {
            let index = terms
                .get_item("index")?
                .ok_or_else(|| crate::common::errors::invalid_value("Missing coupon index"))?;
            let spread = terms
                .get_item("spread")?
                .map(|v| v.extract::<f64>())
                .transpose()?
                .unwrap_or(0.0);
            let bound = |name| -> PyResult<Option<f64>> {
                terms
                    .get_item(name)?
                    .filter(|v| !v.is_none())
                    .map(|v| v.extract())
                    .transpose()
            };
            let inner = CashflowType::floating(
                extract_market_index(&index)?,
                spread,
                bound("floor")?,
                bound("cap")?,
            );
            inner.validate_coupon().map_err(qs_err)?;
            return Ok(Self { inner });
        }
        let value = match kind.as_str() {
            "Fixed" | "FixedRateCoupon" => {
                py.get_type::<FixedRateCoupon>().call((), Some(terms))?
            }
            "Floating" | "FloatingRateCoupon" => {
                py.get_type::<FloatingRateCoupon>().call((), Some(terms))?
            }
            "OptionEmbeddedCoupon" => py
                .get_type::<OptionEmbeddedCoupon>()
                .call((), Some(terms))?,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unsupported coupon cashflow kind '{kind}'"
                )))
            }
        };
        Self::from_py(&value)
    }
    pub fn specification(&self) -> PyResult<serde_json::Value> {
        let (kind, mut terms) = match &self.inner {
            CashflowType::FixedRateCoupon(c) => (
                "FixedRateCoupon",
                serde_json::json!({"rate":c.rate(),"notional":c.notional(),"accrual_start_date":c.accrual_start_date(),"accrual_end_date":c.accrual_end_date(),"payment_date":c.payment_date()}),
            ),
            CashflowType::FloatingRateCoupon(c) => (
                "FloatingRateCoupon",
                serde_json::json!({"index":c.market_index(),"spread":c.spread(),"day_counter":c.day_counter(),"fixing":c.fixing(),"notional":c.notional(),"accrual_start_date":c.accrual_start_date(),"accrual_end_date":c.accrual_end_date(),"payment_date":c.payment_date()}),
            ),
            CashflowType::OptionEmbeddedCoupon(c) => (
                "OptionEmbeddedCoupon",
                serde_json::json!({"index":c.market_index(),"spread":c.spread(),"payoff":c.payoff_ops(),"day_counter":c.day_counter().map_err(qs_err)?,"fixing":c.fixing(),"notional":c.notional(),"accrual_start_date":c.accrual_start_date(),"accrual_end_date":c.accrual_end_date(),"payment_date":c.payment_date()}),
            ),
            _ => {
                return Err(crate::common::errors::invalid_value(
                    "Expected a native coupon cashflow",
                ))
            }
        };
        // Template dates are placeholders rather than a dated accrual interval.
        if terms["accrual_start_date"] == terms["accrual_end_date"] {
            for name in ["accrual_start_date", "accrual_end_date", "payment_date"] {
                terms.as_object_mut().unwrap().remove(name);
            }
        }
        Ok(serde_json::json!({kind:terms}))
    }
}

crate::common::representation::impl_dict_protocol!(
    InterestRate,
    PayoffOps,
    FixedRateCoupon,
    FloatingRateCoupon,
    OptionEmbeddedCoupon,
);
