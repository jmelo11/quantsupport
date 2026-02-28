use crate::{currencies::currency::Currency, time::date::Date};

/// Contains the cashflow structure of the instrument.
#[derive(Clone, Debug)]
pub struct CashflowsTable {
    payment_dates: Vec<Date>,
    cashflow_types: Vec<String>,
    amount: Vec<f64>,
    fixing: Vec<Option<f64>>,
    accrual_periods: Vec<f64>,
    currencies: Vec<Currency>,
    fx_parity: Vec<f64>,
    floorlet_strike: Vec<Option<f64>>,
    caplet_strike: Vec<Option<f64>>,
}

impl CashflowsTable {
    /// Creates a new [`CashflowsTable`] with empty vectors.
    #[must_use]
    pub fn new() -> Self {
        Self {
            payment_dates: Vec::new(),
            cashflow_types: Vec::new(),
            amount: Vec::new(),
            fixing: Vec::new(),
            accrual_periods: Vec::new(),
            currencies: Vec::new(),
            fx_parity: Vec::new(),
            floorlet_strike: Vec::new(),
            caplet_strike: Vec::new(),
        }
    }

    /// Returns the payment dates of the cashflows.
    #[must_use]
    pub fn payment_dates(&self) -> &[Date] {
        &self.payment_dates
    }

    /// Returns the cashflow types.
    #[must_use]
    pub fn cashflow_types(&self) -> &[String] {
        &self.cashflow_types
    }

    /// Returns the amounts.
    #[must_use]
    pub fn amounts(&self) -> &[f64] {
        &self.amount
    }

    /// Returns the fixing values.
    #[must_use]
    pub fn fixing(&self) -> &[Option<f64>] {
        &self.fixing
    }

    /// Returns the accrual periods.
    #[must_use]
    pub fn accrual_periods(&self) -> &[f64] {
        &self.accrual_periods
    }

    /// Returns the currencies.
    #[must_use]
    pub fn currencies(&self) -> &[Currency] {
        &self.currencies
    }

    /// Returns the FX parities.
    #[must_use]
    pub fn fx_parities(&self) -> &[f64] {
        &self.fx_parity
    }

    /// Adds a cashflow entry to the table.
    pub fn add_cashflow(
        &mut self,
        payment_date: Date,
        cashflow_type: String,
        amount: f64,
        fixing: Option<f64>,
        accrual_period: f64,
        currency: Currency,
        fx_parity: f64,
        floorlet_strike: Option<f64>,
        caplet_strike: Option<f64>,
    ) {
        self.payment_dates.push(payment_date);
        self.cashflow_types.push(cashflow_type);
        self.amount.push(amount);
        self.fixing.push(fixing);
        self.accrual_periods.push(accrual_period);
        self.currencies.push(currency);
        self.fx_parity.push(fx_parity);
        self.floorlet_strike.push(floorlet_strike);
        self.caplet_strike.push(caplet_strike);
    }
}

impl Default for CashflowsTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Maps sensitivity keys to values.
#[derive(Default, Debug)]
pub struct SensitivityMap {
    instrument_key: Vec<String>,
    exposure: Vec<f64>,
}

impl SensitivityMap {
    /// Returns instrument keys.
    #[must_use]
    pub fn instrument_keys(&self) -> &[String] {
        &self.instrument_key
    }

    /// Returns exposures.
    #[must_use]
    pub fn exposure(&self) -> &[f64] {
        &self.exposure
    }

    /// Sets the instrument keys for this sensitivity map.
    #[must_use]
    pub fn with_instrument_keys(mut self, instrument_keys: &[String]) -> Self {
        instrument_keys.clone_into(&mut self.instrument_key);
        self
    }

    /// Sets the exposure values for this sensitivity map.
    #[must_use]
    pub fn with_exposure(mut self, exposure: &[f64]) -> Self {
        exposure.clone_into(&mut self.exposure);
        self
    }
}

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
    /// Fair (par / breakeven) rate.
    fair_rate: Option<f64>,
}

impl EvaluationResults {
    /// Creates a new instance of [`EvaluationResults`].
    #[must_use]
    pub const fn new(evaluation_date: Date, identifier: String) -> Self {
        Self {
            evaluation_date,
            identifier,
            price: None,
            ytm: None,
            sensitivities: None,
            cashflows: None,
            fair_rate: None,
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

    /// Returns sensitivities if available.
    #[must_use]
    pub const fn sensitivities(&self) -> Option<&SensitivityMap> {
        self.sensitivities.as_ref()
    }

    /// Sets the sensitivities to market inputs.
    #[must_use]
    pub fn with_sensitivities(mut self, sensitivities: SensitivityMap) -> Self {
        self.sensitivities = Some(sensitivities);
        self
    }

    /// Sets the cashflows of the instrument.
    #[must_use]
    pub fn with_cashflows(mut self, cashflows: CashflowsTable) -> Self {
        self.cashflows = Some(cashflows.clone());
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

    /// Sets the fair (par / breakeven) rate.
    #[must_use]
    pub const fn with_fair_rate(mut self, fair_rate: f64) -> Self {
        self.fair_rate = Some(fair_rate);
        self
    }

    /// Returns the fair rate if available.
    #[must_use]
    pub const fn fair_rate(&self) -> Option<f64> {
        self.fair_rate
    }
}
