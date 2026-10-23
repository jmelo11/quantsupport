//! Shared worksheet cashflow rendering. Evaluation lives in runtime::context.
use xll_rs::types::{XllError, XLOPER12};

pub(super) fn cashflows_output(
    table: &quantsupport::prelude::CashflowsTable,
    transpose: Option<bool>,
    headers: Option<bool>,
) -> Result<*mut XLOPER12, XllError> {
    const COLUMNS: usize = 9;
    let rows = table.payment_dates().len() + 1;
    let mut cells = Vec::with_capacity(rows * COLUMNS);
    for header in [
        "Payment date",
        "Type",
        "Amount",
        "Fixing",
        "Accrual",
        "Currency",
        "Floor",
        "Cap",
        "Leg",
    ] {
        cells.push(XLOPER12::from_str(header));
    }
    for row in 0..table.payment_dates().len() {
        cells.push(XLOPER12::from_str(&table.payment_dates()[row].to_string()));
        cells.push(XLOPER12::from_str(&table.cashflow_types()[row].to_string()));
        cells.push(XLOPER12::from_f64(table.amounts()[row]));
        cells.push(table.fixing()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64));
        cells.push(XLOPER12::from_f64(table.accrual_periods()[row]));
        cells.push(XLOPER12::from_str(table.currencies()[row].as_str()));
        cells.push(table.floorlet_strikes()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64));
        cells.push(table.caplet_strikes()[row].map_or_else(XLOPER12::nil, XLOPER12::from_f64));
        cells.push(XLOPER12::from_int(
            i32::try_from(table.leg_indices()[row]).unwrap_or(i32::MAX),
        ));
    }
    Ok(crate::functions::helpers::spill(
        cells, rows, COLUMNS, transpose, headers,
    ))
}
