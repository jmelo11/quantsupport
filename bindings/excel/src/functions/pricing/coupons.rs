//! Contractual metadata for native and compressed coupons.
use crate::{functions::helpers::*, runtime::registry::with_registry};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

#[xll_bindgen(
    name = "QS.RESULT.COUPONS",
    category = "QuantSupport - Pricing",
    help = "Spills coupon type, fixing index, dates, signed notional, weighted rate/spread and original conventions"
)]
pub fn qs_result_coupons(
    result: &str,
    transpose: *const XLOPER12,
    headers: *const XLOPER12,
) -> Result<*mut XLOPER12, XllError> {
    let transpose = optional_bool(transpose, "transpose")?;
    let headers = optional_bool(headers, "headers")?;
    with_registry(|registry| {
        let results = registry.evaluation_results(result).map_err(value_error)?;
        let flows = results.cashflows().ok_or_else(|| {
            not_found("Request Cashflows in QS.EVALUATE or QS.EVALUATE.PORTFOLIO")
        })?;
        let columns = [
            "Payment date",
            "Type",
            "Currency",
            "Amount",
            "Accrual start",
            "Accrual end",
            "Fixing index",
            "Coupon index",
            "Signed notional",
            "Coupon rate",
            "Spread",
            "Fixing",
            "Accrual",
            "Day count",
            "Compounding",
            "Rate frequency",
            "Overnight conventions",
            "Payoff",
        ];
        let mut cells: Vec<_> = columns
            .iter()
            .map(|name| XLOPER12::from_str(name))
            .collect();
        for row in 0..flows.payment_dates().len() {
            let details = flows.coupon_details()[row].as_ref();
            cells.extend([
                XLOPER12::from_str(&flows.payment_dates()[row].to_string()),
                XLOPER12::from_str(&flows.cashflow_types()[row].to_string()),
                XLOPER12::from_str(&flows.currencies()[row].to_string()),
                XLOPER12::from_f64(flows.amounts()[row]),
                details.map_or_else(XLOPER12::nil, |terms| {
                    XLOPER12::from_str(&terms.accrual_start().to_string())
                }),
                details.map_or_else(XLOPER12::nil, |terms| {
                    XLOPER12::from_str(&terms.accrual_end().to_string())
                }),
                details
                    .and_then(|terms| terms.fixing_index())
                    .map_or_else(XLOPER12::nil, |index| {
                        XLOPER12::from_str(&index.to_string())
                    }),
                details
                    .and_then(|terms| terms.market_index())
                    .map_or_else(XLOPER12::nil, |index| {
                        XLOPER12::from_str(&index.to_string())
                    }),
                details.map_or_else(XLOPER12::nil, |terms| XLOPER12::from_f64(terms.notional())),
                details
                    .and_then(|terms| terms.rate())
                    .map_or_else(XLOPER12::nil, XLOPER12::from_f64),
                details
                    .and_then(|terms| terms.spread())
                    .map_or_else(XLOPER12::nil, XLOPER12::from_f64),
                flows.fixing()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64),
                XLOPER12::from_f64(flows.accrual_periods()[row]),
                details.map_or_else(XLOPER12::nil, |terms| {
                    XLOPER12::from_str(&format!("{:?}", terms.day_counter()))
                }),
                details
                    .and_then(|terms| terms.rate_definition())
                    .map_or_else(XLOPER12::nil, |definition| {
                        XLOPER12::from_str(&format!("{:?}", definition.compounding()))
                    }),
                details
                    .and_then(|terms| terms.rate_definition())
                    .map_or_else(XLOPER12::nil, |definition| {
                        XLOPER12::from_str(&format!("{:?}", definition.frequency()))
                    }),
                details
                    .and_then(|terms| terms.overnight_conventions())
                    .map_or_else(XLOPER12::nil, |conventions| {
                        XLOPER12::from_str(&format!("{conventions:?}"))
                    }),
                details
                    .and_then(|terms| terms.payoff())
                    .map_or_else(XLOPER12::nil, |payoff| {
                        XLOPER12::from_str(&format!("{payoff:?}"))
                    }),
            ]);
        }
        Ok(spill(
            cells,
            flows.payment_dates().len() + 1,
            columns.len(),
            transpose,
            headers,
        ))
    })
}
