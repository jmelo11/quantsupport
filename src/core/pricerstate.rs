use crate::{
    ad::adreal::ADReal,
    core::{
        elements::{
            curveelement::{DiscountCurveElement, DividendCurveElement},
            montecarlosimulationelement::MonteCarloSimulationElement,
            volatilitycubelement::VolatilityCubeElement,
            volatilitysurfaceelement::VolatilitySurfaceElement,
        },
        marketdatahandling::marketdata::MarketData,
        pillars::Pillars,
    },
    currencies::{currency::Currency, exchangeratestore::ExchangeRateStore},
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// The [`PricerState`] trait defines the interface for accessing
/// market data responses, derived elements and finxing values during the
/// pricing process.
pub trait PricerState {
    /// Retrieves the market data response associated with this state, if available.
    fn get_market_data_reponse(&self) -> Option<&MarketData>;

    /// Retrieves a mutable reference to the market data response associated with this state, if available.
    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData>;

    /// Retrieves the discount curve element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the discount curve for the specified index is not found.
    fn get_discount_curve_element(&self, index: &MarketIndex) -> Result<&DiscountCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .discount_curves()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Curve for index {index}")))
    }

    /// Retrieves the mutable discount curve element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the discount curve for the specified index is not found.
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
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the dividend curve for the specified index is not found.
    fn get_dividend_curve_element(&self, index: &MarketIndex) -> Result<&DividendCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .dividend_curves()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Dividend curve for index {index}")))
    }

    /// Retrieves the exchange rate between two currencies from the exchange-rate store.
    ///
    /// Returns an [`ADReal`] so that sensitivities to FX rates are captured on the AD tape.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response or exchange-rate store is not available,
    /// or if no rate path exists between the two currencies.
    fn get_exchange_rate(&self, base: Currency, quote: Currency) -> Result<ADReal> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .exchange_rate_store()
            .ok_or_else(|| AtlasError::NotFoundErr("ExchangeRateStore not available.".into()))?
            .get_exchange_rate(base, quote)
    }

    /// Retrieves the exchange-rate store from the market data, if available.
    fn get_exchange_rate_store(&self) -> Option<&ExchangeRateStore> {
        self.get_market_data_reponse()
            .and_then(|md| md.exchange_rate_store())
    }

    /// Retrieves the fixing for a given market index and date, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the fixing for the specified index and date is not found.
    fn get_fixing(&self, index: &MarketIndex, date: Date) -> Result<f64> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .fixings()
            .get(index)
            .and_then(|date_map| date_map.get(&date).copied())
            .ok_or_else(|| {
                AtlasError::NotFoundErr(format!(
                    "Fixing for index {index} on date {date} not found."
                ))
            })
    }

    /// Retrieves the volatility surface element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the volatility surface for the specified index is not found.
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
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the volatility surface for the specified index is not found.
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
    ///
    /// ## Errors
    ///
    /// Returns an if the market data response is not available or if the volatility cube for the specified index is not found.
    fn get_volatility_cube_element(&self, index: &MarketIndex) -> Result<&VolatilityCubeElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .volatility_cubes()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Volatility cube for index {index}")))
    }

    /// Retrieves the simulation element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the simulation element for the specified index is not found.
    fn get_simulation_element(&self, index: &MarketIndex) -> Result<&MonteCarloSimulationElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| AtlasError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .simulations()
            .get(index)
            .ok_or_else(|| AtlasError::NotFoundErr(format!("Simulation element for index {index}")))
    }

    /// Puts the pillars into the tape.
    ///
    /// This includes curve/surface pillars **and** exchange-rate spot rates.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available.
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
            // Put FX spot rates on tape
            if let Some(fx_store) = md_response.exchange_rate_store_mut() {
                fx_store.put_pillars_on_tape();
            }
        }
        Ok(())
    }
}
