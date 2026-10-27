//! Explicit Python makers; construction completes before a product can be traded.

use crate::{
    common::conversion::{
        extract_business_day_convention, extract_calendar, extract_date, extract_market_index,
        extract_period, from_py, qs_err,
    },
    instruments::{coupons::CouponDefinition, products::CallScheduleEntry as PyCallScheduleEntry},
    pricing::context::PricingContext,
};
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple, PyType},
};
use quantsupport::{
    prelude::{
        BusinessDayConvention, Calendar, CallScheduleEntry, CashflowType, Date, IsCalendar,
        LoanRepaymentStructure, Period, PrepaymentModelConfiguration, QSError, Result as QsResult,
    },
    time::calendars::nullcalendar::NullCalendar,
};
use std::{collections::HashMap, ffi::CString};

/// Reusable construction parameters shared by the named product makers.
/// Makers are not instruments and cannot be placed into a Trade.
#[pyclass(module = "quantsupport", unsendable, subclass)]
pub struct MakeInstrument {
    product_type: Py<PyType>,
    parameters: Py<PyDict>,
    kind: String,
}

// A schema describes constructor field names and their product-specific anchors.
// It contains no dates or tenors and does not add another date representation.
struct DateField {
    date: &'static str,
    tenor: &'static str,
    anchor: &'static str,
    optional: bool,
}

fn date_fields(kind: &str) -> Vec<DateField> {
    let mut fields = match kind {
        "FxForward" | "EquityForward" => {
            vec![("delivery_date", "delivery_tenor", "reference", false)]
        }
        "FxOption" | "EquityOption" => vec![("expiry_date", "expiry_tenor", "reference", false)],
        "EuropeanSwaption" => vec![
            ("expiry_date", "expiry_tenor", "reference", false),
            ("maturity_date", "tenor", "expiry_date", false),
        ],
        "RateFutures" => vec![
            ("start_date", "start_tenor", "reference", false),
            ("end_date", "tenor", "start_date", false),
        ],
        "CapletFloorlet" => vec![
            ("start_accrual_date", "start_tenor", "reference", false),
            ("end_accrual_date", "tenor", "start_accrual_date", false),
            ("fixing_date", "fixing_tenor", "start_accrual_date", true),
            ("payment_date", "payment_tenor", "end_accrual_date", true),
        ],
        "ScriptedProduct" => vec![],
        "Trade" => vec![("trade_date", "trade_tenor", "reference", false)],
        _ => vec![
            ("start_date", "start_tenor", "reference", false),
            ("maturity_date", "tenor", "start_date", false),
        ],
    };
    if matches!(kind, "FixedRateBond" | "SteppedCouponBond") {
        fields.push((
            "first_coupon_date",
            "first_coupon_tenor",
            "start_date",
            true,
        ));
    }
    if kind == "SteppedCouponBond" {
        fields.push((
            "next_to_last_coupon_date",
            "next_to_last_coupon_tenor",
            "maturity_date",
            true,
        ));
    }
    if kind == "FxForward" {
        fields.push(("fixing_date", "fixing_tenor", "reference", true));
    }
    fields
        .into_iter()
        .map(|(date, tenor, anchor, optional)| DateField {
            date,
            tenor,
            anchor,
            optional,
        })
        .collect()
}

// Copy Python containers without asking native value wrappers to support pickle.
fn copy_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(dict) = value.cast::<PyDict>() {
        return Ok(copy_parameters(dict)?.into_any());
    }
    if let Ok(list) = value.cast::<PyList>() {
        let values = list
            .iter()
            .map(|item| copy_value(&item))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(PyList::new(value.py(), values)?.into_any());
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        let values = tuple
            .iter()
            .map(|item| copy_value(&item))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(PyTuple::new(value.py(), values)?.into_any());
    }
    Ok(value.clone())
}

fn copy_parameters<'py>(parameters: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyDict>> {
    let copy = PyDict::new(parameters.py());
    for (key, value) in parameters.iter() {
        copy.set_item(key, copy_value(&value)?)?;
    }
    Ok(copy)
}

fn resolve_date(
    date: Option<Date>,
    tenor: Option<Period>,
    anchor: Date,
    label: &str,
    calendar: &Calendar,
    convention: BusinessDayConvention,
    eom: bool,
) -> QsResult<Date> {
    match (date, tenor) {
        (Some(date), None) if date != Date::empty() => Ok(date),
        (Some(_), None) => Err(QSError::InvalidValueErr(format!(
            "{label} must not be empty"
        ))),
        (None, Some(tenor)) => calendar.advance(anchor, tenor, Some(convention), eom),
        (Some(_), Some(_)) => Err(QSError::InvalidValueErr(format!(
            "Supply {label} or its tenor, not both"
        ))),
        (None, None) => Err(QSError::ValueNotSetErr(label.into())),
    }
}

impl MakeInstrument {
    fn unresolved_inputs(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        let parameters = self.parameters.bind(py);
        let mut missing = Vec::new();
        for field in date_fields(&self.kind) {
            if parameters
                .get_item(field.tenor)?
                .is_some_and(|value| !value.is_none())
            {
                missing.push(format!(
                    "an explicit {} instead of {}",
                    field.date, field.tenor
                ));
            }
        }
        for (relative, absolute) in [
            ("event_tenors", "events"),
            ("call_schedule_tenors", "call_schedule"),
            ("prepayment_tenors", "prepayment"),
        ] {
            if parameters
                .get_item(relative)?
                .is_some_and(|value| !value.is_none() && !value.is_empty().unwrap_or(false))
            {
                missing.push(format!("explicit {absolute} instead of {relative}"));
            }
        }
        if self.kind == "Loan"
            && !parameters
                .get_item("amortization_rate")?
                .is_some_and(|value| !value.is_none())
        {
            let repayment = parameters
                .get_item("repayment")?
                .filter(|value| !value.is_none())
                .map(|value| from_py::<LoanRepaymentStructure>(&value, "loan repayment"))
                .transpose()?;
            if !matches!(repayment, Some(LoanRepaymentStructure::Bullet)) {
                if let Some(coupons) = parameters
                    .get_item("coupons")?
                    .filter(|value| !value.is_none())
                {
                    let coupons = CouponDefinition::list_from_py(&coupons, "Loan.coupons")?;
                    let needs_observation =
                        coupons.first().is_some_and(|coupon| match &coupon.inner {
                            CashflowType::FloatingRateCoupon(coupon) => coupon.fixing().is_none(),
                            CashflowType::OptionEmbeddedCoupon(coupon) => coupon.fixing().is_none(),
                            _ => false,
                        });
                    if needs_observation {
                        missing.push("an initial floating coupon observation (or an explicit amortization_rate or first coupon fixing)".into());
                    }
                }
            }
        }
        Ok(missing)
    }

    fn resolve_parameters<'py>(
        &self,
        py: Python<'py>,
        pricing_context: &Bound<'py, PricingContext>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let parameters = copy_parameters(self.parameters.bind(py))?;
        let context_ref = pricing_context.borrow();
        let _market = context_ref.market_view()?;
        let reference = parameters
            .get_item("reference_date")?
            .filter(|v| !v.is_none())
            .map(|v| extract_date(&v))
            .transpose()?
            .unwrap_or_else(|| context_ref.base_context().evaluation_date());
        let calendar = match parameters.get_item("calendar")?.filter(|v| !v.is_none()) {
            Some(value) => extract_calendar(&value)?,
            None if matches!(self.kind.as_str(), "Swap" | "EuropeanSwaption") => {
                let index = parameters.get_item("market_index")?.ok_or_else(|| {
                    pyo3::exceptions::PyValueError::new_err("market_index is required")
                })?;
                extract_market_index(&index)?
                    .rate_index_details()
                    .map_err(qs_err)?
                    .calendar()
            }
            None => Calendar::NullCalendar(NullCalendar::new()),
        };
        let default = if matches!(self.kind.as_str(), "Swap" | "EuropeanSwaption" | "CapFloor") {
            BusinessDayConvention::ModifiedFollowing
        } else {
            BusinessDayConvention::Unadjusted
        };
        let convention = parameters
            .get_item("business_day_convention")?
            .filter(|v| !v.is_none())
            .map(|v| extract_business_day_convention(&v))
            .transpose()?
            .unwrap_or(default);
        let eom = parameters
            .get_item("end_of_month")?
            .filter(|v| !v.is_none())
            .map(|v| v.extract::<bool>())
            .transpose()?
            .unwrap_or(false);
        let mut dates = HashMap::from([("reference", reference)]);
        for field in date_fields(&self.kind) {
            let date = parameters
                .get_item(field.date)?
                .filter(|v| !v.is_none())
                .map(|v| extract_date(&v))
                .transpose()?;
            let tenor = parameters
                .get_item(field.tenor)?
                .filter(|v| !v.is_none())
                .map(|v| extract_period(&v))
                .transpose()?;
            if parameters.contains(field.tenor)? {
                parameters.del_item(field.tenor)?;
            }
            let anchor = *dates.get(field.anchor).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "missing date anchor {}",
                    field.anchor
                ))
            })?;
            if date.is_none() && tenor.is_none() && field.optional {
                if self.kind == "CapletFloorlet" {
                    parameters.set_item(field.date, anchor.to_string())?;
                    dates.insert(field.date, anchor);
                }
                continue;
            }
            let resolved =
                resolve_date(date, tenor, anchor, field.date, &calendar, convention, eom)
                    .map_err(qs_err)?;
            dates.insert(field.date, resolved);
            parameters.set_item(field.date, resolved.to_string())?;
        }
        if let Some(points) = parameters.get_item("call_schedule_tenors")? {
            let mut calls = Vec::new();
            let start = dates["start_date"];
            for item in points.try_iter()? {
                let item = item?;
                let values = item.extract::<Vec<Bound<'_, PyAny>>>()?;
                if values.len() != 2 && values.len() != 3 {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "call entries require (tenor, strike) or (start_tenor, end_tenor, strike)",
                    ));
                }
                let date = calendar
                    .advance(start, extract_period(&values[0])?, Some(convention), eom)
                    .map_err(qs_err)?;
                let strike = values[values.len() - 1].extract::<f64>()?;
                let inner = if values.len() == 3 {
                    CallScheduleEntry::window(
                        date,
                        calendar
                            .advance(start, extract_period(&values[1])?, Some(convention), eom)
                            .map_err(qs_err)?,
                        strike,
                    )
                } else {
                    CallScheduleEntry::new(date, strike)
                };
                inner.validate().map_err(qs_err)?;
                calls.push(Py::new(py, PyCallScheduleEntry { inner })?);
            }
            parameters.set_item("call_schedule", calls)?;
            parameters.del_item("call_schedule_tenors")?;
        }
        if let Some(points) = parameters.get_item("prepayment_tenors")? {
            let mut values = Vec::new();
            for item in points.try_iter()? {
                let item = item?;
                let (tenor, rate) = item.extract::<(Bound<'_, PyAny>, f64)>()?;
                values.push((
                    calendar
                        .advance(
                            dates["start_date"],
                            extract_period(&tenor)?,
                            Some(convention),
                            eom,
                        )
                        .map_err(qs_err)?,
                    rate,
                ));
            }
            let configuration = PrepaymentModelConfiguration::CprCurve(values);
            parameters.set_item(
                "prepayment",
                pythonize::pythonize(py, &configuration).map_err(qs_err)?,
            )?;
            parameters.del_item("prepayment_tenors")?;
        }
        if let Some(events) = parameters.get_item("event_tenors")? {
            let mut resolved = Vec::new();
            for item in events.try_iter()? {
                let item = item?;
                let (tenor, code) = item.extract::<(Bound<'_, PyAny>, String)>()?;
                let event = PyDict::new(py);
                event.set_item(
                    "date",
                    calendar
                        .advance(reference, extract_period(&tenor)?, Some(convention), eom)
                        .map_err(qs_err)?
                        .to_string(),
                )?;
                event.set_item("code", code)?;
                resolved.push(event);
            }
            parameters.set_item("events", resolved)?;
            parameters.del_item("event_tenors")?;
        }
        if self.kind == "ScriptedProduct" {
            if parameters.contains("reference_date")? {
                parameters.del_item("reference_date")?;
            }
            return Ok(parameters);
        }
        if self.kind == "Loan" {
            parameters.set_item("context", pricing_context)?;
        }
        // Calendar controls date construction even for product constructors that
        // expose only their concrete schedule dates.
        let inspect = py.import("inspect")?;
        let signature = inspect.call_method1("signature", (self.product_type.bind(py),))?;
        let accepted = signature.getattr("parameters")?;
        for field in [
            "calendar",
            "business_day_convention",
            "end_of_month",
            "reference_date",
        ] {
            if parameters.contains(field)? && !accepted.contains(field)? {
                parameters.del_item(field)?;
            }
        }
        Ok(parameters)
    }
}

#[pymethods]
impl MakeInstrument {
    #[new]
    fn new(product_type: &Bound<'_, PyType>, parameters: &Bound<'_, PyDict>) -> PyResult<Self> {
        let kinds = [
            "Swap",
            "CrossCurrencySwap",
            "BasisSwap",
            "FixFloatCrossCurrencySwap",
            "FixedRateBond",
            "SteppedCouponBond",
            "FloatingRateNote",
            "FixedRateDeposit",
            "FxForward",
            "FxOption",
            "EquityOption",
            "CreditDefaultSwap",
            "CapFloor",
            "CapletFloorlet",
            "RateFutures",
            "EquityForward",
            "EuropeanSwaption",
            "Loan",
            "ScriptedProduct",
            "Trade",
        ];
        let mut kind = product_type.name()?.to_string();
        for base in product_type.getattr("__mro__")?.try_iter()? {
            let name = base?.getattr("__name__")?.extract::<String>()?;
            if kinds.contains(&name.as_str()) {
                kind = name;
                break;
            }
        }
        let parameters = copy_parameters(parameters)?;
        if parameters.contains("context")? {
            return Err(qs_err(QSError::InvalidValueErr(
                "Supply the context to build_using, not as a maker parameter".into(),
            )));
        }
        for (absolute, relative) in [
            ("call_schedule", "call_schedule_tenors"),
            ("prepayment", "prepayment_tenors"),
            ("events", "event_tenors"),
        ] {
            if parameters
                .get_item(absolute)?
                .is_some_and(|value| !value.is_none())
                && parameters.contains(relative)?
            {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "supply {absolute} or {relative}, not both"
                )));
            }
        }
        if let Some(value) = parameters.get_item("maturity_tenor")? {
            if parameters.contains("tenor")? {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "supply tenor or maturity_tenor, not both",
                ));
            }
            parameters.set_item("tenor", value)?;
            parameters.del_item("maturity_tenor")?;
        }
        for field in date_fields(&kind) {
            let date = parameters.get_item(field.date)?.filter(|v| !v.is_none());
            let tenor = parameters.get_item(field.tenor)?.filter(|v| !v.is_none());
            if date.is_some() && tenor.is_some() {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "supply {} or {}, not both",
                    field.date, field.tenor
                )));
            }
            if let Some(value) = date {
                if extract_date(&value)? == Date::empty() {
                    return Err(qs_err(QSError::InvalidValueErr(format!(
                        "{} must not be empty",
                        field.date
                    ))));
                }
            }
            if let Some(value) = tenor {
                extract_period(&value)?;
            }
        }
        Ok(Self {
            product_type: product_type.clone().unbind(),
            parameters: parameters.unbind(),
            kind,
        })
    }

    /// Whether date inputs and required market observations are resolved.
    /// Missing or invalid contractual terms are still validated by build.
    fn is_resolved(&self, py: Python<'_>) -> PyResult<bool> {
        Ok(self.unresolved_inputs(py)?.is_empty())
    }

    /// Builds a concrete product from the supplied parameters.
    /// Supply missing inputs explicitly or call build_using(context).
    fn build<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if !self.is_resolved(py)? {
            return Err(qs_err(QSError::UnresolvedProductErr(format!(
                "{} requires {}. Supply these inputs explicitly or call maker.build_using(context) with an active PricingContext",
                self.kind, self.unresolved_inputs(py)?.join(" and "),
            ))));
        }
        let parameters = copy_parameters(self.parameters.bind(py))?;
        for field in date_fields(&self.kind) {
            if parameters.contains(field.tenor)? {
                parameters.del_item(field.tenor)?;
            }
            if self.kind == "CapletFloorlet"
                && field.optional
                && parameters
                    .get_item(field.date)?
                    .is_none_or(|value| value.is_none())
            {
                let anchor = parameters
                    .get_item(field.anchor)?
                    .ok_or_else(|| qs_err(QSError::ValueNotSetErr(field.anchor.into())))?;
                parameters.set_item(field.date, anchor)?;
            }
        }
        for field in ["event_tenors", "call_schedule_tenors", "prepayment_tenors"] {
            if parameters.contains(field)? {
                parameters.del_item(field)?;
            }
        }
        if self.kind == "ScriptedProduct" {
            if parameters.contains("reference_date")? {
                parameters.del_item("reference_date")?;
            }
            return self
                .product_type
                .bind(py)
                .call_method1("from_dict", (&parameters,));
        }
        let inspect = py.import("inspect")?;
        let signature = inspect.call_method1("signature", (self.product_type.bind(py),))?;
        let accepted = signature.getattr("parameters")?;
        for field in [
            "calendar",
            "business_day_convention",
            "end_of_month",
            "reference_date",
        ] {
            if parameters.contains(field)? && !accepted.contains(field)? {
                parameters.del_item(field)?;
            }
        }
        self.product_type.bind(py).call((), Some(&parameters))
    }

    /// Builds a concrete product using an explicit active pricing context.
    fn build_using<'py>(
        &self,
        py: Python<'py>,
        context: &Bound<'py, PricingContext>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let parameters = self.resolve_parameters(py, context)?;
        if self.kind == "ScriptedProduct" {
            self.product_type
                .bind(py)
                .call_method1("from_dict", (&parameters,))
        } else {
            self.product_type.bind(py).call((), Some(&parameters))
        }
    }

    /// Returns a copy of the maker's construction parameters.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        copy_parameters(self.parameters.bind(py))
    }
}

/// Registers named maker classes with explicit construction methods.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<MakeInstrument>()?;
    let source = CString::new(include_str!(concat!(env!("OUT_DIR"), "/_maker_support.py")))
        .expect("maker definitions contain no NUL bytes");
    module.py().run(&source, Some(&module.dict()), None)?;
    let makers = module
        .getattr("_initialize_makers")?
        .call1((module.dict(),))?;
    for (name, maker) in makers.cast::<PyDict>()?.iter() {
        module.dict().set_item(name, maker)?;
    }
    Ok(())
}

crate::common::representation::impl_dict_protocol!(MakeInstrument);
