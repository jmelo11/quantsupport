use std::collections::HashMap;

use crate::{currencies::currency::Currency, time::date::Date};

/// Column-oriented, undiscounted cashflow report.
///
/// Each position across the columns describes one cashflow in its own currency;
/// row order follows the reporting pricer and need not be chronological.
/// The default [`HandleCashflows`](crate::core::request::HandleCashflows)
/// implementation applies leg-side signs and reverses disbursement amounts:
/// positive amounts are receipts and negative amounts are payments.
/// Reconciling this table to NPV also requires the pricer's payment selection,
/// any FX conversion, and discount factors.
#[derive(Clone, Debug)]
pub struct CashflowsTable {
    payment_dates: Vec<Date>,
    cashflow_types: Vec<String>,
    amount: Vec<f64>,
    fixing: Vec<Option<f64>>,
    accrual_periods: Vec<f64>,
    currencies: Vec<Currency>,
    floorlet_strike: Vec<Option<f64>>,
    caplet_strike: Vec<Option<f64>>,
    leg_indices: Vec<usize>,
}

impl CashflowsTable {
    /// Creates a new [`CashflowsTable`] with empty vectors.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            payment_dates: Vec::new(),
            cashflow_types: Vec::new(),
            amount: Vec::new(),
            fixing: Vec::new(),
            accrual_periods: Vec::new(),
            currencies: Vec::new(),
            floorlet_strike: Vec::new(),
            caplet_strike: Vec::new(),
            leg_indices: Vec::new(),
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

    /// Returns signed, undiscounted amounts in each row's currency.
    #[must_use]
    pub fn amounts(&self) -> &[f64] {
        &self.amount
    }

    /// Returns resolved floating coupon rates, or `None` for non-floating rows.
    ///
    /// Rate values are decimal, not percentage points.
    #[must_use]
    pub fn fixing(&self) -> &[Option<f64>] {
        &self.fixing
    }

    /// Returns coupon year fractions under their day counters, or zero for
    /// non-accruing cashflows in the default report.
    #[must_use]
    pub fn accrual_periods(&self) -> &[f64] {
        &self.accrual_periods
    }

    /// Returns the currencies.
    #[must_use]
    pub fn currencies(&self) -> &[Currency] {
        &self.currencies
    }

    /// Returns floorlet strikes.
    #[must_use]
    pub fn floorlet_strikes(&self) -> &[Option<f64>] {
        &self.floorlet_strike
    }

    /// Returns caplet strikes.
    #[must_use]
    pub fn caplet_strikes(&self) -> &[Option<f64>] {
        &self.caplet_strike
    }

    /// Returns zero-based indices into the trade's reported legs.
    #[must_use]
    pub fn leg_indices(&self) -> &[usize] {
        &self.leg_indices
    }

    /// Adds a cashflow entry to the table.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cashflow(
        &mut self,
        payment_date: Date,
        cashflow_type: String,
        amount: f64,
        fixing: Option<f64>,
        accrual_period: f64,
        currency: Currency,
        floorlet_strike: Option<f64>,
        caplet_strike: Option<f64>,
        leg_index: usize,
    ) {
        self.payment_dates.push(payment_date);
        self.cashflow_types.push(cashflow_type);
        self.amount.push(amount);
        self.fixing.push(fixing);
        self.accrual_periods.push(accrual_period);
        self.currencies.push(currency);
        self.floorlet_strike.push(floorlet_strike);
        self.caplet_strike.push(caplet_strike);
        self.leg_indices.push(leg_index);
    }
}

impl Default for CashflowsTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Labelled first-order derivatives of the reported value with respect to pillars.
///
/// A label identifies the risk variable exposed by the corresponding market
/// object, not necessarily a rate or a quoted calibration instrument. Exposures
/// are raw derivatives per unit of that variable, in value units per pillar
/// unit; they are not automatically DV01 or one-percent vega.
/// For a decimal-rate pillar, `exposure * 1e-4` estimates the value change for
/// a positive one-basis-point move. Other quote types require their own scaling.
///
/// Callers constructing a map must supply equally sized, positionally aligned
/// key and exposure arrays.
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

    /// Returns raw derivatives aligned with [`Self::instrument_keys`].
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

    /// Aggregates duplicate keys by summing their exposures.
    ///
    /// When cross-curve IFT is active, a parent quote may appear both from
    /// the parent curve's own pillars and from the child curve's cross-curve
    /// pillar list.  This method merges them, preserving insertion order of
    /// the first occurrence.
    #[must_use]
    pub fn aggregate(self) -> Self {
        let mut order: Vec<String> = Vec::new();
        let mut sums: HashMap<String, f64> = HashMap::new();

        for (key, exp) in self.instrument_key.iter().zip(self.exposure.iter()) {
            if !sums.contains_key(key) {
                order.push(key.clone());
            }
            *sums.entry(key.clone()).or_insert(0.0) += exp;
        }

        let exposure: Vec<f64> = order.iter().map(|k| sums[k]).collect();
        Self {
            instrument_key: order,
            exposure,
        }
    }
}

/// Optional measures returned by a pricer for one position and evaluation date.
///
/// A field is populated only when the selected pricer supplies that measure.
/// `None` is not a zero result: the measure may not have been requested or may
/// not be supported. Some pricers reject unsupported requests; others leave the
/// field absent, so callers should check the relevant accessor.
///
/// Value currency, quotation units, and supported measures are pricer-specific.
/// The result does not itself carry a reporting currency. Built-in position
/// values are monetary present values, not necessarily clean bond prices or
/// prices normalized per 100 nominal.
pub struct EvaluationResults {
    /// Reference or as-of date of the results.
    evaluation_date: Date,
    /// Instrument name or identifier.
    identifier: String,
    /// Price or present value.
    price: Option<f64>,
    /// Yield to maturity.
    ytm: Option<f64>,
    /// Modified duration with respect to the bond's yield convention.
    modified_duration: Option<f64>,
    /// Continuously compounded zero-volatility spread over the discount curve.
    z_spread: Option<f64>,
    /// Option-adjusted spread.
    oas: Option<f64>,
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
            modified_duration: None,
            z_spread: None,
            oas: None,
            sensitivities: None,
            cashflows: None,
            fair_rate: None,
        }
    }

    /// Returns the reference or as-of date of the evaluation.
    #[must_use]
    pub const fn evaluation_date(&self) -> Date {
        self.evaluation_date
    }

    /// Returns the evaluated instrument identifier.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Sets the price or present value.
    #[must_use]
    pub const fn with_price(mut self, price: f64) -> Self {
        self.price = Some(price);
        self
    }

    /// Returns the requested value in the selected pricer's value units.
    ///
    /// Do not assume that the context's base currency automatically converts
    /// every pricer's output; the instrument and discount policy determine it.
    #[must_use]
    pub const fn price(&self) -> Option<f64> {
        self.price
    }

    /// Returns yield to maturity as a decimal annual rate, if available.
    ///
    /// Built-in bond pricers use the trade's yield compounding, frequency, and
    /// day-count definition. `0.05` denotes 5%, not five basis points.
    #[must_use]
    pub const fn ytm(&self) -> Option<f64> {
        self.ytm
    }

    /// Returns modified duration `-(dP/dy)/P`, if available.
    ///
    /// The derivative is with respect to the bond's decimal yield under its
    /// yield convention, not a parallel market-curve shift.
    #[must_use]
    pub const fn modified_duration(&self) -> Option<f64> {
        self.modified_duration
    }

    /// Returns the continuously compounded Z-spread as an annual decimal rate.
    ///
    /// Built-in bond pricers apply the trade's spread day counter.
    #[must_use]
    pub const fn z_spread(&self) -> Option<f64> {
        self.z_spread
    }

    /// Returns the issuer-call bond's option-adjusted spread, if available.
    ///
    /// Built-in bond engines use a continuously compounded annual decimal
    /// spread under the trade's spread day counter, not a basis-point number.
    #[must_use]
    pub const fn oas(&self) -> Option<f64> {
        self.oas
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
        self.cashflows = Some(cashflows);
        self
    }

    /// Returns cashflows if available.
    #[must_use]
    pub const fn cashflows(&self) -> Option<&CashflowsTable> {
        self.cashflows.as_ref()
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

    /// Sets the modified duration.
    #[must_use]
    pub const fn with_modified_duration(mut self, modified_duration: f64) -> Self {
        self.modified_duration = Some(modified_duration);
        self
    }

    /// Sets the Z-spread.
    #[must_use]
    pub const fn with_z_spread(mut self, z_spread: f64) -> Self {
        self.z_spread = Some(z_spread);
        self
    }

    /// Sets the option-adjusted spread.
    #[must_use]
    pub const fn with_oas(mut self, oas: f64) -> Self {
        self.oas = Some(oas);
        self
    }

    /// Sets the fair (par / breakeven) rate.
    #[must_use]
    pub const fn with_fair_rate(mut self, fair_rate: f64) -> Self {
        self.fair_rate = Some(fair_rate);
        self
    }

    /// Returns the par or breakeven quote, if available.
    ///
    /// Its meaning depends on the product: a swap coupon is an annual decimal
    /// rate, whereas an equity or FX forward returns a delivery price or
    /// exchange rate in the contract's quotation units.
    /// The target is selected by [`Request::FairRate`](crate::core::request::Request::FairRate).
    #[must_use]
    pub const fn fair_rate(&self) -> Option<f64> {
        self.fair_rate
    }
}
