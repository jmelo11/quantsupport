//! Stepwise construction of portfolio accumulators.

use crate::{
    core::{
        cashflowaggregation::{CashflowAggregator, GrossCashflowAggregator},
        portfolio::PortfolioAccumulator,
        request::Request,
    },
    utils::errors::{QSError, Result},
};

/// A builder for creating a [`PortfolioAccumulator`].
///
/// Select the requests before building. Gross cashflow aggregation is the default;
/// use [`Self::with_cashflow_aggregator`] to inject another strategy.
///
/// # Example
/// ```
/// use quantsupport::prelude::*;
///
/// let mut accumulator = MakePortfolioAccumulator::default()
///     .with_requests(&[Request::Value, Request::Cashflows])
///     .with_cashflow_aggregator(NetCashflowAggregator::default())
///     .build()?;
/// let result = EvaluationResults::new(Date::new(2026, 1, 15), "trade".into())
///     .with_price(12.0)
///     .with_cashflows(CashflowsTable::new());
/// accumulator.accumulate_trade_result(&result)?;
/// assert_eq!(accumulator.into_evaluation_results("TEST-PORTFOLIO".into())?.price(), Some(12.0));
/// # Ok::<(), QSError>(())
/// ```
pub struct MakePortfolioAccumulator<A = GrossCashflowAggregator> {
    requests: Option<Vec<Request>>,
    cashflows: A,
}

impl Default for MakePortfolioAccumulator {
    fn default() -> Self {
        Self {
            requests: None,
            cashflows: GrossCashflowAggregator::default(),
        }
    }
}

impl<A> MakePortfolioAccumulator<A> {
    /// Sets the requested measures; duplicates and unsupported measures are ignored.
    /// An explicit empty request list is allowed.
    #[must_use]
    pub fn with_requests(mut self, requests: &[Request]) -> Self {
        self.requests = Some(requests.to_vec());
        self
    }

    /// Sets the strategy that aggregates each member's existing cashflow table.
    #[must_use]
    pub fn with_cashflow_aggregator<B: CashflowAggregator>(
        self,
        cashflow_aggregator: B,
    ) -> MakePortfolioAccumulator<B> {
        MakePortfolioAccumulator {
            requests: self.requests,
            cashflows: cashflow_aggregator,
        }
    }
}

impl<A: CashflowAggregator> MakePortfolioAccumulator<A> {
    /// Builds an empty [`PortfolioAccumulator`] with the selected strategy.
    ///
    /// # Errors
    /// Returns an error if the requests have not been set.
    pub fn build(self) -> Result<PortfolioAccumulator<A>> {
        let requests = self
            .requests
            .ok_or_else(|| QSError::ValueNotSetErr("Portfolio requests".into()))?;
        Request::validate(&requests)?;
        Ok(PortfolioAccumulator::new(&requests, self.cashflows))
    }
}
