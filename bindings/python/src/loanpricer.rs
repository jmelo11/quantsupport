//! Python loan-pricer settings and evaluation through initialized native contexts.

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
///
/// Prices signed expected receipts without changing contractual cashflows.
/// Supports Value, Cashflows, Sensitivities, and FairRate requests.
///
/// Creates a pricer with optional market-input and fair-rate settings.
///
/// ``refinancing_benchmark_index`` is required for refinancing-incentive prepayment;
/// its metadata determines the observation tenor and quote convention. This is
/// an index benchmark, excluding replacement-loan lending spreads and fees.
/// ``discount_policy`` overrides the context's fixed-income policy. Without
/// an override, uses the context policy, whose default maps the context base
/// currency to its base curve. The contract carries no discount-curve choice.
///
/// ``rebuild_amortization_for_fair_rate=False`` preserves principal during FairRate trials.
/// Setting it to True rebuilds amortization for each candidate using the
/// repayment structure and initial trial all-in rate. It does not change the
/// original loan or principal used for Value/Sensitivities calculations.
#[pyclass(module = "quantsupport", name = "LoanPricer")]
pub struct LoanPricer {
    refinancing_benchmark_index: Option<MarketIndex>,
    discount_policy: Option<FixedIncomeDiscountPolicy>,
    rebuild_amortization_for_fair_rate: bool,
}

#[pymethods]
impl LoanPricer {
    /// Creates a pricer with the documented refinancing and fair-rate settings.
    #[new]
    #[pyo3(signature = (*, refinancing_benchmark_index = None, discount_policy = None, rebuild_amortization_for_fair_rate = false))]
    fn new(
        refinancing_benchmark_index: Option<&Bound<'_, PyAny>>,
        discount_policy: Option<FixedIncomeDiscountPolicy>,
        rebuild_amortization_for_fair_rate: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            refinancing_benchmark_index: refinancing_benchmark_index
                .map(extract_market_index)
                .transpose()?,
            discount_policy,
            rebuild_amortization_for_fair_rate,
        })
    }

    /// Evaluates a loan position using an initialized native pricing context.
    ///
    /// ``trade`` must wrap a Loan; a position notional override rebuilds the
    /// contractual principal schedule at that size. ``requests`` accepts Request
    /// values or names and defaults to Value. Returns EvaluationResults.
    ///
    /// Value is signed receipt PV, excluding the initial advance. Cashflows are
    /// future expected receipts in legs 0/1/2 for scheduled payments, prepayment,
    /// and recovery. Sensitivities are derivatives per unit market pillar change.
    /// FairRate solves an annual fixed coupon or floating spread at/before
    /// origination; mixed, stepped, and bounded coupon schedules need an explicitly
    /// selected adjustable segment and are unsupported. Its search is [-5%, 50%].
    ///
    /// Raises TypeError for a non-loan trade. Raises QuantSupportError for an
    /// uninitialized context, unavailable observations, invalid projection terms,
    /// unsupported requests, or an unsuccessful fair-rate solve.
    #[pyo3(signature = (trade, context, requests = None))]
    fn evaluate<'py>(
        &self,
        py: Python<'py>,
        trade: PyRef<'py, Trade>,
        context: PyRef<'py, PricingContext>,
        requests: Option<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<Py<EvaluationResults>> {
        let native_context = context.market_view()?;
        Tape::<ADForward>::stop_recording_fwd();
        let instrument = trade.instrument_bound(py).extract::<Loan>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "LoanPricer requires Trade(Loan(...), trade_date, side)",
            )
        })?;
        let trade = instrument.build_native_loan_trade(&trade)?;
        let requests = requests.as_ref().map_or_else(
            || Ok(vec![Request::Value]),
            |values| extract_requests(values),
        )?;
        let mut pricer = QsLoanPricer::new()
            .with_fair_rate_amortization_rebuild(self.rebuild_amortization_for_fair_rate);
        if let Some(index) = &self.refinancing_benchmark_index {
            pricer = pricer.with_refinancing_benchmark_index(index.clone());
        }
        pricer.set_discount_policy(Box::new(self.discount_policy.as_ref().map_or_else(
            || context.build_loan_discount_policy(),
            FixedIncomeDiscountPolicy::build,
        )));
        let result = pricer
            .evaluate(&trade, &requests, &native_context)
            .map_err(qs_err)?;
        Py::new(py, EvaluationResults::from_qs(&result))
    }
}
