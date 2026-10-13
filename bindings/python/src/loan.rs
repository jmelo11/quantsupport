//! Native contractual loans and position conversion.

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};
use quantsupport::prelude::{
    DualFwd, Loan as QsLoan, LoanRateDefinition, LoanRepaymentStructure, LoanTrade, MakeLoan,
    PrepaymentModelConfiguration,
};

use crate::conv::{
    dataframe, extract_business_day_convention, extract_calendar, extract_currency, extract_date,
    extract_day_counter, extract_frequency, extract_market_index, extract_period, from_py, qs_err,
};
use crate::extension::{Instrument, Trade};

/// A native loan contract; rates and CPRs are fractions, not percentages.
#[pyclass(module = "quantsupport", extends = Instrument, subclass, dict, unsendable, from_py_object)]
#[derive(Clone)]
pub struct Loan {
    pub inner: QsLoan<DualFwd>,
    builder: MakeLoan<DualFwd>,
}

impl Loan {
    pub fn build_trade(&self, position: &Trade) -> PyResult<LoanTrade<DualFwd>> {
        let loan = match position.notional() {
            Some(notional) => self
                .builder
                .clone()
                .with_notional(notional)
                .build()
                .map_err(qs_err)?,
            None => self.inner.clone(),
        };
        Ok(LoanTrade::new(loan, position.trade_date(), position.side()))
    }
}

#[pymethods]
impl Loan {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (identifier, start_date, maturity_date, notional, rate, currency,
        discount_index, repayment = None, *, payment_frequency = None, fixing_frequency = None,
        day_counter = None, prepayment = None, credit_index = None, recovery = None,
        fixing_lag = None, index_tenor = None, calendar = None, business_day_convention = None,
        amortization_rate = None))]
    fn new(
        identifier: String,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        notional: f64,
        rate: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
        discount_index: &Bound<'_, PyAny>,
        repayment: Option<&Bound<'_, PyAny>>,
        payment_frequency: Option<&Bound<'_, PyAny>>,
        fixing_frequency: Option<&Bound<'_, PyAny>>,
        day_counter: Option<&Bound<'_, PyAny>>,
        prepayment: Option<&Bound<'_, PyAny>>,
        credit_index: Option<&Bound<'_, PyAny>>,
        recovery: Option<f64>,
        fixing_lag: Option<i32>,
        index_tenor: Option<&Bound<'_, PyAny>>,
        calendar: Option<&Bound<'_, PyAny>>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
        amortization_rate: Option<f64>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let rate = match rate.extract::<f64>() {
            Ok(value) => LoanRateDefinition::Fixed(value),
            Err(_) => from_py(rate, "loan rate")?,
        };
        let mut builder = MakeLoan::default()
            .with_identifier(identifier.clone())
            .with_start_date(extract_date(start_date)?)
            .with_maturity_date(extract_date(maturity_date)?)
            .with_notional(notional)
            .with_rate(rate)
            .with_currency(extract_currency(currency)?)
            .with_discount_index(extract_market_index(discount_index)?);
        if let Some(value) = repayment {
            builder = builder.with_repayment_structure(from_py::<LoanRepaymentStructure>(
                value,
                "loan repayment",
            )?);
        }
        if let Some(value) = payment_frequency {
            builder = builder.with_payment_frequency(extract_frequency(value)?);
        }
        if let Some(value) = fixing_frequency {
            builder = builder.with_fixing_frequency(extract_frequency(value)?);
        }
        if let Some(value) = day_counter {
            builder = builder.with_day_counter(extract_day_counter(value)?);
        }
        if let Some(value) = prepayment {
            builder = builder.with_prepayment(from_py::<PrepaymentModelConfiguration>(
                value,
                "prepayment",
            )?);
        }
        if let Some(value) = credit_index {
            builder = builder.with_credit_index(extract_market_index(value)?);
        }
        if let Some(value) = recovery {
            builder = builder.with_recovery(value);
        }
        if let Some(value) = fixing_lag {
            builder = builder.with_fixing_lag(value);
        }
        if let Some(value) = index_tenor {
            builder = builder.with_index_tenor(extract_period(value)?);
        }
        if let Some(value) = calendar {
            builder = builder.with_calendar(extract_calendar(value)?);
        }
        if let Some(value) = business_day_convention {
            builder = builder.with_business_day_convention(extract_business_day_convention(value)?);
        }
        if let Some(value) = amortization_rate {
            builder = builder.with_amortization_rate(value);
        }
        let inner = builder.clone().build().map_err(qs_err)?;
        Ok(PyClassInitializer::from(Instrument::new(identifier))
            .add_subclass(Self { inner, builder }))
    }

    /// Required index observations in contractual reset order.
    fn fixing_requests<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut seen = HashSet::new();
        let requests: Vec<_> = self
            .inner
            .observations()
            .iter()
            .flatten()
            .filter(|request| seen.insert((request.market_index(), request.fixing_date())))
            .collect();
        let market_index: Vec<_> = requests
            .iter()
            .map(|request| request.market_index().to_string())
            .collect();
        let fixing_date: Vec<_> = requests
            .iter()
            .map(|request| request.fixing_date().to_string())
            .collect();
        let start_date: Vec<_> = requests
            .iter()
            .map(|request| request.start_date().map(|date| date.to_string()))
            .collect();
        let end_date: Vec<_> = requests
            .iter()
            .map(|request| request.end_date().map(|date| date.to_string()))
            .collect();
        dataframe(
            py,
            &[
                ("market_index", market_index.into_pyobject(py)?.into_any()),
                ("fixing_date", fixing_date.into_pyobject(py)?.into_any()),
                ("start_date", start_date.into_pyobject(py)?.into_any()),
                ("end_date", end_date.into_pyobject(py)?.into_any()),
            ],
        )
    }

    #[classmethod]
    fn from_dict<'py>(
        cls: &Bound<'py, PyType>,
        obj: &Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyAny>> {
        cls.call((), Some(obj))
    }

    #[classmethod]
    fn from_json<'py>(cls: &Bound<'py, PyType>, path: &str) -> PyResult<Bound<'py, PyAny>> {
        let value: serde_json::Value = crate::conv::from_json_file(path, "loan")?;
        let obj = pythonize::pythonize(cls.py(), &value).map_err(qs_err)?;
        cls.call((), Some(obj.cast::<PyDict>()?))
    }

    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        pythonize::pythonize(py, &self.builder).map_err(qs_err)
    }

    /// Exposes serialized construction parameters.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let terms = self.to_dict(py)?;
        terms.cast::<PyDict>()?.get_item(name)?.ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err(format!("Loan has no attribute '{name}'"))
        })
    }
}
