//! Aggregate completed, numeric trade results without retaining an AD graph.

use crate::core::cashflowaggregation::{CashflowAggregator, GrossCashflowAggregator};
use crate::core::instrument::Instrument;
use crate::math::compensatedsum::CompensatedSum;
use crate::prelude::{
    CashflowKind, CashflowsTable, Currency, Date, EvaluationResults, QSError, Request,
    SensitivityMap,
};
use crate::utils::errors::Result;
use std::collections::{BTreeMap, HashSet};

/// Ordered trade collection. Repeated entries represent repeated positions.
///
/// Valuation belongs to [`PortfolioPricer`](crate::pricers::portfoliopricer::PortfolioPricer).
pub struct Portfolio<T> {
    identifier: String,
    trades: Vec<T>,
}

impl<T> Portfolio<T> {
    /// Creates a nonempty portfolio, preserving order and multiplicity.
    ///
    /// # Errors
    /// Rejects a blank identifier or an empty trade list.
    pub fn new(identifier: String, trades: Vec<T>) -> Result<Self> {
        if identifier.trim().is_empty() {
            return Err(QSError::InvalidValueErr(
                "A portfolio requires an identifier".into(),
            ));
        }
        if trades.is_empty() {
            return Err(QSError::InvalidValueErr(
                "A portfolio requires at least one trade".into(),
            ));
        }
        Ok(Self { identifier, trades })
    }

    /// Returns the caller-supplied portfolio identifier.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the original trade list.
    #[must_use]
    pub fn trades(&self) -> &[T] {
        &self.trades
    }
}

impl<T: Send + Sync> Instrument for Portfolio<T> {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

/// Forecast undiscounted payments grouped by payment date and original currency.
pub struct PortfolioCashflowForecast {
    /// Actual payment date; historical payments are retained.
    payment_date: Date,
    /// Currency of the contractual payments.
    currency: Currency,
    /// Net fixed, floating and option-embedded coupon payments.
    interest: f64,
    /// Net redemptions less disbursements, using native cashflow signs.
    capital: f64,
    /// Payments with a type that does not identify interest or capital.
    other: f64,
}

impl PortfolioCashflowForecast {
    /// Returns the forecast payment date, including retained historical payments.
    #[must_use]
    pub const fn payment_date(&self) -> Date {
        self.payment_date
    }

    /// Returns the original payment currency.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }

    /// Returns forecast coupon interest.
    #[must_use]
    pub const fn interest(&self) -> f64 {
        self.interest
    }

    /// Returns redemptions less disbursements.
    #[must_use]
    pub const fn capital(&self) -> f64 {
        self.capital
    }

    /// Returns other forecast payments.
    #[must_use]
    pub const fn other(&self) -> f64 {
        self.other
    }

    /// Returns the sum of the three payment categories.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.interest + self.capital + self.other
    }
}

/// Compensated payment-category totals for one date and currency.
struct CashflowSums {
    currency: Currency,
    interest: CompensatedSum,
    capital: CompensatedSum,
    other: CompensatedSum,
}

/// Incrementally combines numeric results using an injected cashflow strategy.
///
/// Use [`MakePortfolioAccumulator`](crate::core::makeportfolioaccumulator::MakePortfolioAccumulator) for stepwise construction.
pub struct PortfolioAccumulator<A = GrossCashflowAggregator> {
    requests: Vec<Request>,
    unavailable: HashSet<Request>,
    evaluation_date: Option<Date>,
    value: CompensatedSum,
    accrued_amount: CompensatedSum,
    sensitivities: BTreeMap<String, CompensatedSum>,
    cashflows: A,
}

impl<A: CashflowAggregator> PortfolioAccumulator<A> {
    /// Creates an empty accumulator with the requested measures and cashflow strategy.
    ///
    /// Unsupported requests and duplicates are removed, preserving request order.
    #[must_use]
    pub fn new(requests: &[Request], cashflow_aggregator: A) -> Self {
        let mut unique = Vec::new();
        for request in requests {
            if matches!(
                request,
                Request::Value
                    | Request::Sensitivities
                    | Request::Cashflows
                    | Request::AccruedAmount(_, _)
            ) && !unique.contains(request)
            {
                unique.push(*request);
            }
        }
        Self {
            requests: unique,
            unavailable: HashSet::new(),
            evaluation_date: None,
            value: CompensatedSum::default(),
            accrued_amount: CompensatedSum::default(),
            sensitivities: BTreeMap::new(),
            cashflows: cashflow_aggregator,
        }
    }

    /// Returns supported, deduplicated aggregate requests in their original order.
    #[must_use]
    pub fn requests(&self) -> &[Request] {
        &self.requests
    }

    /// Adds one completed trade result.
    ///
    /// # Errors
    /// Rejects inconsistent dates and nonfinite values.
    /// Discard the accumulator after an error; no partial result should be published.
    pub fn accumulate_trade_result(&mut self, result: &EvaluationResults) -> Result<()> {
        Request::validate(&self.requests)?;
        if self
            .evaluation_date
            .is_some_and(|date| date != result.evaluation_date())
        {
            return Err(QSError::InvalidValueErr(
                "Portfolio members have different evaluation dates".into(),
            ));
        }
        self.evaluation_date = Some(result.evaluation_date());
        for request in &self.requests {
            if self.unavailable.contains(request) {
                continue;
            }
            match request {
                Request::Value => {
                    let Some(value) = result.price() else {
                        self.unavailable.insert(*request);
                        continue;
                    };
                    self.value.add(value)?;
                }
                Request::AccruedAmount(_, _) => {
                    let Some(value) = result.accrued_amount() else {
                        self.unavailable.insert(*request);
                        continue;
                    };
                    self.accrued_amount.add(value)?;
                }
                Request::Sensitivities => {
                    let Some(risk) = result.sensitivities() else {
                        self.unavailable.insert(*request);
                        continue;
                    };
                    if risk.instrument_keys().len() != risk.exposure().len() {
                        return Err(QSError::InvalidValueErr(
                            "Mismatched sensitivity keys and values".into(),
                        ));
                    }
                    for (key, value) in risk.instrument_keys().iter().zip(risk.exposure()) {
                        self.sensitivities
                            .entry(key.clone())
                            .or_default()
                            .add(*value)?;
                    }
                }
                Request::Cashflows => {
                    let Some(flows) = result.cashflows() else {
                        self.unavailable.insert(*request);
                        continue;
                    };
                    self.cashflows.accumulate_cashflows(flows)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Completes the portfolio result, omitting measures unavailable for any member.
    ///
    /// # Errors
    /// Rejects an empty accumulator or failure of the injected cashflow strategy.
    pub fn into_evaluation_results(self, identifier: String) -> Result<EvaluationResults> {
        let date = self
            .evaluation_date
            .ok_or_else(|| QSError::ValueNotSetErr("Portfolio evaluation date".into()))?;
        let mut result = EvaluationResults::new(date, identifier);
        if self.requests.contains(&Request::Value) && !self.unavailable.contains(&Request::Value) {
            result = result.with_price(self.value.total());
        }
        if self
            .requests
            .iter()
            .any(|r| matches!(r, Request::AccruedAmount(_, _)) && !self.unavailable.contains(r))
        {
            result = result.with_accrued_amount(self.accrued_amount.total());
        }
        if self.requests.contains(&Request::Sensitivities)
            && !self.unavailable.contains(&Request::Sensitivities)
        {
            let (keys, values): (Vec<_>, Vec<_>) = self
                .sensitivities
                .into_iter()
                .map(|(key, sum)| (key, sum.total()))
                .unzip();
            result = result.with_sensitivities(
                SensitivityMap::default()
                    .with_instrument_keys(&keys)
                    .with_exposure(&values),
            );
        }
        if self.requests.contains(&Request::Cashflows)
            && !self.unavailable.contains(&Request::Cashflows)
        {
            result = result.with_cashflows(self.cashflows.into_cashflows_table()?);
        }
        Ok(result)
    }
}

/// Groups a cashflow table into signed interest, capital and other payments.
///
/// # Errors
/// Rejects nonfinite amounts or aggregation overflow.
pub fn summarize_cashflows(flows: &CashflowsTable) -> Result<Vec<PortfolioCashflowForecast>> {
    let mut rows = BTreeMap::new();
    add_cashflows(&mut rows, flows)?;
    Ok(rows
        .into_iter()
        .map(|((date, _), sums)| PortfolioCashflowForecast {
            payment_date: date,
            currency: sums.currency,
            interest: sums.interest.total(),
            capital: sums.capital.total(),
            other: sums.other.total(),
        })
        .collect())
}

/// Accumulates signed payments by date, original currency and payment category.
fn add_cashflows(
    rows: &mut BTreeMap<(Date, String), CashflowSums>,
    flows: &CashflowsTable,
) -> Result<()> {
    for row in 0..flows.payment_dates().len() {
        let currency = flows.currencies()[row];
        let sums = rows
            .entry((flows.payment_dates()[row], currency.to_string()))
            .or_insert_with(|| CashflowSums {
                currency,
                interest: CompensatedSum::default(),
                capital: CompensatedSum::default(),
                other: CompensatedSum::default(),
            });
        let sum = match &flows.cashflow_types()[row] {
            CashflowKind::FixedRateCoupon
            | CashflowKind::FloatingRateCoupon
            | CashflowKind::OptionEmbeddedCoupon
            | CashflowKind::Interest => &mut sums.interest,
            CashflowKind::Redemption | CashflowKind::Disbursement | CashflowKind::Capital => {
                &mut sums.capital
            }
            _ => &mut sums.other,
        };
        sum.add(flows.amounts()[row])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{
        MakePortfolioAccumulator, MarketDataProvider, MarketDataRequest, PortfolioPricer, Pricer,
        PricingContext,
    };

    struct FixturePricer<F>(F);

    impl<F> Pricer for FixturePricer<F>
    where
        F: Fn(usize, &usize, &[Request]) -> Result<EvaluationResults> + Send + Sync,
    {
        type Item = usize;
        type Policy = ();

        fn evaluate(
            &self,
            trade: &usize,
            requests: &[Request],
            _context: &impl MarketDataProvider,
        ) -> Result<EvaluationResults> {
            (self.0)(*trade, trade, requests)
        }
        fn market_data_request(&self, _trade: &usize) -> Option<MarketDataRequest> {
            None
        }
        fn set_discount_policy(&mut self, _policy: Box<()>) {}
        fn discount_policy(&self) -> Option<&()> {
            None
        }
    }

    /// Evaluates numeric fixtures through the standard portfolio pricer.
    fn evaluate(
        portfolio: &Portfolio<usize>,
        requests: &[Request],
        member: impl Fn(usize, &usize, &[Request]) -> Result<EvaluationResults> + Send + Sync,
    ) -> Result<EvaluationResults> {
        PortfolioPricer::new(FixturePricer(member)).evaluate(
            portfolio,
            requests,
            &PricingContext::new(),
        )
    }

    /// Builds a completed trade result with signed mixed-currency payments and repeated risk labels.
    fn result(index: usize) -> EvaluationResults {
        let date = Date::new(2026, 1, 15);
        let mut flows = CashflowsTable::new();
        for (kind, amount) in [
            ("FixedRateCoupon", 10.0),
            ("FloatingRateCoupon", -4.0),
            ("Redemption", 100.0),
            ("Disbursement", -20.0),
            ("ConstantAmount", 2.0),
        ] {
            flows.add_cashflow(
                date,
                CashflowKind::from(kind),
                amount,
                None,
                0.0,
                Currency::USD,
                None,
                None,
                0,
            );
        }
        flows.add_cashflow(
            date,
            CashflowKind::FixedRateCoupon,
            7.0,
            None,
            0.0,
            Currency::EUR,
            None,
            None,
            0,
        );
        EvaluationResults::new(date, index.to_string())
            .with_price(12.0)
            .with_cashflows(flows)
            .with_sensitivities(
                SensitivityMap::default()
                    .with_instrument_keys(&["SOFR_1Y".into(), "SOFR_1Y".into(), "SOFR_2Y".into()])
                    .with_exposure(&[2.0, -0.5, -1.0]),
            )
    }

    #[test]
    fn compresses_results_and_separates_interest_capital_and_currencies() -> Result<()> {
        let portfolio = Portfolio::new("TEST-PORTFOLIO".into(), vec![1, 1, 2])?;
        let results = evaluate(
            &portfolio,
            &[
                Request::Value,
                Request::Cashflows,
                Request::Sensitivities,
                Request::Value,
            ],
            |index, _, requests| {
                assert_eq!(requests.len(), 3);
                Ok(result(index))
            },
        )?;
        assert_eq!(results.price(), Some(36.0));
        assert_eq!(
            results.sensitivities().map(SensitivityMap::instrument_keys),
            Some(["SOFR_1Y".into(), "SOFR_2Y".into()].as_slice())
        );
        assert_eq!(
            results.sensitivities().map(SensitivityMap::exposure),
            Some([4.5, -3.0].as_slice())
        );
        let rows = summarize_cashflows(
            results
                .cashflows()
                .ok_or_else(|| QSError::ValueNotSetErr("flows".into()))?,
        )?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].currency, Currency::EUR);
        assert_eq!(rows[0].interest, 21.0);
        assert_eq!(rows[0].capital, 0.0);
        assert_eq!(rows[1].interest, 18.0);
        assert_eq!(rows[1].capital, 240.0);
        assert_eq!(rows[1].other, 6.0);
        assert_eq!(rows[1].total(), 264.0);
        Ok(())
    }

    #[test]
    fn unsupported_and_missing_measures_remain_absent_without_errors_or_partial_totals(
    ) -> Result<()> {
        let portfolio = Portfolio::new("TEST-PORTFOLIO".into(), vec![0, 1])?;
        let results = evaluate(
            &portfolio,
            &[Request::FairRate(0), Request::YieldToMaturity],
            |index, _, requests| {
                assert!(requests.is_empty());
                Ok(result(index))
            },
        )?;
        assert!(
            results.price().is_none() && results.fair_rate().is_none() && results.ytm().is_none()
        );
        let results = evaluate(
            &portfolio,
            &[Request::Value, Request::Cashflows, Request::Sensitivities],
            |index, _, _| {
                Ok(if index == 0 {
                    result(index)
                } else {
                    EvaluationResults::new(Date::new(2026, 1, 15), "missing".into())
                })
            },
        )?;
        assert!(
            results.price().is_none()
                && results.cashflows().is_none()
                && results.sensitivities().is_none()
        );
        Ok(())
    }

    #[test]
    fn rejects_inconsistent_dates_and_nonfinite_values() -> Result<()> {
        let portfolio = Portfolio::new("TEST-PORTFOLIO".into(), vec![0, 1])?;
        assert!(
            evaluate(&portfolio, &[Request::Cashflows], |index, _, _| Ok(result(
                index
            )
            .with_evaluation_date(Date::new(
                2026,
                1,
                if index == 0 { 15 } else { 16 }
            ))))
            .is_err()
        );
        assert!(
            evaluate(&portfolio, &[Request::Value], |index, _, _| Ok(result(
                index
            )
            .with_price(f64::NAN)))
            .is_err()
        );
        assert!(Portfolio::<()>::new("TEST-PORTFOLIO".into(), vec![]).is_err());
        Ok(())
    }

    #[test]
    fn compensated_sums_preserve_small_exposures_after_large_offsets() -> Result<()> {
        let portfolio = Portfolio::new("TEST-PORTFOLIO".into(), vec![0, 1, 2])?;
        let result = evaluate(&portfolio, &[Request::Value], |_, value, _| {
            Ok(result(0).with_price([1e16, 1.0, -1e16][*value]))
        })?;
        assert_eq!(result.price(), Some(1.0));
        Ok(())
    }
    #[test]
    fn builder_injects_net_aggregation_and_rejects_empty_results() -> Result<()> {
        use crate::prelude::NetCashflowAggregator;
        assert!(matches!(
            MakePortfolioAccumulator::default().build(),
            Err(QSError::ValueNotSetErr(_))
        ));
        assert!(MakePortfolioAccumulator::default()
            .with_requests(&[Request::Value])
            .build()?
            .into_evaluation_results("TEST-PORTFOLIO".into())
            .is_err());
        let mut accumulator = MakePortfolioAccumulator::default()
            .with_requests(&[
                Request::Value,
                Request::Cashflows,
                Request::Value,
                Request::FairRate(0),
            ])
            .with_cashflow_aggregator(NetCashflowAggregator::default())
            .build()?;
        assert_eq!(
            accumulator.requests,
            vec![Request::Value, Request::Cashflows]
        );
        accumulator.accumulate_trade_result(&result(0))?;
        let result = accumulator.into_evaluation_results("TEST-PORTFOLIO".into())?;
        let flows = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("net cashflows".into()))?;
        assert_eq!(result.price(), Some(12.0));
        assert!(flows.cashflow_types().contains(&CashflowKind::Capital));
        assert!(!flows.cashflow_types().contains(&CashflowKind::Disbursement));
        Ok(())
    }

    #[test]
    fn injected_strategy_errors_propagate_with_member_context() -> Result<()> {
        /// A custom strategy that fails when consuming a table.
        struct RejectCashflows;
        impl CashflowAggregator for RejectCashflows {
            /// Rejects incoming cashflows to exercise portfolio aggregation error propagation.
            fn accumulate_cashflows(&mut self, _: &CashflowsTable) -> Result<()> {
                Err(QSError::InvalidValueErr("custom cashflow strategy".into()))
            }
            /// Returns an empty cashflow table for the deliberately rejecting test strategy.
            fn into_cashflows_table(self) -> Result<CashflowsTable> {
                Ok(CashflowsTable::new())
            }
        }
        let pricer = PortfolioPricer::new(FixturePricer(|index, _: &usize, _: &[Request]| {
            Ok(result(index))
        }))
        .with_cashflow_aggregator_factory(|| RejectCashflows);
        let error = pricer
            .evaluate(
                &Portfolio::new("TEST-PORTFOLIO".into(), vec![0])?,
                &[Request::Cashflows],
                &PricingContext::new(),
            )
            .err()
            .ok_or_else(|| QSError::ValueNotSetErr("strategy error".into()))?;
        let message = error.to_string();
        assert!(message.contains("custom cashflow strategy"));
        assert!(message.contains("aggregating portfolio member 1"));
        Ok(())
    }
    #[test]
    fn aggregates_accrued_amount_and_propagates_missing_members() -> Result<()> {
        let start = Date::new(2026, 1, 1);
        let end = Date::new(2026, 2, 1);
        let requests = [Request::AccruedAmount(start, end)];
        let mut accumulator = MakePortfolioAccumulator::default()
            .with_requests(&requests)
            .build()?;
        accumulator.accumulate_trade_result(&result(0).with_accrued_amount(12.0))?;
        accumulator.accumulate_trade_result(&result(1).with_accrued_amount(-4.0))?;
        let result = accumulator.into_evaluation_results("INTEREST".into())?;
        assert_eq!(result.accrued_amount(), Some(8.0));
        assert!(result.price().is_none());
        let mut accumulator = MakePortfolioAccumulator::default()
            .with_requests(&requests)
            .build()?;
        accumulator.accumulate_trade_result(&result)?;
        accumulator.accumulate_trade_result(&EvaluationResults::new(
            result.evaluation_date(),
            "MISSING".into(),
        ))?;
        assert!(accumulator
            .into_evaluation_results("PARTIAL".into())?
            .accrued_amount()
            .is_none());
        Ok(())
    }
}
