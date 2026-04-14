use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    indices::marketindex::MarketIndex,
    instruments::cashflows::payoffops::PayoffOps,
};

/// A cashflow with an embedded option payoff (e.g. cap/floor).
#[derive(Clone)]
pub struct OptionEmbeddedCashflow<T: Scalar> {
    payoff_ops: PayoffOps,
    market_index: MarketIndex,
    value: T,
}

impl<T> OptionEmbeddedCashflow<T>
where
    T: Scalar,
{
    /// Creates a new [`OptionEmbeddedCashflow`] with the specified payoff operations, market index, and value.
    #[must_use]
    pub const fn new(payoff_ops: PayoffOps, market_index: MarketIndex, value: T) -> Self {
        Self {
            payoff_ops,
            market_index,
            value,
        }
    }

    /// Returns the payoff operations associated with this cashflow.
    #[must_use]
    pub const fn payoff_ops(&self) -> &PayoffOps {
        &self.payoff_ops
    }

    /// Returns the market index associated with this cashflow.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the value of this cashflow.
    #[must_use]
    pub const fn value(&self) -> T {
        self.value
    }
}

impl From<OptionEmbeddedCashflow<f64>> for OptionEmbeddedCashflow<DualFwd> {
    fn from(value: OptionEmbeddedCashflow<f64>) -> Self {
        Self::new(
            value.payoff_ops.clone(),
            value.market_index.clone(),
            DualFwd::new(value.value),
        )
    }
}

impl From<OptionEmbeddedCashflow<DualFwd>> for OptionEmbeddedCashflow<f64> {
    fn from(value: OptionEmbeddedCashflow<DualFwd>) -> Self {
        Self::new(
            value.payoff_ops.clone(),
            value.market_index.clone(),
            value.value.value(),
        )
    }
}
