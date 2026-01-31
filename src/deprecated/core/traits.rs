use super::meta::MarketRequest;
use crate::{currencies::enums::Currency, utils::errors::Result};

/// A trait for objects that have a currency.
pub trait HasCurrency {
    /// Returns the currency of the object.
    ///
    /// # Errors
    ///
    /// Returns an error if the currency cannot be determined.
    fn currency(&self) -> Result<Currency>;
}

/// A trait for objects that have a discount curve ID.
pub trait HasDiscountCurveId {
    /// Returns the discount curve ID of the object.
    ///
    /// # Errors
    ///
    /// Returns an error if the discount curve ID cannot be determined.
    fn discount_curve_id(&self) -> Result<usize>;
}

/// A trait for objects that have a forecast curve ID.
pub trait HasForecastCurveId {
    /// Returns the forecast curve ID of the object.
    ///
    /// # Errors
    ///
    /// Returns an error if the forecast curve ID cannot be determined.
    fn forecast_curve_id(&self) -> Result<usize>;
}

/// A trait for objects that can be registered for market data.
pub trait Registrable: HasDiscountCurveId + HasForecastCurveId + HasCurrency {
    /// Returns the ID of the object.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID cannot be determined.
    fn id(&self) -> Result<usize>;
    /// Sets the ID of the object.
    fn set_id(&mut self, id: usize);
    /// Returns the market request for the object.
    ///
    /// # Errors
    ///
    /// Returns an error if the market request cannot be created.
    fn market_request(&self) -> Result<MarketRequest>;
}
