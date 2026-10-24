//! Portfolio constructors, streaming valuation, and compact result tables.

use crate::{
    functions::helpers::*,
    runtime::{
        portfolios, pricing,
        registry::{with_registry, with_registry_mut, QsObject},
    },
};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

#[xll_bindgen(
    name = "QS.PORTFOLIO.CREATE",
    category = "QuantSupport - Pricing",
    help = "Creates a portfolio from a range of live trade handles; repeated handles count as repeated positions"
)]
pub fn qs_portfolio_create(name: &str, trades: *const XLOPER12) -> Result<String, XllError> {
    let trades = string_values(trades, "trades")?;
    let key = input_key(&trades)?;
    let portfolio =
        with_registry(|registry| portfolios::create(registry, trades)).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::Portfolio(Box::new(portfolio)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.EVALUATE.PORTFOLIO",
    category = "QuantSupport - Pricing",
    help = "Returns standard EvaluationResults; pricers accepts one handle or one per trade, and value_currency identifies the common monetary PV currency"
)]
pub fn qs_portfolio_evaluate(
    context: &str,
    portfolio: &str,
    pricers: *const XLOPER12,
    requests: *const XLOPER12,
    value_currency: Option<String>,
) -> Result<String, XllError> {
    let name = with_registry(|r| r.cell_name("portfolio.evaluation")).map_err(value_error)?;
    let pricers = string_values(pricers, "pricers")?;
    let names = string_values(requests, "requests")?;
    let requests = pricing::parse_requests(&names).map_err(value_error)?;
    let market_revision = with_registry(|r| crate::runtime::context::revision_key(r, context))
        .map_err(value_error)?;
    let key = input_key(&(
        portfolio,
        &pricers,
        context,
        &names,
        &value_currency,
        market_revision,
    ))?;
    let currency = value_currency
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_currency)
        .transpose()?;
    with_registry(|registry| {
        let portfolio = registry.portfolio(portfolio)?;
        for trade in portfolio.trades() {
            registry.object(trade)?;
        }
        registry.pricing_context(context)?;
        for pricer in &pricers {
            registry.pricer(pricer)?;
        }
        Ok::<(), crate::runtime::registry::RegistryError>(())
    })
    .map_err(value_error)?;
    if let Some(handle) =
        with_registry(|registry| registry.handle_if_unchanged(&name, "EvaluationResults", &key))
            .map_err(value_error)?
    {
        return Ok(handle);
    }
    let result = with_registry(|registry| {
        portfolios::evaluate(registry, portfolio, &pricers, context, &requests, currency)
    })
    .map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            &name,
            QsObject::EvaluationResults(Box::new(result.with_identifier(name.clone()))),
            key,
        )
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.RESULT.CASHFLOW.SUMMARY",
    category = "QuantSupport - Pricing",
    help = "Spills date and currency with separate signed Interest, Capital, Other and Total amounts; no accrual allocation or FX conversion"
)]
pub fn qs_portfolio_result_cashflows(
    result: &str,
    transpose: *const XLOPER12,
    headers: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = optional_bool(transpose, "transpose")?;
    let headers = optional_bool(headers, "headers")?;
    with_registry(|registry| {
        let result = registry.evaluation_results(result).map_err(value_error)?;
        let flows = result.cashflows().ok_or_else(|| {
            not_found("Request Cashflows in QS.EVALUATE or QS.EVALUATE.PORTFOLIO")
        })?;
        let rows = quantsupport::prelude::summarize_cashflows(flows).map_err(value_error)?;
        let mut cells = [
            "Payment date",
            "Currency",
            "Interest",
            "Capital",
            "Other",
            "Total",
        ]
        .into_iter()
        .map(XLOPER12::from_str)
        .collect::<Vec<_>>();
        for row in &rows {
            cells.extend([
                XLOPER12::from_str(&row.payment_date().to_string()),
                XLOPER12::from_str(&row.currency().to_string()),
                XLOPER12::from_f64(row.interest()),
                XLOPER12::from_f64(row.capital()),
                XLOPER12::from_f64(row.other()),
                XLOPER12::from_f64(row.total()),
            ]);
        }
        Ok(spill(cells, rows.len() + 1, 6, transpose, headers))
    })
}
