use std::collections::HashMap;

use crate::{
    ad::adreal::ADReal,
    core::marketdataprovider::{
        DerivedElementRequest, DiscountCurveElement, DividendCurveElement, MarketDataProvider,
        MarketDataRequest, MarketDataResponse, SimulationElement, VolNodeKey,
    },
    indices::marketindex::MarketIndex,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

/// In-memory market-data provider that resolves requests from pre-loaded elements.
pub struct InMemoryMarketDataProvider {
    evaluation_date: Date,
    discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
    dividend_curves: HashMap<MarketIndex, DividendCurveElement>,
    fixings: HashMap<(MarketIndex, Date), ADReal>,
    vol_nodes: HashMap<VolNodeKey, ADReal>,
    simulations: HashMap<MarketIndex, SimulationElement>,
}

impl InMemoryMarketDataProvider {
    #[must_use]
    pub fn new(evaluation_date: Date) -> Self {
        Self {
            evaluation_date,
            discount_curves: HashMap::new(),
            dividend_curves: HashMap::new(),
            fixings: HashMap::new(),
            vol_nodes: HashMap::new(),
            simulations: HashMap::new(),
        }
    }

    #[must_use]
    pub const fn with_evaluation_date(mut self, evaluation_date: Date) -> Self {
        self.evaluation_date = evaluation_date;
        self
    }

    #[must_use]
    pub fn with_discount_curve(mut self, element: DiscountCurveElement) -> Self {
        self.discount_curves
            .insert(element.market_index.clone(), element);
        self
    }

    #[must_use]
    pub fn with_dividend_curve(mut self, element: DividendCurveElement) -> Self {
        self.dividend_curves
            .insert(element.market_index.clone(), element);
        self
    }

    #[must_use]
    pub fn with_fixing(mut self, market_index: MarketIndex, date: Date, value: ADReal) -> Self {
        self.fixings.insert((market_index, date), value);
        self
    }

    #[must_use]
    pub fn with_vol_node(
        mut self,
        market_index: MarketIndex,
        date: Date,
        axis: f64,
        value: ADReal,
    ) -> Self {
        self.vol_nodes
            .insert(VolNodeKey::new(market_index, date, axis), value);
        self
    }

    #[must_use]
    pub fn with_simulation(mut self, simulation: SimulationElement) -> Self {
        self.simulations
            .insert(simulation.market_index.clone(), simulation);
        self
    }
}

impl MarketDataProvider for InMemoryMarketDataProvider {
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketDataResponse> {
        let mut response = MarketDataResponse::default();

        for element in request.element_requests() {
            match element {
                DerivedElementRequest::DiscountCurve { market_index } => {
                    let curve =
                        self.discount_curves
                            .get(market_index)
                            .ok_or(AtlasError::NotFoundErr(format!(
                                "Discount curve for {market_index} was requested but is missing"
                            )))?;
                    response.discount_curves.insert(
                        market_index.clone(),
                        DiscountCurveElement {
                            market_index: curve.market_index.clone(),
                            currency: curve.currency,
                            curve: curve.curve.clone(),
                        },
                    );
                }
                DerivedElementRequest::DividendCurve { market_index } => {
                    let curve =
                        self.dividend_curves
                            .get(market_index)
                            .ok_or(AtlasError::NotFoundErr(format!(
                                "Dividend curve for {market_index} was requested but is missing"
                            )))?;
                    response.dividend_curves.insert(
                        market_index.clone(),
                        DividendCurveElement {
                            market_index: curve.market_index.clone(),
                            currency: curve.currency,
                            curve: curve.curve.clone(),
                        },
                    );
                }
                DerivedElementRequest::VolNode {
                    market_index,
                    date,
                    axis,
                } => {
                    let key = VolNodeKey::new(market_index.clone(), *date, *axis);
                    let node = self.vol_nodes.get(&key).ok_or(AtlasError::NotFoundErr(format!(
                        "Vol node for {market_index} at {date} / axis {axis} was requested but is missing"
                    )))?;
                    response.vol_nodes.insert(key, *node);
                }
                DerivedElementRequest::Simulation { market_index } => {
                    let sim = self
                        .simulations
                        .get(market_index)
                        .ok_or(AtlasError::NotFoundErr(format!(
                            "Simulation for {market_index} was requested but is missing"
                        )))?;
                    response
                        .simulations
                        .insert(market_index.clone(), sim.clone());
                }
                // Surface/cube data are resolved upstream to VolNode in current pricer flow.
                DerivedElementRequest::VolatilitySurface { .. }
                | DerivedElementRequest::VolatilityCube { .. } => {}
            }
        }

        for fixing in request.fixing_requests() {
            let key = (fixing.market_index().clone(), fixing.date());
            let value = self
                .fixings
                .get(&key)
                .ok_or(AtlasError::NotFoundErr(format!(
                    "Fixing for {} at {} was requested but is missing",
                    fixing.market_index(),
                    fixing.date()
                )))?;
            response.fixings.insert(key, *value);
        }

        Ok(response)
    }

    fn evaluation_date(&self) -> Date {
        self.evaluation_date
    }
}

impl Default for InMemoryMarketDataProvider {
    fn default() -> Self {
        Self::new(Date::empty())
    }
}
