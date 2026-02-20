use std::collections::HashMap;

use crate::{
    ad::adreal::ADReal,
    core::marketdatarequest::{
        curveelement::{DiscountCurveElement, DividendCurveElement},
        derivedelementrequest::{DerivedElementRequest, MarketDataProvider, MarketDataRequest, MarketDataResponse},
        simulationelement::SimulationElement,
        volatilityelements::{VolatilityAxis, VolatilityCubeElement, VolatilityNodeKey, VolatilitySurfaceElement},
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// In-memory market-data provider that resolves requests from pre-loaded elements.
pub struct MarketDataElementOwner {
    evaluation_date: Date,
    discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
    dividend_curves: HashMap<MarketIndex, DividendCurveElement>,
    fixings: HashMap<(MarketIndex, Date), f64>,
    volatility_surfaces: HashMap<MarketIndex, VolatilitySurfaceElement>,
    volatility_cubes: HashMap<MarketIndex, VolatilityCubeElement>,
    simulations: HashMap<MarketIndex, SimulationElement>,
}


impl MarketDataElementOwner {
    #[must_use]
    /// Creates a new provider at an evaluation date.
    pub fn new(evaluation_date: Date) -> Self {
        Self {
            evaluation_date,
            discount_curves: HashMap::new(),
            dividend_curves: HashMap::new(),
            fixings: HashMap::new(),
            volatility_surfaces: HashMap::new(),
            volatility_cubes: HashMap::new(),
            simulations: HashMap::new(),
        }
    }

    #[must_use]
    /// Sets evaluation date.
    pub const fn with_evaluation_date(mut self, evaluation_date: Date) -> Self {
        self.evaluation_date = evaluation_date;
        self
    }

    #[must_use]
    /// Adds a discount curve.
    pub fn with_discount_curve(mut self, element: DiscountCurveElement) -> Self {
        self.discount_curves.insert(element.market_index().clone(), element);
        self
    }

    #[must_use]
    /// Adds a dividend curve.
    pub fn with_dividend_curve(mut self, element: DividendCurveElement) -> Self {
        self.dividend_curves.insert(element.market_index().clone(), element);
        self
    }

    #[must_use]
    /// Adds a fixing observation.
    pub fn with_fixing(mut self, market_index: MarketIndex, date: Date, value: f64) -> Self {
        self.fixings.insert((market_index, date), value);
        self
    }

    #[must_use]
    /// Adds a single volatility node to the owned surface map.
    pub fn with_vol_node(
        mut self,
        market_index: MarketIndex,
        date: Date,
        axis: VolatilityAxis,
        value: ADReal,
    ) -> Self {
        self.volatility_surfaces
            .entry(market_index.clone())
            .or_insert_with(|| VolatilitySurfaceElement::new(market_index.clone(), HashMap::new()))
            .nodes_mut()
            .insert(VolatilityNodeKey::new(market_index, date, axis), value);
        self
    }

    #[must_use]
    /// Adds a volatility surface.
    pub fn with_vol_surface(mut self, element: VolatilitySurfaceElement) -> Self {
        self.volatility_surfaces.insert(element.market_index().clone(), element);
        self
    }

    #[must_use]
    /// Adds a volatility cube.
    pub fn with_vol_cube(mut self, element: VolatilityCubeElement) -> Self {
        self.volatility_cubes.insert(element.market_index().clone(), element);
        self
    }

    #[must_use]
    /// Adds a simulation element.
    pub fn with_simulation(mut self, simulation: SimulationElement) -> Self {
        self.simulations.insert(simulation.market_index().clone(), simulation);
        self
    }
}

impl MarketDataProvider for MarketDataElementOwner {
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketDataResponse> {
        let mut response = MarketDataResponse::default();

        for element in request.element_requests() {
            match element {
                DerivedElementRequest::DiscountCurve { market_index } => {
                    let curve = self.discount_curves.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Discount curve for {market_index} missing"))
                    })?;
                    response.discount_curves_mut().insert(market_index.clone(), curve.clone());
                }
                DerivedElementRequest::DividendCurve { market_index } => {
                    let curve = self.dividend_curves.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Dividend curve for {market_index} missing"))
                    })?;
                    response.dividend_curves_mut().insert(market_index.clone(), curve.clone());
                }
                DerivedElementRequest::VolNode { market_index, date, axis } => {
                    let node = self
                        .volatility_surfaces
                        .get(market_index)
                        .and_then(|surface| surface.node(*date, *axis))
                        .ok_or_else(|| {
                            AtlasError::NotFoundErr(format!(
                                "Volatility node for {market_index} at {date} / axis {axis:?} missing"
                            ))
                        })?;
                    response
                        .volatility_nodes_mut()
                        .insert(VolatilityNodeKey::new(market_index.clone(), *date, *axis), node);
                }
                DerivedElementRequest::Simulation { market_index } => {
                    let sim = self.simulations.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Simulation for {market_index} missing"))
                    })?;
                    response.simulations_mut().insert(market_index.clone(), sim.clone());
                }
                DerivedElementRequest::VolatilitySurface { market_index } => {
                    let surface = self.volatility_surfaces.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Volatility surface for {market_index} missing"))
                    })?;
                    response.volatility_surfaces_mut().insert(market_index.clone(), surface.clone());
                }
                DerivedElementRequest::VolatilityCube { market_index } => {
                    let cube = self.volatility_cubes.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Volatility cube for {market_index} missing"))
                    })?;
                    response.volatility_cubes_mut().insert(market_index.clone(), cube.clone());
                }
            }
        }

        for fixing in request.fixing_requests() {
            let key = (fixing.market_index().clone(), fixing.date());
            let value = self.fixings.get(&key).ok_or_else(|| {
                AtlasError::NotFoundErr(format!("Fixing for {} at {} missing", fixing.market_index(), fixing.date()))
            })?;
            response.fixings_mut().insert(key, *value);
        }

        Ok(response)
    }

    fn evaluation_date(&self) -> Date {
        self.evaluation_date
    }
}

impl Default for MarketDataElementOwner {
    fn default() -> Self {
        Self::new(Date::empty())
    }
}
