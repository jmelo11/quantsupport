use crate::core::contextmanager::ContextManager;
use crate::core::evaluationresults::SensitivityMap;
use crate::utils::errors::Result;

/// # `Request`
pub enum Request {
    /// Price
    Value,
    /// Yield to maturity
    YieldToMaturity,
    /// Modified Duration
    ModifiedDuration,
    /// Sensitivities
    Sensitivities,
    /// Cashflows
    Cashflows,
}

/// # `HandleValue`
///
/// The `HandleValue` trait defines a method for handling price-related operations.
pub trait HandleValue<T, S> {
    /// Handles price-related operations and returns a floating-point result.
    fn handle_value(&self, trade: &T, ctx: &ContextManager, state: &mut S) -> Result<f64>;
}

/// # `HandleYield`
///
/// The `HandleYield` trait defines a method for handling yield-related operations.
pub trait HandleYieldToMaturity<T, S> {
    /// Handles yield-related operations and returns a floating-point result.
    fn handle_yield(&self, trade: &T, ctx: &ContextManager, state: &mut S) -> Result<f64>;
}

/// # `HandleModifiedDuration`
///
/// The `HandleModifiedDuration` trait defines a method for handling modified duration operations.
pub trait HandleModifiedDuration<T, S> {
    /// Handles modified duration operations and returns a floating-point result.
    fn handle_modified_duration(
        &self,
        trade: &T,
        ctx: &ContextManager,
        state: &mut S,
    ) -> Result<f64>;
}
/// # `HandleSensitivities`
///
/// The `HandleSensitivities` trait defines a method for handling sensitivities-related operations.
pub trait HandleSensitivities<T, S> {
    /// Handles sensitivities-related operations and returns a sensitivy map result.
    fn handle_sensitivities(
        &self,
        trade: &T,
        ctx: &ContextManager,
        state: &mut S,
    ) -> Result<SensitivityMap>;
}
