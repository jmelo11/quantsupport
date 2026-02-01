use crate::time::date::Date;

/// # `CashflowsTable`
/// Contains the cashflow structure of the instrument.
pub struct CashflowsTable;

/// # `SensitivitiesTable`>
/// Contains the cashflow structure of the instrument.
pub struct SensitivitiesTable;

/// # `EvaluationResults`
///
/// Holds the results of an instrument evaluation, including price, sensitivities, among others.
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
    sensitivities: Option<SensitivitiesTable>,
    /// Cashflows of the instrument.
    cashflows: Option<CashflowsTable>,
}

impl EvaluationResults {
    /// Creates a new instance of `EvaluationResults`.
    #[must_use]
    pub const fn new(reference_date: Date, id: usize, identifier: &'static str) -> Self {
        Self {
            reference_date,
            id,
            identifier,
            price: None,
            sensitivities: None,
            cashflows: None,
        }
    }

    /// Sets the price or present value.
    #[must_use]
    pub const fn with_price(mut self, price: f64) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the sensitivities to market inputs.
    #[must_use]
    pub const fn with_sensitivities(mut self, sensitivities: SensitivitiesTable) -> Self {
        self.sensitivities = Some(sensitivities);
        self
    }

    /// Sets the cashflows of the instrument.
    #[must_use]
    pub const fn with_cashflows(mut self, cashflows: CashflowsTable) -> Self {
        self.cashflows = Some(cashflows);
        self
    }

    /// Sets the reference or as-of date.
    #[must_use]
    pub const fn with_reference_date(mut self, reference_date: Date) -> Self {
        self.reference_date = reference_date;
        self
    }

    /// Sets the instrument internal id.
    #[must_use]
    pub const fn with_id(mut self, id: usize) -> Self {
        self.id = id;
        self
    }

    /// Sets the instrument name or identifier.
    #[must_use]
    pub const fn with_identifier(mut self, identifier: &'static str) -> Self {
        self.identifier = identifier;
        self
    }
}
