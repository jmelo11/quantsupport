use crate::{
    core::{
        evaluationresults::SensitivityMap,
        marketdatarequest::{
            curveelement::DiscountCurveElement, derivedelementrequest::MarketDataResponse,
        },
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

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
    pub fn rank(&self) -> u8 {
        match self {
            Request::Value => 0,
            Request::Sensitivities => 1,
            Request::YieldToMaturity => 2,
            Request::ModifiedDuration => 3,
            Request::Cashflows => 4,
        }
    }
}

/// # PricerState
///
/// The `PricerState` trait defines the interface for accessing
/// market data responses, derived elements and finxing values during the
/// pricing process.
pub trait PricerState {
    /// Retrieves the market data response associated with this state, if available.
    fn get_market_data_reponse(&self) -> Option<&impl MarketDataResponse>;

    /// Retrieves a mutable reference to the market data response associated with this state, if available.
    fn get_market_data_reponse_mut(&mut self) -> Option<&mut impl MarketDataResponse>;

    /// Retrieves the discount curve element associated with the given market index, if available.
    fn get_discount_curve_element(&self, index: &MarketIndex) -> Result<&DiscountCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .discount_curves()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Curve for index {index}")))
    }

    /// Retrieves a mutable reference to the discount curve element associated with the given market index, if available.
    fn get_discount_curve_element_mut(
        &mut self,
        index: &MarketIndex,
    ) -> Result<&mut DiscountCurveElement> {
        self.get_market_data_reponse_mut()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not set.".into()))?
            .discount_curves_mut()
            .get_mut(&index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Curve for index {index}")))
    }

    /// Retrieves the fixing for a given market index and date, if available.
    fn get_fixing(&self, index: &MarketIndex, date: Date) -> Result<f64> {
        let key = (index.clone(), date);
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .fixings()
            .get(&key)
            .ok_or_else(|| {
                AtlasError::NotFoundErr(format!(
                    "Fixing for index {index} on date {date} not found."
                ))
            })
            .copied()
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
