use pyo3::prelude::*;
use quantsupport::prelude::{
    ADForward, LoanPricer as QsLoanPricer, MarketIndex, Pricer, Request, Tape,
};

use crate::{
    collateral::FixedIncomeDiscountPolicy,
    context::PricingContext,
    conv::{extract_market_index, extract_requests, qs_err},
    extension::Trade,
    loan::Loan,
    results::EvaluationResults,
};

/// Native loan valuation with optional refinancing and discount-policy overrides.
#[pyclass(module = "quantsupport", name = "LoanPricer")]
pub struct LoanPricer {
    refinancing_index: Option<MarketIndex>,
    discount_policy: Option<FixedIncomeDiscountPolicy>,
    recompute_schedule: bool,
}

#[pymethods]
impl LoanPricer {
    #[new]
    #[pyo3(signature = (*, refinancing_index = None, discount_policy = None, recompute_schedule = false))]
    fn new(
        refinancing_index: Option<&Bound<'_, PyAny>>,
        discount_policy: Option<FixedIncomeDiscountPolicy>,
        recompute_schedule: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            refinancing_index: refinancing_index.map(extract_market_index).transpose()?,
            discount_policy,
            recompute_schedule,
        })
    }

    /// Evaluates a loan position using an initialized native pricing context.
    #[pyo3(signature = (trade, context, requests = None))]
    fn evaluate<'py>(
        &self,
        py: Python<'py>,
        trade: PyRef<'py, Trade>,
        context: PyRef<'py, PricingContext>,
        requests: Option<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<Py<EvaluationResults>> {
        let native_context = context.native_context()?;
        Tape::<ADForward>::stop_recording_fwd();
        let instrument = trade.instrument_bound(py).extract::<Loan>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "LoanPricer requires Trade(Loan(...), trade_date, side)",
            )
        })?;
        let trade = instrument.build_trade(&trade)?;
        let requests = requests.as_ref().map_or_else(
            || Ok(vec![Request::Value]),
            |values| extract_requests(values),
        )?;
        let mut pricer = QsLoanPricer::new().with_recompute_schedule(self.recompute_schedule);
        if let Some(index) = &self.refinancing_index {
            pricer = pricer.with_refinancing_index(index.clone());
        }
        pricer.set_discount_policy(Box::new(self.discount_policy.as_ref().map_or_else(
            || context.loan_discount_policy(),
            FixedIncomeDiscountPolicy::build,
        )));
        let result = pricer
            .evaluate(&trade, &requests, native_context)
            .map_err(qs_err)?;
        Py::new(py, EvaluationResults::from_qs(&result))
    }
}
