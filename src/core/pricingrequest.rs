use crate::core::evaluationresults::SensitivityMap;
use crate::core::pricingdata::PricingDataContext;
use crate::utils::errors::Result;

/// # `PricingRequest`
pub enum PricingRequest {
    /// Price
    Price,
    /// Yield to maturity
    YieldToMaturity,
    /// Modified Duration
    ModifiedDuration,
    /// Sensitivities
    Sensitivities,
    /// Cashflows
    Cashflows,
}

/// # `HandlePrice`
///
/// The `HandlePrice` trait defines a method for handling price-related operations.
pub trait HandlePrice<T, S> {
    /// Handles price-related operations and returns a floating-point result.
    fn handle_price(&self, trade: &T, ctx: &PricingDataContext, state: &mut S) -> Result<f64>;
}

/// # `HandleYield`
///
/// The `HandleYield` trait defines a method for handling yield-related operations.
pub trait HandleYieldToMaturity<T, S> {
    /// Handles yield-related operations and returns a floating-point result.
    fn handle_yield(&self, trade: &T, ctx: &PricingDataContext, state: &mut S) -> Result<f64>;
}

/// # `HandleModifiedDuration`
///
/// The `HandleModifiedDuration` trait defines a method for handling modified duration operations.
pub trait HandleModifiedDuration<T, S> {
    /// Handles modified duration operations and returns a floating-point result.
    fn handle_modified_duration(
        &self,
        trade: &T,
        ctx: &PricingDataContext,
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
        ctx: &PricingDataContext,
        state: &mut S,
    ) -> Result<SensitivityMap>;
}
