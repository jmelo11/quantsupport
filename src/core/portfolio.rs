//! Aggregate completed, numeric trade results without retaining an AD graph.

use crate::math::compensatedsum::CompensatedSum;
use crate::prelude::{
    CashflowsTable, Currency, Date, EvaluationResults, QSError, Request, SensitivityMap,
};
use crate::utils::errors::Result;
use std::collections::{BTreeMap, HashSet};

/// Ordered trade collection. Repeated entries represent repeated positions.
pub struct Portfolio<T> {
    trades: Vec<T>,
}

impl<T> Portfolio<T> {
    /// Creates a nonempty portfolio, preserving order and multiplicity.
    ///
    /// # Errors
    /// Rejects an empty trade list.
    pub fn new(trades: Vec<T>) -> Result<Self> {
        if trades.is_empty() {
            return Err(QSError::InvalidValueErr(
                "A portfolio requires at least one trade".into(),
            ));
        }
        Ok(Self { trades })
    }

    /// Returns the original trade list.
    #[must_use]
    pub fn trades(&self) -> &[T] {
        &self.trades
    }

    /// Prices each trade and immediately aggregates its numeric results.
    ///
    /// The callback completes differentiation before returning and identifies
    /// the currency of its monetary PV and derivatives. Cashflows retain their
    /// own currencies. Built-in pricers reuse their per-trade AD tape; no AD
    /// values or per-trade result collection are retained by this method.
    /// Unsupported aggregate requests are ignored. A measure omitted by any
    /// member remains absent from the returned standard [`EvaluationResults`].
    /// PV and sensitivities must share one pricing currency. Cashflow compression
    /// retains coupon types, periods, indices, conventions and pay/receive
    /// direction, weighting compatible rates/spreads by signed principal.
    ///
    /// # Errors
    /// Rejects absent value currencies for supplied monetary measures,
    /// inconsistent dates, nonfinite results, or any trade pricing failure.
    /// No partial portfolio result is returned on failure.
    pub fn evaluate(
        &self,
        requests: &[Request],
        mut evaluate: impl FnMut(usize, &T, &[Request]) -> Result<(Option<Currency>, EvaluationResults)>,
    ) -> Result<EvaluationResults> {
        let mut unique = Vec::new();
        for request in requests {
            if matches!(
                request,
                Request::Value | Request::Sensitivities | Request::Cashflows
            ) && !unique.contains(request)
            {
                unique.push(*request);
            }
        }
        let mut results = PortfolioAccumulator {
            requests: unique.clone(),
            ..PortfolioAccumulator::default()
        };
        for (index, trade) in self.trades.iter().enumerate() {
            let (currency, value) = evaluate(index, trade, &unique).map_err(|error| {
                error.context(format!("pricing portfolio member {}", index + 1))
            })?;
            results.add(currency, &value).map_err(|error| {
                error.context(format!(
                    "aggregating portfolio member {} ({})",
                    index + 1,
                    value.identifier()
                ))
            })?;
        }
        results.finish()
    }
}

/// Signed undiscounted payments grouped by payment date and original currency.
pub struct PortfolioCashflow {
    /// Actual payment date; historical payments are retained.
    pub payment_date: Date,
    /// Currency of the contractual payments.
    pub currency: Currency,
    /// Net fixed, floating and option-embedded coupon payments.
    pub interest: f64,
    /// Net redemptions less disbursements, using native cashflow signs.
    pub capital: f64,
    /// Payments with a type that does not identify interest or capital.
    pub other: f64,
}

impl PortfolioCashflow {
    /// Returns the sum of the three payment categories.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.interest + self.capital + self.other
    }
}

struct CashflowSums {
    currency: Currency,
    interest: CompensatedSum,
    capital: CompensatedSum,
    other: CompensatedSum,
}

#[derive(Default)]
struct PortfolioAccumulator {
    requests: Vec<Request>,
    unavailable: HashSet<Request>,
    evaluation_date: Option<Date>,
    value_currency: Option<Currency>,
    value: CompensatedSum,
    sensitivities: BTreeMap<String, CompensatedSum>,
    cashflows: super::cashflowaggregation::CashflowCompressor,
}

impl PortfolioAccumulator {
    fn add(&mut self, currency: Option<Currency>, result: &EvaluationResults) -> Result<()> {
        if self
            .evaluation_date
            .is_some_and(|date| date != result.evaluation_date())
        {
            return Err(QSError::InvalidValueErr(
                "Portfolio members have different evaluation dates".into(),
            ));
        }
        self.evaluation_date = Some(result.evaluation_date());
        if let Some(currency) = currency {
            if self
                .requests
                .iter()
                .any(|request| matches!(request, Request::Value | Request::Sensitivities))
            {
                if self
                    .value_currency
                    .is_some_and(|previous| previous != currency)
                {
                    return Err(QSError::InvalidValueErr("Portfolio PV and sensitivities require one common pricing currency; use a common discount policy or aggregate currency portfolios separately".into()));
                }
                self.value_currency = Some(currency);
            }
        }
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
                    currency.ok_or_else(|| QSError::InvalidValueErr("Supply the pricing currency for every portfolio member when requesting Value or Sensitivities".into()))?;
                    self.value.add(value)?;
                }
                Request::Sensitivities => {
                    let Some(risk) = result.sensitivities() else {
                        self.unavailable.insert(*request);
                        continue;
                    };
                    currency.ok_or_else(|| QSError::InvalidValueErr("Supply the pricing currency for every portfolio member when requesting Value or Sensitivities".into()))?;
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
                    self.cashflows.add(flows)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<EvaluationResults> {
        let date = self
            .evaluation_date
            .ok_or_else(|| QSError::ValueNotSetErr("Portfolio evaluation date".into()))?;
        let mut result = EvaluationResults::new(date, "Portfolio".into());
        if self.requests.contains(&Request::Value) && !self.unavailable.contains(&Request::Value) {
            result = result.with_price(self.value.total());
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
            result = result.with_cashflows(self.cashflows.finish());
        }
        Ok(result)
    }
}

/// Groups a cashflow table into signed interest, capital and other payments.
///
/// # Errors
/// Rejects nonfinite amounts or aggregation overflow.
pub fn summarize_cashflows(flows: &CashflowsTable) -> Result<Vec<PortfolioCashflow>> {
    let mut rows = BTreeMap::new();
    add_cashflows(&mut rows, flows)?;
    Ok(rows
        .into_iter()
        .map(|((date, _), sums)| PortfolioCashflow {
            payment_date: date,
            currency: sums.currency,
            interest: sums.interest.total(),
            capital: sums.capital.total(),
            other: sums.other.total(),
        })
        .collect())
}

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
        let sum = match flows.cashflow_types()[row].as_str() {
            "FixedRateCoupon" | "FloatingRateCoupon" | "OptionEmbeddedCoupon" | "Interest" => {
                &mut sums.interest
            }
            "Redemption" | "Disbursement" | "Capital" => &mut sums.capital,
            _ => &mut sums.other,
        };
        sum.add(flows.amounts()[row])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
                kind.into(),
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
            "FixedRateCoupon".into(),
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
        let portfolio = Portfolio::new(vec![1, 1, 2])?;
        let results = portfolio.evaluate(
            &[
                Request::Value,
                Request::Cashflows,
                Request::Sensitivities,
                Request::Value,
            ],
            |index, _, requests| {
                assert_eq!(requests.len(), 3);
                Ok((Some(Currency::USD), result(index)))
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
        let portfolio = Portfolio::new(vec![0, 1])?;
        let results = portfolio.evaluate(
            &[Request::FairRate(0), Request::YieldToMaturity],
            |index, _, requests| {
                assert!(requests.is_empty());
                Ok((None, result(index)))
            },
        )?;
        assert!(
            results.price().is_none() && results.fair_rate().is_none() && results.ytm().is_none()
        );
        let results = portfolio.evaluate(
            &[Request::Value, Request::Cashflows, Request::Sensitivities],
            |index, _, _| {
                Ok((
                    Some(Currency::USD),
                    if index == 0 {
                        result(index)
                    } else {
                        EvaluationResults::new(Date::new(2026, 1, 15), "missing".into())
                    },
                ))
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
    fn rejects_mixed_pv_units_inconsistent_dates_and_nonfinite_values() -> Result<()> {
        let portfolio = Portfolio::new(vec![0, 1])?;
        assert!(portfolio
            .evaluate(&[Request::Value], |index, _, _| Ok((
                Some(if index == 0 {
                    Currency::USD
                } else {
                    Currency::EUR
                }),
                result(index)
            )))
            .is_err());
        assert!(portfolio
            .evaluate(&[Request::Value], |index, _, _| Ok((None, result(index))))
            .is_err());
        assert!(portfolio
            .evaluate(&[Request::Cashflows], |index, _, _| Ok((
                None,
                result(index).with_evaluation_date(Date::new(
                    2026,
                    1,
                    if index == 0 { 15 } else { 16 }
                ))
            )))
            .is_err());
        assert!(portfolio
            .evaluate(&[Request::Value], |index, _, _| Ok((
                Some(Currency::USD),
                result(index).with_price(f64::NAN)
            )))
            .is_err());
        assert!(Portfolio::<()>::new(vec![]).is_err());
        Ok(())
    }

    #[test]
    fn compensated_sums_preserve_small_exposures_after_large_offsets() -> Result<()> {
        let portfolio = Portfolio::new(vec![1e16, 1.0, -1e16])?;
        let result = portfolio.evaluate(&[Request::Value], |_, value, _| {
            Ok((Some(Currency::USD), result(0).with_price(*value)))
        })?;
        assert_eq!(result.price(), Some(1.0));
        Ok(())
    }
}
