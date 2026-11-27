use std::collections::HashMap;

use crate::{
    ad::Recorded,
    core::{
        elements::{
            curveelement::{CreditCurveElement, DiscountCurveElement, DividendCurveElement},
            montecarlosimulationelement::MonteCarloSimulationElement,
            volatilitycubelement::VolatilityCubeElement,
            volatilitysurfaceelement::VolatilitySurfaceElement,
        },
        marketdatahandling::marketdata::MarketData,
        pillars::Pillars,
    },
    currencies::currency::Currency,
    indices::{fxpair::FxPair, marketindex::MarketIndex},
    quotes::fxstore::FxStore,
    time::date::Date,
    utils::errors::{QSError, Result},
    volatility::orientedfxvolsurface::OrientedFxVolSurface,
};

/// Registers market inputs in a deterministic order for compatible native code reuse.
fn register_in_order<T>(elements: &mut HashMap<MarketIndex, T>, mut register: impl FnMut(&mut T)) {
    let mut ordered: Vec<_> = elements.iter_mut().collect();
    ordered.sort_by_cached_key(|(index, _)| index.to_string());
    for (_, element) in ordered {
        register(element);
    }
}

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
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .discount_curves()
            .get(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Curve for index {index}")))
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
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements_mut()
            .discount_curves_mut()
            .get_mut(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Curve for index {index}")))
    }

    /// Retrieves the dividend curve element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the dividend curve for the specified index is not found.
    fn get_dividend_curve_element(&self, index: &MarketIndex) -> Result<&DividendCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .dividend_curves()
            .get(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Dividend curve for index {index}")))
    }

    /// Retrieves the credit (survival) curve element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the credit curve for the specified index is not found.
    fn get_credit_curve_element(&self, index: &MarketIndex) -> Result<&CreditCurveElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .credit_curves()
            .get(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Credit curve for index {index}")))
    }

    /// Retrieves the exchange rate between two currencies from the exchange-rate store.
    ///
    /// Returns a [`Recorded`] value so that FX sensitivities are included in the compiler capture.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response or exchange-rate store is not available,
    /// or if no rate path exists between the two currencies.
    fn get_exchange_rate(&self, base: Currency, quote: Currency) -> Result<Recorded> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .fx_store()
            .ok_or_else(|| QSError::NotFoundErr("FxStore not available.".into()))?
            .get_fx_rate(base, quote)
    }

    /// Retrieves the exchange-rate store from the market data, if available.
    fn get_fx_store(&self) -> Option<&FxStore> {
        self.get_market_data_reponse().and_then(|md| md.fx_store())
    }

    /// Collects spot-pillar derivatives after the pricer differentiates its value.
    ///
    /// # Errors
    /// Propagates derivative errors for spot observations on the valuation capture.
    fn get_fx_sensitivities(&self) -> Result<Vec<(String, f64)>> {
        self.get_fx_store().map_or_else(
            || Ok(Vec::new()),
            |store| {
                store
                    .pillars()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(label, pillar)| Ok((label, pillar.derivative()?.value())))
                    .collect()
            },
        )
    }

    /// Retrieves the fixing for a given market index and date, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the fixing for the specified index and date is not found.
    fn get_fixing(&self, index: &MarketIndex, date: Date) -> Result<f64> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .fixings()
            .get(index)
            .and_then(|date_map| date_map.get(&date).copied())
            .ok_or_else(|| {
                QSError::NotFoundErr(format!(
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
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .volatility_surfaces()
            .get(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Volatility surface for index {index}")))
    }

    /// Retrieves an FX volatility surface for the given pair, returning an
    /// [`OrientedFxVolSurface`] that transparently handles parity inversion.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if no
    /// volatility surface is found for either orientation of the pair.
    fn get_fx_volatility_surface(&self, pair: &FxPair) -> Result<OrientedFxVolSurface<'_>> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .fx_volatility_surface(pair)
            .ok_or_else(|| QSError::NotFoundErr(format!("Volatility surface for FX pair {pair}")))
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
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements_mut()
            .volatility_surfaces_mut()
            .get_mut(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Volatility surface for index {index}")))
    }

    /// Retrieves the volatility cube element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an if the market data response is not available or if the volatility cube for the specified index is not found.
    fn get_volatility_cube_element(&self, index: &MarketIndex) -> Result<&VolatilityCubeElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .volatility_cubes()
            .get(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Volatility cube for index {index}")))
    }

    /// Retrieves the simulation element associated with the given market index, if available.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available or if the simulation element for the specified index is not found.
    fn get_simulation_element(&self, index: &MarketIndex) -> Result<&MonteCarloSimulationElement> {
        self.get_market_data_reponse()
            .ok_or_else(|| QSError::NotFoundErr("MarketDataResponse not available.".into()))?
            .constructed_elements()
            .simulations()
            .get(index)
            .ok_or_else(|| QSError::NotFoundErr(format!("Simulation element for index {index}")))
    }

    /// Registers market pillars as compiler inputs in a stable layout.
    ///
    /// This includes curve/surface pillars **and** exchange-rate spot rates.
    ///
    /// ## Errors
    ///
    /// Returns an error if the market data response is not available.
    fn register_pillars(&mut self) -> Result<()> {
        if let Some(md_response) = self.get_market_data_reponse_mut() {
            let elements = md_response.constructed_elements_mut();
            register_in_order(elements.discount_curves_mut(), |curve| {
                curve.curve_mut().register_pillars();
            });
            register_in_order(elements.dividend_curves_mut(), |curve| {
                curve.curve_mut().register_pillars();
            });
            register_in_order(elements.credit_curves_mut(), |curve| {
                curve.curve_mut().register_pillars();
            });
            register_in_order(elements.volatility_surfaces_mut(), |surface| {
                surface.surface_mut().register_pillars();
            });
            // Register FX spot inputs after the ordered market pillars.
            if let Some(fx_store) = md_response.fx_store_mut() {
                fx_store.register_pillars();
            }
        }
        Ok(())
    }
}
