use crate::{
    core::cashflowaggregation::CouponDetails,
    currencies::currency::Currency,
    rates::interestrate::{InterestRate, RateDefinition},
    time::date::Date,
    utils::errors::{QSError, Result},
};
use std::collections::HashMap;

/// Economic cashflow kind used by numeric reports and aggregators.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CashflowKind {
    /// Fixed coupon interest.
    FixedRateCoupon,
    /// Floating coupon interest.
    FloatingRateCoupon,
    /// Coupon with an option payoff.
    OptionEmbeddedCoupon,
    /// Principal returned at maturity or amortization.
    Redemption,
    /// Principal advanced at origination.
    Disbursement,
}

impl CashflowKind {
    /// Returns the external report label.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::FixedRateCoupon => "FixedRateCoupon",
            Self::FloatingRateCoupon => "FloatingRateCoupon",
            Self::OptionEmbeddedCoupon => "OptionEmbeddedCoupon",
            Self::Redemption => "Redemption",
            Self::Disbursement => "Disbursement",
        }
    }
}

impl TryFrom<&str> for CashflowKind {
    type Error = QSError;

    fn try_from(label: &str) -> Result<Self> {
        match label {
            "FixedRateCoupon" => Ok(Self::FixedRateCoupon),
            "FloatingRateCoupon" => Ok(Self::FloatingRateCoupon),
            "OptionEmbeddedCoupon" => Ok(Self::OptionEmbeddedCoupon),
            "Redemption" => Ok(Self::Redemption),
            "Disbursement" => Ok(Self::Disbursement),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid cashflow kind: {label}"
            ))),
        }
    }
}

impl std::fmt::Display for CashflowKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One signed, undiscounted cashflow borrowed from a [`CashflowsTable`].
///
/// Scalar fields are copied; the kind and contractual coupon terms borrow the
/// table. Iterating rows does not allocate or clone coupon metadata.
#[derive(Clone, Copy, Debug)]
pub struct CashflowRow<'a> {
    /// Contractual coupon terms, when available.
    pub coupon_details: Option<&'a CouponDetails>,
    /// Actual payment date.
    pub payment_date: Date,
    /// Economic cashflow kind.
    pub kind: &'a CashflowKind,
    /// Signed, undiscounted payment in the cashflow's currency.
    pub amount: f64,
    /// Resolved floating coupon rate, in decimal units, when applicable.
    pub fixing: Option<f64>,
    /// Contractual term-rate observation date, when applicable.
    pub fixing_date: Option<Date>,
    /// Coupon year fraction, or zero for non-accruing cashflows.
    pub accrual_period: f64,
    /// Original payment currency.
    pub currency: Currency,
    /// Embedded floorlet strike, when present.
    pub floorlet_strike: Option<f64>,
    /// Embedded caplet strike, when present.
    pub caplet_strike: Option<f64>,
    /// Zero-based leg index within the reporting instrument.
    pub leg_index: usize,
}

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
    coupon_details: Vec<Option<CouponDetails>>,
    payment_dates: Vec<Date>,
    cashflow_types: Vec<CashflowKind>,
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
            coupon_details: Vec::new(),
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

    /// Iterates over borrowed cashflows in the report's original row order.
    ///
    /// The table keeps its column-oriented storage; each row provides named
    /// fields for values at the same position across all columns.
    #[must_use = "iterators are lazy and do nothing unless consumed"]
    pub fn rows(&self) -> impl ExactSizeIterator<Item = CashflowRow<'_>> + DoubleEndedIterator {
        (0..self.payment_dates.len()).map(|row| CashflowRow {
            coupon_details: self.coupon_details[row].as_ref(),
            payment_date: self.payment_dates[row],
            kind: &self.cashflow_types[row],
            amount: self.amount[row],
            fixing: self.fixing[row],
            fixing_date: self.coupon_details[row]
                .as_ref()
                .and_then(CouponDetails::fixing_date),
            accrual_period: self.accrual_periods[row],
            currency: self.currencies[row],
            floorlet_strike: self.floorlet_strike[row],
            caplet_strike: self.caplet_strike[row],
            leg_index: self.leg_indices[row],
        })
    }

    /// Selects payments strictly after a cutoff, or retains the entire schedule.
    /// Filtering compacts columns in place and preserves their order.
    #[must_use]
    pub fn with_payment_cutoff(mut self, cutoff: Option<Date>) -> Self {
        if let Some(cutoff) = cutoff {
            fn retain<T>(values: &mut Vec<T>, dates: &[Date], cutoff: Date) {
                let mut row = 0;
                values.retain(|_| {
                    let keep = dates[row] > cutoff;
                    row += 1;
                    keep
                });
            }
            retain(&mut self.coupon_details, &self.payment_dates, cutoff);
            retain(&mut self.cashflow_types, &self.payment_dates, cutoff);
            retain(&mut self.amount, &self.payment_dates, cutoff);
            retain(&mut self.fixing, &self.payment_dates, cutoff);
            retain(&mut self.accrual_periods, &self.payment_dates, cutoff);
            retain(&mut self.currencies, &self.payment_dates, cutoff);
            retain(&mut self.floorlet_strike, &self.payment_dates, cutoff);
            retain(&mut self.caplet_strike, &self.payment_dates, cutoff);
            retain(&mut self.leg_indices, &self.payment_dates, cutoff);
            self.payment_dates.retain(|date| *date > cutoff);
        }
        self
    }

    /// Returns the payment dates of the cashflows.
    #[must_use]
    pub fn payment_dates(&self) -> &[Date] {
        &self.payment_dates
    }

    /// Returns contractual coupon metadata, including signed notional and fixing index.
    #[must_use]
    pub fn coupon_details(&self) -> &[Option<CouponDetails>] {
        &self.coupon_details
    }

    /// Computes coupon rates from actual reported interest under the given convention.
    ///
    /// Rows without contractual coupon terms have no implied rate. This includes
    /// net interest with zero principal and non-interest payments.
    ///
    /// # Errors
    /// Rejects invalid principal, accrual dates, interest or rate conventions.
    pub fn implied_rates(
        &self,
        convention: RateDefinition,
    ) -> Result<Vec<Option<InterestRate<f64>>>> {
        self.coupon_details
            .iter()
            .zip(&self.amount)
            .map(|(terms, interest)| {
                terms
                    .as_ref()
                    .map(|terms| terms.implied_rate(*interest, convention))
                    .transpose()
            })
            .collect()
    }

    /// Attaches contractual metadata to the most recently added cashflow.
    pub fn set_last_coupon_details(&mut self, details: CouponDetails) {
        if let Some(last) = self.coupon_details.last_mut() {
            *last = Some(details);
        }
    }

    /// Returns the cashflow types.
    #[must_use]
    pub fn cashflow_types(&self) -> &[CashflowKind] {
        &self.cashflow_types
    }

    /// Returns signed, undiscounted amounts in each row's currency.
    #[must_use]
    pub fn amounts(&self) -> &[f64] {
        &self.amount
    }

    /// Iterates contractual term-rate fixing dates without allocating a column copy.
    pub fn fixing_dates(&self) -> impl ExactSizeIterator<Item = Option<Date>> + '_ {
        self.coupon_details
            .iter()
            .map(|terms| terms.as_ref().and_then(CouponDetails::fixing_date))
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
        cashflow_type: CashflowKind,
        amount: f64,
        fixing: Option<f64>,
        accrual_period: f64,
        currency: Currency,
        floorlet_strike: Option<f64>,
        caplet_strike: Option<f64>,
        leg_index: usize,
    ) {
        self.coupon_details.push(None);
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
/// not be supported. Pricers leave unsupported result fields absent, so callers
/// should check the relevant accessor.
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
    /// Signed, undiscounted coupon interest for the requested date window.
    accrued_amount: Option<f64>,
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
            accrued_amount: None,
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

    /// Sets earned coupon interest for `Request::AccruedAmount`.
    #[must_use]
    pub const fn with_accrued_amount(mut self, amount: f64) -> Self {
        self.accrued_amount = Some(amount);
        self
    }

    /// Returns signed, undiscounted interest for the requested accrual window.
    #[must_use]
    pub const fn accrued_amount(&self) -> Option<f64> {
        self.accrued_amount
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{DayCounter, MarketIndex, QSError};

    fn report() -> CashflowsTable {
        let mut flows = CashflowsTable::new();
        flows.add_cashflow(
            Date::new(2026, 6, 5),
            CashflowKind::FixedRateCoupon,
            -20.0,
            None,
            120.0 / 360.0,
            Currency::EUR,
            None,
            None,
            0,
        );
        flows.set_last_coupon_details(
            CouponDetails::new(
                Date::new(2026, 1, 31),
                Date::new(2026, 5, 31),
                -1_000.0,
                DayCounter::Thirty360,
            )
            .with_fixed_rate(0.06),
        );
        flows.add_cashflow(
            Date::new(2026, 7, 1),
            CashflowKind::FloatingRateCoupon,
            500.0 * 0.04 * 91.0 / 360.0,
            Some(0.03),
            91.0 / 360.0,
            Currency::EUR,
            None,
            None,
            0,
        );
        flows.set_last_coupon_details(
            CouponDetails::new(
                Date::new(2026, 4, 1),
                Date::new(2026, 7, 1),
                500.0,
                DayCounter::Actual360,
            )
            .with_floating_rate(MarketIndex::EURIBOR3m, 0.01),
        );
        flows.add_cashflow(
            Date::new(2026, 4, 1),
            CashflowKind::Redemption,
            1e9,
            None,
            0.0,
            Currency::USD,
            None,
            None,
            0,
        );
        flows
    }

    #[test]
    fn report_kinds_are_closed_and_reject_synthetic_categories() {
        for label in ["Capital", "Interest", "ConstantAmount", "Other", "Unknown"] {
            assert!(CashflowKind::try_from(label).is_err());
        }
        assert_eq!(
            CashflowKind::try_from("Redemption").ok(),
            Some(CashflowKind::Redemption)
        );
    }

    #[test]
    fn row_iteration_keeps_report_order_and_borrows_coupon_terms() -> Result<()> {
        let flows = report();
        let mut rows = flows.rows();
        assert_eq!(rows.len(), 3);

        // Reports can be nonchronological; traversing from either end must
        // retain their original order and each cashflow's own metadata.
        let first = rows
            .next()
            .ok_or_else(|| QSError::NotFoundErr("First cashflow".into()))?;
        let last = rows
            .next_back()
            .ok_or_else(|| QSError::NotFoundErr("Last cashflow".into()))?;
        assert_eq!(first.payment_date, Date::new(2026, 6, 5));
        assert_eq!(first.amount, -20.0);
        assert_eq!(first.currency, Currency::EUR);
        let terms = first
            .coupon_details
            .ok_or_else(|| QSError::ValueNotSetErr("Coupon terms".into()))?;
        assert!(flows.coupon_details[0]
            .as_ref()
            .is_some_and(|stored| std::ptr::eq(terms, stored)));
        assert_eq!(last.payment_date, Date::new(2026, 4, 1));
        assert_eq!(*last.kind, CashflowKind::Redemption);
        assert_eq!(last.currency, Currency::USD);
        assert!(last.coupon_details.is_none());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows.next().and_then(|flow| flow.fixing), Some(0.03));
        assert!(rows.next().is_none());
        assert!(rows.next_back().is_none());
        assert_eq!(CashflowsTable::new().rows().len(), 0);
        Ok(())
    }
}
