use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use crate::{
    ad::adreal::ADReal,
    core::marketdatarequest::{
        curveelement::{DiscountCurveElement, DividendCurveElement},
        derivedelementrequest::{
            DerivedElementRequest, MarketDataProvider, MarketDataRequest, MarketDataResponse,
            SharedElement,
        },
        simulationelement::SimulationElement,
        volatilityelements::{
            VolatilityAxis, VolatilityCubeElement, VolatilityNodeKey, VolatilitySurfaceElement,
        },
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// # `MarketDataElementOwner`
/// In-memory market-data provider that resolves requests from pre-loaded elements.
pub struct MarketDataElementOwner {
    evaluation_date: Date,
    discount_curves: HashMap<MarketIndex, SharedElement<DiscountCurveElement>>,
    dividend_curves: HashMap<MarketIndex, SharedElement<DividendCurveElement>>,
    fixings: HashMap<(MarketIndex, Date), f64>,
    volatility_surfaces: HashMap<MarketIndex, SharedElement<VolatilitySurfaceElement>>,
    volatility_cubes: HashMap<MarketIndex, SharedElement<VolatilityCubeElement>>,
    simulations: HashMap<MarketIndex, SharedElement<SimulationElement>>,
}

impl MarketDataElementOwner {
    /// # `new`
    /// Creates a new provider at an evaluation date.
    #[must_use]
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

    /// # `with_evaluation_date`
    /// Sets evaluation date.
    #[must_use]
    pub const fn with_evaluation_date(mut self, evaluation_date: Date) -> Self {
        self.evaluation_date = evaluation_date;
        self
    }

    /// # `with_discount_curve`
    /// Adds a discount curve.
    #[must_use]
    pub fn with_discount_curve(mut self, element: DiscountCurveElement) -> Self {
        self.discount_curves
            .insert(element.market_index().clone(), Rc::new(RefCell::new(element)));
        self
    }

    /// # `with_dividend_curve`
    /// Adds a dividend curve.
    #[must_use]
    pub fn with_dividend_curve(mut self, element: DividendCurveElement) -> Self {
        self.dividend_curves
            .insert(element.market_index().clone(), Rc::new(RefCell::new(element)));
        self
    }

    /// # `with_fixing`
    /// Adds a fixing observation.
    #[must_use]
    pub fn with_fixing(mut self, market_index: MarketIndex, date: Date, value: f64) -> Self {
        self.fixings.insert((market_index, date), value);
        self
    }

    /// # `with_vol_node`
    /// Adds a single volatility node to the owned surface map.
    #[must_use]
    pub fn with_vol_node(
        mut self,
        market_index: MarketIndex,
        date: Date,
        axis: VolatilityAxis,
        value: ADReal,
    ) -> Self {
        let key = VolatilityNodeKey::new(market_index.clone(), date, axis);
        self.volatility_surfaces
            .entry(market_index.clone())
            .or_insert_with(|| {
                Rc::new(RefCell::new(VolatilitySurfaceElement::new(
                    market_index.clone(),
                    HashMap::new(),
                )))
            })
            .borrow_mut()
            .nodes_mut()
            .insert(key, value);
        self
    }

    /// # `with_vol_node_with_label`
    /// Adds a single labeled volatility node to the owned surface map.
    #[must_use]
    pub fn with_vol_node_with_label(
        mut self,
        market_index: MarketIndex,
        date: Date,
        axis: VolatilityAxis,
        value: ADReal,
        quote_identifier: String,
    ) -> Self {
        let key = VolatilityNodeKey::new(market_index.clone(), date, axis);
        let surface = self
            .volatility_surfaces
            .entry(market_index.clone())
            .or_insert_with(|| {
                Rc::new(RefCell::new(VolatilitySurfaceElement::new(
                    market_index.clone(),
                    HashMap::new(),
                )))
            })
            .clone();

        {
            let mut surface_mut = surface.borrow_mut();
            surface_mut.nodes_mut().insert(key.clone(), value);
            surface_mut.labels_mut().insert(key, quote_identifier);
        }
        self
    }

    /// # `with_vol_surface`
    /// Adds a volatility surface.
    #[must_use]
    pub fn with_vol_surface(mut self, element: VolatilitySurfaceElement) -> Self {
        self.volatility_surfaces
            .insert(element.market_index().clone(), Rc::new(RefCell::new(element)));
        self
    }

    /// # `with_vol_cube`
    /// Adds a volatility cube.
    #[must_use]
    pub fn with_vol_cube(mut self, element: VolatilityCubeElement) -> Self {
        self.volatility_cubes
            .insert(element.market_index().clone(), Rc::new(RefCell::new(element)));
        self
    }

    /// # `with_simulation`
    /// Adds a simulation element.
    #[must_use]
    pub fn with_simulation(mut self, simulation: SimulationElement) -> Self {
        self.simulations
            .insert(simulation.market_index().clone(), Rc::new(RefCell::new(simulation)));
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
                    response
                        .discount_curves_mut()
                        .insert(market_index.clone(), Rc::clone(curve));
                }
                DerivedElementRequest::DividendCurve { market_index } => {
                    let curve = self.dividend_curves.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Dividend curve for {market_index} missing"))
                    })?;
                    response
                        .dividend_curves_mut()
                        .insert(market_index.clone(), Rc::clone(curve));
                }
                DerivedElementRequest::VolNode {
                    market_index,
                    date,
                    axis,
                } => {
                    let node = self
                        .volatility_surfaces
                        .get(market_index)
                        .and_then(|surface| surface.borrow().node(*date, *axis))
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
                    response
                        .simulations_mut()
                        .insert(market_index.clone(), Rc::clone(sim));
                }
                DerivedElementRequest::VolatilitySurface { market_index } => {
                    let surface = self.volatility_surfaces.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Volatility surface for {market_index} missing"))
                    })?;
                    response
                        .volatility_surfaces_mut()
                        .insert(market_index.clone(), Rc::clone(surface));
                }
                DerivedElementRequest::VolatilityCube { market_index } => {
                    let cube = self.volatility_cubes.get(market_index).ok_or_else(|| {
                        AtlasError::NotFoundErr(format!("Volatility cube for {market_index} missing"))
                    })?;
                    response
                        .volatility_cubes_mut()
                        .insert(market_index.clone(), Rc::clone(cube));
                }
            }
        }

        for fixing in request.fixing_requests() {
            let key = (fixing.market_index().clone(), fixing.date());
            let value = self.fixings.get(&key).ok_or_else(|| {
                AtlasError::NotFoundErr(format!(
                    "Fixing for {} at {} missing",
                    fixing.market_index(),
                    fixing.date()
                ))
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

#[cfg(test)]
mod tests {
    use crate::{
        core::marketdatarequest::{
            derivedelementrequest::{DerivedElementRequest, MarketDataProvider, MarketDataRequest},
            simulationelement::SimulationElement,
        },
        indices::marketindex::MarketIndex,
        time::date::Date,
    };

    use super::MarketDataElementOwner;

    #[test]
    fn simulation_growth_persists_back_to_owner() {
        let index = MarketIndex::Equity("SPX".to_string());
        let owner = MarketDataElementOwner::new(Date::new(2025, 1, 1)).with_simulation(
            SimulationElement::new(index.clone(), vec![1.0, 2.0]),
        );

        let request = MarketDataRequest::default().with_element_requests(vec![
            DerivedElementRequest::Simulation {
                market_index: index.clone(),
            },
        ]);

        let response = owner.handle_request(&request).expect("request must resolve");
        let simulation = response
            .simulations()
            .get(&index)
            .expect("simulation present");
        simulation.borrow_mut().draws_mut().extend([3.0, 4.0]);

        let follow_up = owner
            .handle_request(&request)
            .expect("follow-up request must resolve");
        let updated_len = follow_up
            .simulations()
            .get(&index)
            .expect("simulation present")
            .borrow()
            .draws()
            .len();

        assert_eq!(updated_len, 4);
    }
}
