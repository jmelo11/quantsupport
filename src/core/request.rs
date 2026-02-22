use crate::{core::evaluationresults::SensitivityMap, utils::errors::Result};

/// # `Request`
///
/// Enumeration of different types of requests that can be made for instrument evaluation,
/// including value, yield to maturity, modified duration, sensitivities, and cashflows.
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

impl Request {
    /// Returns the rank of the request, which can be used for ordering or prioritization.
    #[must_use]
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Value => 0,
            Self::Sensitivities => 1,
            Self::YieldToMaturity => 2,
            Self::ModifiedDuration => 3,
            Self::Cashflows => 4,
        }
    }
}

/// # `HandleValue`
///
/// The `HandleValue` trait defines a method for handling price-related operations.
pub trait HandleValue<T, S> {
    /// Handles price-related operations and returns a floating-point result.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the evaluation fails.
    fn handle_value(&self, trade: &T, state: &mut S) -> Result<f64>;
}

/// # `HandleYield`
///
/// The `HandleYield` trait defines a method for handling yield-related operations.
pub trait HandleYieldToMaturity<T, S> {
    /// Handles yield-related operations and returns a floating-point result.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the evaluation fails.
    fn handle_yield(&self, trade: &T, state: &mut S) -> Result<f64>;
}

/// # `HandleModifiedDuration`
///
/// The `HandleModifiedDuration` trait defines a method for handling modified duration operations.
pub trait HandleModifiedDuration<T, S> {
    /// Handles modified duration operations and returns a floating-point result.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the evaluation fails.
    fn handle_modified_duration(&self, trade: &T, state: &mut S) -> Result<f64>;
}
/// # `HandleSensitivities`
///
/// The `HandleSensitivities` trait defines a method for handling sensitivities-related operations.
pub trait HandleSensitivities<T, S> {
    /// Handles sensitivities-related operations and returns a sensitivy map result.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the evaluation fails.
    fn handle_sensitivities(&self, trade: &T, state: &mut S) -> Result<SensitivityMap>;
}
