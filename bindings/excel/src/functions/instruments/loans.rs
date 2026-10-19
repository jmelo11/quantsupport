use quantsupport::prelude::{DayCounter, Frequency};
use xll_rs::types::XllError;
use xllgen::xll_bindgen;

use crate::{
    functions::helpers::{
        cached_handle, input_key, parse_currency, parse_date, parse_day_counter, parse_frequency,
        parse_index, parse_json, parse_side, value_error,
    },
    runtime::{
        loans::{LoanOptions, LoanTerms},
        registry::{with_registry_mut, QsObject},
    },
};

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[xll_bindgen(
    name = "QS.TRADE.LOAN",
    volatile,
    category = "QuantSupport - Instruments",
    help = "Creates a fixed or floating loan with French, bullet, or balloon principal and optional prepayment/credit terms"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_loan(
    name: &str,
    trade_date: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    notional: f64,
    rate: f64,
    market_index: Option<String>,
    repayment: Option<String>,
    payment_frequency: Option<String>,
    day_counter: Option<String>,
    side: Option<String>,
    options: Option<String>,
) -> Result<String, XllError> {
    let terms = LoanTerms {
        trade_date: parse_date(trade_date)?,
        start_date: parse_date(start_date)?,
        maturity_date: parse_date(maturity_date)?,
        currency: parse_currency(currency)?,
        notional,
        rate,
        market_index: optional_text(market_index)
            .as_deref()
            .map(parse_index)
            .transpose()?,
        repayment: optional_text(repayment).unwrap_or_else(|| "French".into()),
        payment_frequency: optional_text(payment_frequency)
            .as_deref()
            .map(parse_frequency)
            .transpose()?
            .unwrap_or(Frequency::Monthly),
        day_counter: optional_text(day_counter)
            .as_deref()
            .map(parse_day_counter)
            .transpose()?
            .unwrap_or(DayCounter::Actual365),
        side: parse_side(optional_text(side).as_deref().unwrap_or("Receive"))?,
        options: optional_text(options)
            .as_deref()
            .map(|json| parse_json::<LoanOptions>(json, "loan options"))
            .transpose()?
            .unwrap_or_default(),
    };
    let key = input_key(&terms)?;
    if let Some(handle) = cached_handle(name, "LoanTrade", &key)? {
        return Ok(handle);
    }
    let loan = terms.build(name).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::LoanTrade(Box::new(loan)), key)
    })
    .map_err(value_error)
}
