use std::{fmt::Display, str::FromStr};

use quantsupport::prelude::{
    CapFloorType, CapletFloorletType, Compounding, Currency, Date, DayCounter, EuroOptionType,
    Frequency, FxPair, Interpolator, MarketIndex, Side,
};
use serde::Serialize;
use xll_rs::convert::build_multi;
use xll_rs::types::{
    XLLError, XllError, XLOPER12, XLTYPE_BOOL, XLTYPE_INT, XLTYPE_MISSING, XLTYPE_MULTI,
    XLTYPE_NIL, XLTYPE_NUM, XLTYPE_STR,
};

pub(crate) fn value_error(error: impl Display) -> XllError {
    XllError::msg(XLLError::VALUE, error.to_string())
}

pub(crate) fn not_found(error: impl Display) -> XllError {
    XllError::msg(XLLError::NA, error.to_string())
}

pub(crate) fn parse_date(value: &str) -> Result<Date, XllError> {
    Date::from_str(value.trim(), "%Y-%m-%d").map_err(value_error)
}

pub(crate) fn parse_currency(value: &str) -> Result<Currency, XllError> {
    Currency::from_str(value.trim().to_ascii_uppercase().as_str()).map_err(value_error)
}

pub(crate) fn parse_index(value: &str) -> Result<MarketIndex, XllError> {
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

pub(crate) fn parse_day_counter(value: &str) -> Result<DayCounter, XllError> {
    DayCounter::try_from(value.trim().to_string()).map_err(value_error)
}

pub(crate) fn parse_compounding(value: &str) -> Result<Compounding, XllError> {
    Compounding::try_from(value.trim().to_string()).map_err(value_error)
}

pub(crate) fn parse_frequency(value: &str) -> Result<Frequency, XllError> {
    Frequency::from_str(value.trim()).map_err(value_error)
}

pub(crate) fn input_key<T: Serialize>(inputs: &T) -> Result<String, XllError> {
    serde_json::to_string(inputs)
        .map_err(|error| value_error(format!("could not fingerprint worksheet inputs: {error}")))
}

pub(crate) fn parse_interpolator(value: &str) -> Result<Interpolator, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "linear" => Ok(Interpolator::Linear),
        "loglinear" | "log-linear" | "log linear" => Ok(Interpolator::LogLinear),
        "cubicspline" | "cubic-spline" | "cubic spline" => Ok(Interpolator::CubicSpline),
        _ => Err(value_error(format!(
            "invalid interpolator '{value}'; use Linear, LogLinear, or CubicSpline"
        ))),
    }
}

pub(crate) fn parse_time_unit(value: &str) -> Result<quantsupport::prelude::TimeUnit, XllError> {
    quantsupport::prelude::TimeUnit::try_from(value.trim().to_string()).map_err(value_error)
}

pub(crate) fn parse_side(value: &str) -> Result<Side, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "longreceive" | "receive" | "long" | "buy" => Ok(Side::LongReceive),
        "payshort" | "pay" | "short" | "sell" => Ok(Side::PayShort),
        _ => Err(value_error(format!(
            "invalid side '{value}'; use LongReceive/Receive/Buy or PayShort/Pay/Sell"
        ))),
    }
}

pub(crate) fn parse_option_type(value: &str) -> Result<EuroOptionType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "call" => Ok(EuroOptionType::Call),
        "put" => Ok(EuroOptionType::Put),
        _ => Err(value_error(format!(
            "invalid option type '{value}'; use Call or Put"
        ))),
    }
}

pub(crate) fn parse_cap_floor_type(value: &str) -> Result<CapFloorType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cap" => Ok(CapFloorType::Cap),
        "floor" => Ok(CapFloorType::Floor),
        _ => Err(value_error(format!(
            "invalid cap/floor type '{value}'; use Cap or Floor"
        ))),
    }
}

pub(crate) fn parse_caplet_floorlet_type(value: &str) -> Result<CapletFloorletType, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "caplet" | "cap" => Ok(CapletFloorletType::Caplet),
        "floorlet" | "floor" => Ok(CapletFloorletType::Floorlet),
        _ => Err(value_error(format!(
            "invalid caplet/floorlet type '{value}'; use Caplet or Floorlet"
        ))),
    }
}

pub(crate) fn string_column(values: Vec<String>) -> *mut XLOPER12 {
    let rows = values.len();
    let cells = values
        .iter()
        .map(|value| XLOPER12::from_str(value))
        .collect();
    build_multi(cells, rows, 1)
}

pub(crate) fn string_values(value: *const XLOPER12, argument_name: &str) -> Result<Vec<String>, XllError> {
    if value.is_null() {
        return Err(value_error(format!("{argument_name} cannot be null")));
    }
    let value = unsafe { &*value };
    match value.base_type() {
        XLTYPE_STR => value
            .as_string()
            .filter(|text| !text.trim().is_empty())
            .map(|text| vec![text])
            .ok_or_else(|| value_error(format!("{argument_name} cannot be blank"))),
        XLTYPE_MULTI => {
            let array = unsafe { &*std::ptr::addr_of!(value.val.array) };
            if array.rows <= 0 || array.columns <= 0 || array.lparray.is_null() {
                return Err(value_error(format!(
                    "{argument_name} must contain at least one cell"
                )));
            }
            let total = (array.rows as usize)
                .checked_mul(array.columns as usize)
                .ok_or_else(|| value_error(format!("{argument_name} range is too large")))?;
            let mut values = Vec::with_capacity(total);
            for index in 0..total {
                let cell = unsafe { &*array.lparray.add(index) };
                match cell.base_type() {
                    XLTYPE_STR => {
                        let text = cell.as_string().ok_or_else(|| {
                            value_error(format!("{argument_name} contains invalid text"))
                        })?;
                        if !text.trim().is_empty() {
                            values.push(text);
                        }
                    }
                    XLTYPE_NIL | XLTYPE_MISSING => {}
                    _ => {
                        return Err(value_error(format!(
                            "{argument_name} must be a range containing quote identifier text"
                        )));
                    }
                }
            }
            if values.is_empty() {
                return Err(value_error(format!("{argument_name} cannot be blank")));
            }
            Ok(values)
        }
        _ => Err(value_error(format!(
            "{argument_name} must be a quote identifier or an Excel range of identifiers"
        ))),
    }
}

pub(crate) fn bool_value(value: *const XLOPER12, argument_name: &str) -> Result<bool, XllError> {
    if value.is_null() {
        return Err(value_error(format!("{argument_name} cannot be null")));
    }
    let value = unsafe { &*value };
    match value.base_type() {
        XLTYPE_BOOL => Ok(unsafe { value.val.xbool } != 0),
        XLTYPE_INT => Ok(unsafe { value.val.w } != 0),
        XLTYPE_NUM => Ok(unsafe { value.val.num } != 0.0),
        _ => Err(value_error(format!(
            "{argument_name} must be TRUE or FALSE"
        ))),
    }
}

pub(crate) fn string_matrix(values: Vec<Vec<String>>) -> *mut XLOPER12 {
    let rows = values.len();
    let columns = values.iter().map(Vec::len).max().unwrap_or(0);
    let cells = values
        .into_iter()
        .flat_map(|mut row| {
            row.resize(columns, String::new());
            row
        })
        .map(|value| XLOPER12::from_str(&value))
        .collect();
    build_multi(cells, rows, columns)
}
