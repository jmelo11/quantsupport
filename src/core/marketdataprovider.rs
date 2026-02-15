use std::{collections::HashMap, hash::Hash};

use crate::{
    ad::adreal::ADReal, currencies::currency::Currency, indices::marketindex::MarketIndex,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::date::Date, utils::errors::Result,
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

pub struct DiscountCurveElement {
    pub market_index: MarketIndex,
    pub currency: Currency,
    pub pillars: Vec<(String, ADReal)>,
    pub curve: Box<dyn InterestRatesTermStructure<ADReal>>,
}

pub struct FixingRequest {
    market_index: MarketIndex,
    date: Date,
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
}

impl Default for MarketDataRequest {
    fn default() -> Self {
        Self {
            element_requests: Vec::new(),
            fixing_requests: Vec::new(),
        }
    }
}

pub struct MarketDataResponse {
    pub discount_curves: HashMap<MarketIndex, DiscountCurveElement>,
}

pub trait MarketDataProvider {
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketDataResponse>;
    fn evaluation_date(&self) -> Date;
}
