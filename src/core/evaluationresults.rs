use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// # `CashflowsTable`
/// Contains the cashflow structure of the instrument.
pub struct CashflowsTable;

/// # `SensitivityKey`
/// Identifies a sensitivity by market index and curve pillar date.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct SensitivityKey {
    market_index: MarketIndex,
    pillar_date: Date,
}

/// # `SensitivityMap`
/// Maps sensitivity keys to values.
#[derive(Default)]
pub struct SensitivityMap {
    instrument_key: Vec<String>,
    exposure: Vec<f64>,
}

impl SensitivityMap {
    /// Creates a new sensitifity map
    pub fn new() -> Self {
        Self {
            instrument_key: Vec::new(),
            exposure: Vec::new(),
        }
    }

    pub fn with_instrument_keys(mut self, instrument_keys: Vec<String>) -> Self {
        self.instrument_key = instrument_keys.clone();
        self
    }

    pub fn with_exposure(mut self, exposure: Vec<f64>) -> Self {
        self.exposure = exposure.clone();
        self
    }
}

/// # `EvaluationResults`
///
/// Holds the results of an instrument evaluation, including price, sensitivities, among others.
pub struct EvaluationResults {
    /// Reference or as-of date of the results.
    evaluation_date: Date,
    /// Instrument name or identifier.
    identifier: String,
    /// Price or present value.
    price: Option<f64>,
    /// Yield to maturity.
    ytm: Option<f64>,
    /// Sensitivities to market inputs.    
    sensitivities: Option<SensitivityMap>,
    /// Cashflows of the instrument.
    cashflows: Option<CashflowsTable>,
}

impl EvaluationResults {
    /// Creates a new instance of `EvaluationResults`.
    #[must_use]
    pub const fn new(evaluation_date: Date, identifier: String) -> Self {
        Self {
            evaluation_date,
            identifier,
            price: None,
            ytm: None,
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

    /// Returns the price or present value.
    #[must_use]
    pub const fn price(&self) -> Option<f64> {
        self.price
    }

    /// Sets the sensitivities to market inputs.
    #[must_use]
    pub fn with_sensitivities(mut self, sensitivities: SensitivityMap) -> Self {
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
    pub const fn with_evaluation_date(mut self, evaluation_date: Date) -> Self {
        self.evaluation_date = evaluation_date;
        self
    }

    /// Sets the instrument name or identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = identifier;
        self
    }

    /// Sets the yield to maturity.
    #[must_use]
    pub const fn with_ytm(mut self, ytm: f64) -> Self {
        self.ytm = Some(ytm);
        self
    }
}
