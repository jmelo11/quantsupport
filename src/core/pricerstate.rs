use crate::{
    core::{
        elements::{
            curveelement::{DiscountCurveElement, DividendCurveElement},
            volatilitycubelement::VolatilityCubeElement,
            volatilitysurfaceelement::VolatilitySurfaceElement,
        },
        marketdatahandling::marketdata::MarketData,
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// # `PricerState`
///
/// The `PricerState` trait defines the interface for accessing
/// market data responses, derived elements and finxing values during the
/// pricing process.
pub trait PricerState {
    /// Retrieves the market data response associated with this state, if available.
    fn get_market_data_reponse(&self) -> Option<&MarketData>;

    /// Retrieves a mutable reference to the market data response associated with this state, if available.
    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData>;

    /// Retrieves the discount curve element associated with the given market index, if available.
    fn get_discount_curve_element(&self, index: &MarketIndex) -> Result<&DiscountCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .discount_curves()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Curve for index {index}")))
    }

    /// Retrieves the mutable discount curve element associated with the given market index, if available.
    fn get_discount_curve_element_mut(
        &mut self,
        index: &MarketIndex,
    ) -> Result<&mut DiscountCurveElement> {
        self.get_market_data_reponse_mut()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements_mut()
            .discount_curves_mut()
            .get_mut(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Curve for index {index}")))
    }

    /// Retrieves the dividend curve element associated with the given market index, if available.
    fn get_dividend_curve_element(&self, index: &MarketIndex) -> Result<&DividendCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .dividend_curves()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Dividend curve for index {index}")))
    }
    /// Retrieves the fixing for a given market index and date, if available.
    fn get_fixing(&self, index: &MarketIndex, date: Date) -> Result<f64> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .fixings()
            .get(&index)
            .and_then(|date_map| date_map.get(&date).copied())
            .ok_or_else(|| {
                AtlasError::NotFoundErr(format!(
                    "Fixing for index {index} on date {date} not found."
                ))
            })
    }

    /// Retrieves the volatility surface element associated with the given market index, if available.
    fn get_volatility_surface_element(
        &self,
        index: &MarketIndex,
    ) -> Result<&VolatilitySurfaceElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .volatility_surfaces()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Volatility surface for index {index}")))
    }

    /// Retrieves the volatility surface element associated with the given market index, if available.
    fn get_volatility_surface_element_mut(
        &mut self,
        index: &MarketIndex,
    ) -> Result<&mut VolatilitySurfaceElement> {
        self.get_market_data_reponse_mut()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements_mut()
            .volatility_surfaces_mut()
            .get_mut(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Volatility surface for index {index}")))
    }

    /// Retrieves the volatility cube element associated with the given market index, if available.
    fn get_volatility_cube_element(&self, index: &MarketIndex) -> Result<&VolatilityCubeElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .volatility_cubes()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Volatility cube for index {index}")))
    }

    /// Puts the pillars into the tape.
    fn put_pillars_on_tape(&mut self) -> Result<()> {
        if let Some(md_response) = self.get_market_data_reponse_mut() {
            for curve in md_response
                .constructed_elements_mut()
                .discount_curves_mut()
                .values_mut()
            {
                curve.curve_mut().put_pillars_on_tape();
            }
            for curve in md_response
                .constructed_elements_mut()
                .dividend_curves_mut()
                .values_mut()
            {
                curve.curve_mut().put_pillars_on_tape();
            }
            for surface in md_response
                .constructed_elements_mut()
                .volatility_surfaces_mut()
                .values_mut()
            {
                surface.surface_mut().put_pillars_on_tape();
            }
        }
        Ok(())
    }
}
