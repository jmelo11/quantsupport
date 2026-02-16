use std::{collections::HashMap, sync::Arc};

use crate::{
    ad::adreal::ADReal,
    core::pillars::Pillars,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

pub enum DerivedElementRequest {
    DiscountCurve {
        market_index: MarketIndex,
    },
    DividendCurve {
        market_index: MarketIndex,
    },
    VolatilitySurface {
        market_index: MarketIndex,
    },
    VolatilityCube {
        market_index: MarketIndex,
    },
    VolNode {
        market_index: MarketIndex,
        date: Date,
        axis: f64,
    },
    Simulation {
        market_index: MarketIndex,
    },
}

pub trait ADCurveElement:
    InterestRatesTermStructure<ADReal> + Pillars<ADReal> + Send + Sync
{
}

pub struct DiscountCurveElement {
    market_index: MarketIndex,
    currency: Currency,
    curve: Box<dyn ADCurveElement>,
}

impl DiscountCurveElement {
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        currency: Currency,
        curve: Box<dyn ADCurveElement>,
    ) -> Self {
        Self {
            market_index,
            currency,
            curve,
        }
    }

    #[must_use]
    pub fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    #[must_use]
    pub fn currency(&self) -> &Currency {
        &self.currency
    }

    #[must_use]
    pub fn curve(&self) -> &dyn ADCurveElement {
        self.curve.as_ref()
    }

    #[must_use]
    pub fn curve_mut(&mut self) -> &mut dyn ADCurveElement {
        self.curve.as_mut()
    }
}

#[derive(Clone)]
pub struct DividendCurveElement {
    pub market_index: MarketIndex,
    pub currency: Currency,
    pub curve: Arc<dyn InterestRatesTermStructure<ADReal> + Send + Sync>,
}

#[derive(Clone)]
pub struct SimulationElement {
    pub market_index: MarketIndex,
    pub draws: Vec<f64>,
}

pub struct FixingRequest {
    market_index: MarketIndex,
    date: Date,
}

impl FixingRequest {
    #[must_use]
    pub fn new(market_index: MarketIndex, date: Date) -> Self {
        Self { market_index, date }
    }

    #[must_use]
    pub fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    #[must_use]
    pub fn date(&self) -> Date {
        self.date
    }
}

pub struct MarketDataRequest {
    element_requests: Vec<DerivedElementRequest>,
    fixing_requests: Vec<FixingRequest>,
}

impl MarketDataRequest {
    pub fn with_element_requests(mut self, element_requests: Vec<DerivedElementRequest>) -> Self {
        self.element_requests = element_requests;
        self
    }

    pub fn with_fixing_requests(mut self, fixing_requests: Vec<FixingRequest>) -> Self {
        self.fixing_requests = fixing_requests;
        self
    }

    #[must_use]
    pub fn element_requests(&self) -> &[DerivedElementRequest] {
        &self.element_requests
    }

    #[must_use]
    pub fn fixing_requests(&self) -> &[FixingRequest] {
        &self.fixing_requests
    }
}

impl Default for MarketDataRequest {
    fn default() -> Self {
        Self {
            element_requests: Vec::new(),
            fixing_requests: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VolNodeKey {
    market_index: MarketIndex,
    date: Date,
    axis_bits: u64,
}

impl VolNodeKey {
    #[must_use]
    pub fn new(market_index: MarketIndex, date: Date, axis: f64) -> Self {
        Self {
            market_index,
            date,
            axis_bits: axis.to_bits(),
        }
    }
}

#[derive(Default)]
pub struct MarketDataResponse {
    pub discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
    pub dividend_curves: HashMap<MarketIndex, DividendCurveElement>,
    pub fixings: HashMap<(MarketIndex, Date), ADReal>,
    pub vol_nodes: HashMap<VolNodeKey, ADReal>,
    pub simulations: HashMap<MarketIndex, SimulationElement>,
}

pub trait MarketDataProvider {
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketDataResponse>;
    fn evaluation_date(&self) -> Date;
}
