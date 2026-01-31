use std::collections::HashMap;

use crate::prelude::Date;

/// # CashflowsTable
/// Contains the cashflow structure of the instrument.
pub struct CashflowsTable;

/// # `EvaluationResults`
pub struct EvaluationResults {
    /// Reference or as-of date of the results.
    reference_date: Date,
    /// Iternal id of the instrument.
    id: usize,
    /// Instrument name or identifier.
    identifier: &'static str,
    /// Price or present value.
    price: Option<f64>,
    /// Sensitivities to market inputs.    
    sensitivities: Option<HashMap<usize, f64>>,
    /// Cashflows of the instrument.
    cashflows: Option<CashflowsTable>,
}

impl EvaluationResults {
    /// Creates a new instance of `EvaluationResults`.
    pub fn new(reference_date: Date, id: usize, identifier: &'static str) -> Self {
        Self {
            reference_date,
            id,
            identifier,
            price: None,
            sensitivities: None,
            cashflows: None,
        }
    }
}
