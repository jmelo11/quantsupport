//! Positions refer to an existing native contract, as in Rust Trade<I>.
use crate::{
    functions::helpers::*,
    runtime::{
        products::PositionTerms,
        registry::{with_registry, with_registry_mut},
    },
};
use quantsupport::prelude::BondPricingAnchor;
use quantsupport_xll_macros::xll_bindgen;
use xll_rs::types::XllError;

#[xll_bindgen(
    name = "QS.TRADE.CREATE",
    category = "QuantSupport - Instruments",
    help = "Creates a native position on an existing instrument; contract terms and pricing policy remain separate"
)]
#[allow(clippy::too_many_arguments)]
pub fn qs_trade_create(
    name: &str,
    instrument: &str,
    trade_date: &str,
    side: &str,
    notional: Option<f64>,
    foreign_notional: Option<f64>,
    pricing_anchor: Option<String>,
    anchor_value: Option<f64>,
    settlement_date: Option<String>,
    spread_day_counter: Option<String>,
) -> Result<String, XllError> {
    let key = input_key(&(
        instrument,
        trade_date,
        side,
        notional,
        foreign_notional,
        pricing_anchor.as_deref(),
        anchor_value,
        settlement_date.as_deref(),
        spread_day_counter.as_deref(),
    ))?;
    let kind = with_registry(|registry| {
        registry.instrument(instrument).map(|value| {
            if value.kind() == "CreditDefaultSwap" {
                "CdsTrade".to_string()
            } else {
                format!("{}Trade", value.kind())
            }
        })
    })
    .map_err(value_error)?;
    if let Some(handle) = cached_handle(name, &kind, &key)? {
        return Ok(handle);
    }
    let anchor = match (
        pricing_anchor
            .as_deref()
            .filter(|value| !value.trim().is_empty()),
        anchor_value,
    ) {
        (None, None) => None,
        (Some("CleanPrice"), Some(value)) => Some(BondPricingAnchor::CleanPrice(value)),
        (Some("DirtyPrice"), Some(value)) => Some(BondPricingAnchor::DirtyPrice(value)),
        (Some("ZSpread"), Some(value)) => Some(BondPricingAnchor::ZSpread(value)),
        _ => return Err(value_error(
            "supply pricing_anchor and anchor_value together: CleanPrice, DirtyPrice, or ZSpread",
        )),
    };
    let terms = PositionTerms {
        notional,
        foreign_notional,
        pricing_anchor: anchor,
        settlement_date: settlement_date
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(parse_date)
            .transpose()?,
        spread_day_counter: spread_day_counter
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(parse_day_counter)
            .transpose()?,
    };
    let date = parse_date(trade_date)?;
    let side = parse_side(side)?;
    let trade = with_registry(|registry| {
        registry
            .instrument(instrument)
            .map_err(|error| error.to_string())?
            .trade(date, side, &terms)
    })
    .map_err(value_error)?;
    with_registry_mut(|registry| registry.upsert_if_changed(name, trade, key)).map_err(value_error)
}
