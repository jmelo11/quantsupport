use std::{fmt::Display, str::FromStr};

use quantsupport::prelude::{
    CapFloorType, CapletFloorletType, Compounding, Currency, Date, DayCounter, EuroOptionType,
    Frequency, FxPair, MarketIndex, Side,
};
use xll_rs::types::{XLLError, XllError};
use xll_rs::{convert::build_multi, types::XLOPER12};

pub fn value_error(error: impl Display) -> XllError {
    XllError::msg(XLLError::VALUE, error.to_string())
}

pub fn not_found(error: impl Display) -> XllError {
    XllError::msg(XLLError::NA, error.to_string())
}

pub fn parse_date(value: &str) -> Result<Date, XllError> {
    Date::from_str(value.trim(), "%Y-%m-%d").map_err(value_error)
}

pub fn parse_currency(value: &str) -> Result<Currency, XllError> {
    Currency::from_str(value.trim().to_ascii_uppercase().as_str()).map_err(value_error)
}

pub fn parse_index(value: &str) -> Result<MarketIndex, XllError> {
    let value = value.trim();
    if let Some(name) = value.strip_prefix("Equity:") {
        return Ok(MarketIndex::Equity(name.to_string()));
    }
    if let Some(name) = value.strip_prefix("Credit:") {
        return Ok(MarketIndex::Credit(name.to_string()));
    }
    if let Some(pair) = value.strip_prefix("FX:") {
        let (base, quote) = pair
            .split_once('/')
            .ok_or_else(|| value_error(format!("invalid FX market index '{value}'")))?;
        return FxPair::new(parse_currency(base)?, parse_currency(quote)?)
            .map(MarketIndex::FxPair)
            .map_err(value_error);
    }
    if let Some(pair) = value.strip_prefix("Collateral:") {
        let (currency, collateral) = pair
            .split_once('/')
            .ok_or_else(|| value_error(format!("invalid collateral market index '{value}'")))?;
        return Ok(MarketIndex::Collateral(
            parse_currency(currency)?,
            parse_currency(collateral)?,
        ));
    }
    Ok(MarketIndex::from_str(value).unwrap_or_else(|never| match never {}))
}

pub fn parse_day_counter(value: &str) -> Result<DayCounter, XllError> {
    DayCounter::try_from(value.trim().to_string()).map_err(value_error)
}

pub fn parse_compounding(value: &str) -> Result<Compounding, XllError> {
    Compounding::try_from(value.trim().to_string()).map_err(value_error)
}

pub fn parse_frequency(value: &str) -> Result<Frequency, XllError> {
    Frequency::from_str(value.trim()).map_err(value_error)
}

pub fn parse_time_unit(value: &str) -> Result<quantsupport::prelude::TimeUnit, XllError> {
    quantsupport::prelude::TimeUnit::try_from(value.trim().to_string()).map_err(value_error)
}

pub fn parse_side(value: &str) -> Result<Side, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "longreceive" | "receive" | "long" | "buy" => Ok(Side::LongReceive),
        "payshort" | "pay" | "short" | "sell" => Ok(Side::PayShort),
        _ => Err(value_error(format!(
            "invalid side '{value}'; use LongReceive/Receive/Buy or PayShort/Pay/Sell"
        ))),
    }
}

pub fn parse_option_type(value: &str) -> Result<EuroOptionType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "call" => Ok(EuroOptionType::Call),
        "put" => Ok(EuroOptionType::Put),
        _ => Err(value_error(format!(
            "invalid option type '{value}'; use Call or Put"
        ))),
    }
}

pub fn parse_cap_floor_type(value: &str) -> Result<CapFloorType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cap" => Ok(CapFloorType::Cap),
        "floor" => Ok(CapFloorType::Floor),
        _ => Err(value_error(format!(
            "invalid cap/floor type '{value}'; use Cap or Floor"
        ))),
    }
}

pub fn parse_caplet_floorlet_type(value: &str) -> Result<CapletFloorletType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "caplet" | "cap" => Ok(CapletFloorletType::Caplet),
        "floorlet" | "floor" => Ok(CapletFloorletType::Floorlet),
        _ => Err(value_error(format!(
            "invalid caplet/floorlet type '{value}'; use Caplet or Floorlet"
        ))),
    }
}

pub fn string_column(values: Vec<String>) -> *mut XLOPER12 {
    let rows = values.len();
    let cells = values
        .iter()
        .map(|value| XLOPER12::from_str(value))
        .collect();
    build_multi(cells, rows, 1)
}
