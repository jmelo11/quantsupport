use quantsupport::prelude::{DayCounter, Frequency};
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::{XllError, XLOPER12};

use crate::{
    functions::helpers::{
        cached_handle, input_key, parse_currency, parse_date, parse_day_counter, parse_frequency,
        parse_index, parse_side, value_error, worksheet_table,
    },
    runtime::{
        loan_inputs,
        loans::LoanTerms,
        products::{InstrumentObject, NativeInstrument},
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
    settings: *const XLOPER12,
    coupons: *const XLOPER12,
    prepayments: *const XLOPER12,
) -> Result<String, XllError> {
    let terms = LoanTerms {
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
        options: loan_inputs::options(
            &worksheet_table(settings, "settings")?,
            &worksheet_table(coupons, "coupons")?,
            &worksheet_table(prepayments, "prepayments")?,
        )
        .map_err(value_error)?,
    };
    let key = input_key(&(&terms, trade_date, side.as_deref()))?;
    if let Some(handle) = cached_handle(name, "LoanTrade", &key)? {
        return Ok(handle);
    }
    let loan = terms.build_instrument(name).map_err(value_error)?;
    let trade = quantsupport::prelude::LoanTrade::new(
        loan,
        parse_date(trade_date)?,
        parse_side(optional_text(side).as_deref().unwrap_or("Receive"))?,
    );
    with_registry_mut(|registry| {
        registry.upsert_if_changed(name, QsObject::LoanTrade(Box::new(trade)), key)
    })
    .map_err(value_error)
}

#[xll_bindgen(
    name = "QS.INSTRUMENT.LOAN",
    category = "QuantSupport - Instruments",
    help = "Creates a native loan contract from worksheet terms and ranges; positions, pricers and discount policies are separate"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_instrument_loan(
    name: &str,
    start_date: &str,
    maturity_date: &str,
    currency: &str,
    notional: f64,
    rate: f64,
    market_index: Option<String>,
    repayment: Option<String>,
    payment_frequency: Option<String>,
    day_counter: Option<String>,
    settings: *const XLOPER12,
    coupons: *const XLOPER12,
    prepayments: *const XLOPER12,
) -> Result<String, XllError> {
    let terms = LoanTerms {
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
        options: loan_inputs::options(
            &worksheet_table(settings, "settings")?,
            &worksheet_table(coupons, "coupons")?,
            &worksheet_table(prepayments, "prepayments")?,
        )
        .map_err(value_error)?,
    };
    let key = input_key(&terms)?;
    if let Some(handle) = cached_handle(name, "Loan", &key)? {
        return Ok(handle);
    }
    let notional = terms.notional;
    let loan = terms.build_instrument(name).map_err(value_error)?;
    with_registry_mut(|registry| {
        registry.upsert_if_changed(
            name,
            QsObject::Instrument(Box::new(InstrumentObject {
                native: NativeInstrument::Loan(loan),
                notional,
                foreign_notional: None,
            })),
            key,
        )
    })
    .map_err(value_error)
}
