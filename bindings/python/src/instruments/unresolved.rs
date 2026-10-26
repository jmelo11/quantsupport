//! Context-dependent product terms held by a Python Trade until evaluation.

use crate::{
    common::conversion::{
        extract_business_day_convention, extract_calendar, extract_date, extract_market_index,
        extract_period, qs_err,
    },
    pricing::{context::PricingContext, extension::Instrument},
};
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple, PyType},
};
use quantsupport::prelude::*;
use quantsupport::time::calendars::nullcalendar::NullCalendar;
use std::collections::HashMap;

/// Product parameters containing explicit tenor fields instead of calendar dates.
/// Create through a product's ``from_tenors`` factory and put it in ``Trade``.
/// PricingContext.evaluate resolves it against that evaluation's market.
#[pyclass(module="quantsupport", extends=Instrument, unsendable)]
pub struct UnresolvedInstrument {
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

pub fn from_tenors<'py>(
    cls: &Bound<'py, PyType>,
    parameters: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
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
    ];
    let mut kind = cls.name()?.to_string();
    for base in cls.getattr("__mro__")?.try_iter()? {
        let name = base?.getattr("__name__")?.extract::<String>()?;
        if kinds.contains(&name.as_str()) {
            kind = name;
            break;
        }
    }
    let parameters = copy_parameters(parameters)?;
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
            extract_date(&value)?;
        }
        if let Some(value) = tenor {
            extract_period(&value)?;
        }
    }
    let identifier = parameters
        .get_item("identifier")?
        .or(parameters.get_item("name")?)
        .map(|v| v.extract::<String>())
        .transpose()?
        .unwrap_or_else(|| kind.clone());
    Py::new(
        cls.py(),
        PyClassInitializer::from(Instrument::new(identifier)).add_subclass(UnresolvedInstrument {
            product_type: cls.clone().unbind(),
            parameters: parameters.unbind(),
            kind,
        }),
    )
    .map(|v| v.into_bound(cls.py()).into_any())
}

impl UnresolvedInstrument {
    pub fn resolve_with_market<'py>(
        &self,
        py: Python<'py>,
        market: &impl MarketDataProvider,
    ) -> PyResult<Bound<'py, PyAny>> {
        let parameters = copy_parameters(self.parameters.bind(py))?;
        let reference = market.evaluation_date();
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
        let context =
            ResolutionContext::new(market).with_date_conventions(&calendar, convention, eom);
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
            let resolved = context
                .resolve_date(date, tenor, anchor, field.date)
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
                let date = context
                    .advance_date(start, extract_period(&values[0])?)
                    .map_err(qs_err)?;
                let strike = values[values.len() - 1].extract::<f64>()?;
                let inner = if values.len() == 3 {
                    CallScheduleEntry::window(
                        date,
                        context
                            .advance_date(start, extract_period(&values[1])?)
                            .map_err(qs_err)?,
                        strike,
                    )
                } else {
                    CallScheduleEntry::new(date, strike)
                };
                inner.validate().map_err(qs_err)?;
                calls.push(Py::new(
                    py,
                    crate::instruments::products::CallScheduleEntry { inner },
                )?);
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
                    context
                        .advance_date(dates["start_date"], extract_period(&tenor)?)
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
                    context
                        .advance_date(reference, extract_period(&tenor)?)
                        .map_err(qs_err)?
                        .to_string(),
                )?;
                event.set_item("code", code)?;
                resolved.push(event);
            }
            parameters.set_item("events", resolved)?;
            parameters.del_item("event_tenors")?;
        }
        let instrument = self
            .product_type
            .bind(py)
            .call_method1("from_dict", (&parameters,))?;
        if let Ok(mut loan) = instrument.extract::<PyRefMut<'_, crate::instruments::loan::Loan>>() {
            loan.freeze_resolved_contract(market)?;
        }
        Ok(instrument)
    }
}

#[pymethods]
impl UnresolvedInstrument {
    /// Resolves tenor inputs into a concrete product using this pricing context.
    fn resolve_instrument<'py>(
        &self,
        py: Python<'py>,
        context: PyRef<'_, PricingContext>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.resolve_with_market(py, &context.market_view()?)
    }

    /// Original constructor parameters, copied so callers cannot mutate the trade.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let result = PyDict::new(py);
        result.set_item("instrument_type", self.kind.clone())?;
        result.set_item("parameters", copy_parameters(self.parameters.bind(py))?)?;
        Ok(result)
    }
}

crate::common::representation::impl_dict_protocol!(UnresolvedInstrument);
