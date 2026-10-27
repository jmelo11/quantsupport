use crate::runtime::{context, registry::with_registry};
use quantsupport::{
    prelude::{
        BusinessDayConvention, Calendar, CapFloorType, CapletFloorletType, Compounding, Currency,
        Date, DayCounter, EuroOptionType, Frequency, Interpolator, MarketIndex, Period,
        PricingContext, Side,
    },
    time::calendars::traits::IsCalendar,
};
use serde::Serialize;
use std::{fmt::Display, rc::Rc, str::FromStr};
use xll_rs::{
    convert::build_multi,
    types::{
        XLLError, XllError, XLOPER12, XLTYPE_BOOL, XLTYPE_INT, XLTYPE_MISSING, XLTYPE_MULTI,
        XLTYPE_NIL, XLTYPE_NUM, XLTYPE_STR,
    },
};

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
    crate::runtime::inputs::market_index(value).map_err(value_error)
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

pub fn input_key<T: Serialize>(inputs: &T) -> Result<String, XllError> {
    serde_json::to_string(inputs)
        .map_err(|error| value_error(format!("could not fingerprint worksheet inputs: {error}")))
}

pub fn parse_interpolator(value: &str) -> Result<Interpolator, XllError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "linear" => Ok(Interpolator::Linear),
        "loglinear" | "log-linear" | "log linear" => Ok(Interpolator::LogLinear),
        "cubicspline" | "cubic-spline" | "cubic spline" => Ok(Interpolator::CubicSpline),
        _ => Err(value_error(format!(
            "invalid interpolator '{value}'; use Linear, LogLinear, or CubicSpline"
        ))),
    }
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

pub fn string_column(values: Vec<String>, transpose: Option<bool>) -> *mut XLOPER12 {
    let rows = values.len();
    let cells = values
        .iter()
        .map(|value| XLOPER12::from_str(value))
        .collect();
    spill(cells, rows, 1, transpose, None)
}

pub fn string_values(value: *const XLOPER12, argument_name: &str) -> Result<Vec<String>, XllError> {
    read_string_values(value, argument_name, false)
}

pub fn optional_string_values(
    value: *const XLOPER12,
    argument_name: &str,
) -> Result<Vec<String>, XllError> {
    read_string_values(value, argument_name, true)
}

fn read_string_values(
    value: *const XLOPER12,
    argument_name: &str,
    allow_empty: bool,
) -> Result<Vec<String>, XllError> {
    if value.is_null() {
        return Err(value_error(format!("{argument_name} cannot be null")));
    }
    let value = unsafe { &*value };
    match value.base_type() {
        XLTYPE_NIL | XLTYPE_MISSING if allow_empty => Ok(Vec::new()),
        XLTYPE_STR => value
            .as_string()
            .filter(|text| allow_empty || !text.trim().is_empty())
            .map(|text| {
                if text.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![text]
                }
            })
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
                            "{argument_name} must be a range containing text"
                        )));
                    }
                }
            }
            if values.is_empty() && !allow_empty {
                return Err(value_error(format!("{argument_name} cannot be blank")));
            }
            Ok(values)
        }
        _ => Err(value_error(format!(
            "{argument_name} must be text or an Excel range of text"
        ))),
    }
}

/// Read every numeric cell without dropping blanks and misaligning curve pillars.
pub fn numeric_values(value: *const XLOPER12, argument_name: &str) -> Result<Vec<f64>, XllError> {
    if value.is_null() {
        return Err(value_error(format!("{argument_name} cannot be null")));
    }
    let value = unsafe { &*value };
    let number = |cell: &XLOPER12| {
        let number = match cell.base_type() {
            XLTYPE_NUM => unsafe { cell.val.num },
            XLTYPE_INT => f64::from(unsafe { cell.val.w }),
            _ => {
                return Err(value_error(format!(
                    "{argument_name} must contain only finite numbers; blanks are not allowed"
                )))
            }
        };
        if !number.is_finite() {
            return Err(value_error(format!(
                "{argument_name} must contain only finite numbers"
            )));
        }
        Ok(number)
    };
    if value.base_type() != XLTYPE_MULTI {
        return number(value).map(|number| vec![number]);
    }
    let array = unsafe { &*std::ptr::addr_of!(value.val.array) };
    if array.rows <= 0 || array.columns <= 0 || array.lparray.is_null() {
        return Err(value_error(format!(
            "{argument_name} must contain at least one cell"
        )));
    }
    let total = (array.rows as usize)
        .checked_mul(array.columns as usize)
        .ok_or_else(|| value_error("Range is too large"))?;
    (0..total)
        .map(|index| number(unsafe { &*array.lparray.add(index) }))
        .collect()
}

/// Reads one scalar, row or column array, retaining interior blanks and types.
pub fn worksheet_array(
    value: *const XLOPER12,
    name: &str,
) -> Result<Vec<crate::runtime::inputs::Cell>, XllError> {
    use crate::runtime::inputs;
    if value.is_null() {
        return Ok(Vec::new());
    }
    let array = worksheet_table(value, name)?;
    let values = inputs::vector(&array, name).map_err(value_error)?;
    if matches!(values.as_slice(), [cell] if cell.is_blank()) {
        return Ok(Vec::new());
    }
    Ok(values)
}

/// Aligns independent worksheet arrays; no rectangular input layout is required.
pub fn worksheet_columns(
    fields: &[(&str, *const XLOPER12)],
) -> Result<crate::runtime::inputs::Table, XllError> {
    let arrays = fields
        .iter()
        .map(|(name, value)| worksheet_array(*value, name))
        .collect::<Result<Vec<_>, _>>()?;
    let columns: Vec<_> = fields
        .iter()
        .zip(&arrays)
        .map(|((name, _), values)| (*name, values.as_slice()))
        .collect();
    crate::runtime::inputs::aligned(&columns).map_err(value_error)
}

/// Reads quote identifiers and sides as independent arrays. Identifier-only
/// inputs select existing context quotes when used in a curve definition.
pub fn worksheet_quotes(
    identifiers: *const XLOPER12,
    mid: *const XLOPER12,
    bid: *const XLOPER12,
    ask: *const XLOPER12,
) -> Result<crate::runtime::inputs::Table, XllError> {
    let arrays = [
        worksheet_array(identifiers, "identifiers")?,
        worksheet_array(mid, "mid")?,
        worksheet_array(bid, "bid")?,
        worksheet_array(ask, "ask")?,
    ];
    if arrays[1..].iter().all(Vec::is_empty) {
        return Ok(arrays[0].iter().cloned().map(|cell| vec![cell]).collect());
    }
    crate::runtime::market_inputs::quote_rows(&arrays[0], &arrays[1], &arrays[2], &arrays[3])
        .map_err(value_error)
}

/// Internal reader for typed array values, preserving blanks and dimensions.
pub fn worksheet_table(
    value: *const XLOPER12,
    name: &str,
) -> Result<crate::runtime::inputs::Table, XllError> {
    use crate::runtime::inputs::Cell;
    if value.is_null() {
        return Err(value_error(format!("{name} cannot be null")));
    }
    let cell = |value: &XLOPER12| -> Result<Cell, XllError> {
        match value.base_type() {
            XLTYPE_NIL | XLTYPE_MISSING => Ok(Cell::Blank),
            XLTYPE_STR => value
                .as_string()
                .map(Cell::Text)
                .ok_or_else(|| value_error(format!("{name} contains invalid text"))),
            XLTYPE_BOOL => Ok(Cell::Boolean(unsafe { value.val.xbool } != 0)),
            XLTYPE_INT => Ok(Cell::Number(f64::from(unsafe { value.val.w }))),
            XLTYPE_NUM if unsafe { value.val.num }.is_finite() => {
                Ok(Cell::Number(unsafe { value.val.num }))
            }
            _ => Err(value_error(format!(
                "{name} contains an error or unsupported cell value"
            ))),
        }
    };
    let value = unsafe { &*value };
    if value.base_type() != XLTYPE_MULTI {
        return Ok(vec![vec![cell(value)?]]);
    }
    let array = unsafe { &*std::ptr::addr_of!(value.val.array) };
    if array.rows <= 0 || array.columns <= 0 || array.lparray.is_null() {
        return Err(value_error(format!("{name} has invalid range dimensions")));
    }
    let columns = array.columns as usize;
    let total = (array.rows as usize)
        .checked_mul(columns)
        .ok_or_else(|| value_error(format!("{name} is too large")))?;
    (0..total)
        .step_by(columns)
        .map(|start| {
            (start..start + columns)
                .map(|index| cell(unsafe { &*array.lparray.add(index) }))
                .collect()
        })
        .collect()
}

pub fn bool_value(value: *const XLOPER12, argument_name: &str) -> Result<bool, XllError> {
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

pub fn string_matrix(
    values: Vec<Vec<String>>,
    transpose: Option<bool>,
    headers: Option<bool>,
) -> *mut XLOPER12 {
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
    spill(cells, rows, columns, transpose, headers)
}

pub fn cached_handle(name: &str, kind: &str, key: &str) -> Result<Option<String>, XllError> {
    with_registry(|registry| registry.handle_if_unchanged(name, kind, key)).map_err(value_error)
}

/// Builds a typed spill, optionally transposing rows and columns together.
pub fn spill(
    cells: Vec<XLOPER12>,
    rows: usize,
    columns: usize,
    transpose: Option<bool>,
    headers: Option<bool>,
) -> *mut XLOPER12 {
    // XLOPER12 has no Drop implementation; excluded header strings need explicit cleanup.
    let (cells, rows, columns) = crate::runtime::output::prepare(
        cells,
        rows,
        columns,
        transpose.unwrap_or(false),
        headers.unwrap_or(true),
        |cell| xll_rs::memory::xlAutoFree12(Box::into_raw(Box::new(cell))),
    );
    if cells.is_empty() {
        return xll_rs::returning::XlReturn::nil().into_raw();
    }
    // xll-rs reconstructs the array with capacity equal to its length when freeing it.
    let cells = cells.into_boxed_slice().into_vec();
    build_multi(cells, rows, columns)
}

pub fn optional_bool(value: *const XLOPER12, name: &str) -> Result<Option<bool>, XllError> {
    if value.is_null() {
        return Err(value_error(format!("{name} cannot be null")));
    }
    if matches!(unsafe { &*value }.base_type(), XLTYPE_NIL | XLTYPE_MISSING) {
        return Ok(None);
    }
    bool_value(value, name).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_flags_preserve_types_and_remove_headers_before_transposing() {
        for (transpose, headers, expected_rows, expected_columns) in [
            (false, true, 3, 2),
            (true, true, 2, 3),
            (false, false, 2, 2),
            (true, false, 2, 2),
        ] {
            let cells = vec![
                XLOPER12::from_str("Date"),
                XLOPER12::from_str("Amount"),
                XLOPER12::from_str("2026-01-01"),
                XLOPER12::from_f64(12.0),
                XLOPER12::from_str("2026-02-01"),
                XLOPER12::nil(),
            ];
            let result = spill(cells, 3, 2, Some(transpose), Some(headers));
            let array = unsafe { &*std::ptr::addr_of!((*result).val.array) };
            assert_eq!(
                (array.rows, array.columns),
                (expected_rows, expected_columns)
            );
            let first = unsafe { &*array.lparray };
            assert_eq!(
                first.as_string().as_deref(),
                Some(if headers { "Date" } else { "2026-01-01" })
            );
            let cells = unsafe {
                std::slice::from_raw_parts(array.lparray, (array.rows * array.columns) as usize)
            };
            assert_eq!(
                cells
                    .iter()
                    .filter(|cell| cell.base_type() == XLTYPE_NUM)
                    .count(),
                1
            );
            assert_eq!(
                cells
                    .iter()
                    .filter(|cell| cell.base_type() == XLTYPE_NIL)
                    .count(),
                1
            );
            xll_rs::memory::xlAutoFree12(result);
        }
        let result = spill(vec![XLOPER12::from_str("Header")], 1, 1, None, Some(false));
        assert_eq!(unsafe { &*result }.base_type(), XLTYPE_NIL);
        xll_rs::memory::xlAutoFree12(result);
        assert_eq!(
            optional_bool(&XLOPER12::missing(), "transpose").unwrap(),
            None
        );
        assert_eq!(
            optional_bool(&XLOPER12::from_bool(true), "transpose").unwrap(),
            Some(true)
        );
        let invalid = XLOPER12::from_str("sideways");
        assert!(optional_bool(&invalid, "transpose").is_err());
        xll_rs::memory::xlAutoFree12(Box::into_raw(Box::new(invalid)));
    }
}

/// Captures the complete construction market and its input revision.
pub fn construction_market(context: &str) -> Result<(Rc<PricingContext>, String), XllError> {
    with_registry(|registry| -> Result<_, String> {
        Ok((
            context::resolve(registry, context)?,
            context::revision_key(registry, context)?,
        ))
    })
    .map_err(value_error)
}

pub fn construction_date(
    date: Option<Date>,
    tenor: Option<Period>,
    anchor: Date,
    label: &str,
    calendar: Option<&Calendar>,
    convention: BusinessDayConvention,
) -> Result<Date, String> {
    match (date, tenor) {
        (Some(date), None) => Ok(date),
        (None, Some(tenor)) => match calendar {
            Some(calendar) => calendar.advance(anchor, tenor, Some(convention), false),
            None => anchor.advance(tenor.length(), tenor.units()),
        }
        .map_err(|error| error.to_string()),
        (Some(_), Some(_)) => Err(format!("supply {label} or its tenor, not both")),
        (None, None) => Err(format!("{label} or its tenor is required")),
    }
}
