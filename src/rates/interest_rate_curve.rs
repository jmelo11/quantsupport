use std::sync::Arc;

use crate::{
    ad::adreal::ADReal,
    core::assets::{Asset, AssetType},
    indices::marketindex::MarketIndex,
    rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
};

/// Interest rate curve asset backed by a discount term structure.
#[derive(Clone)]
pub struct InterestRateCurveAsset {
    market_index: MarketIndex,
    curve: DiscountTermStructure<ADReal>,
    inputs: Vec<(String, ADReal)>,
}

impl InterestRateCurveAsset {
    /// Creates a new interest rate curve asset.
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        curve: DiscountTermStructure<ADReal>,
        inputs: Vec<(String, ADReal)>,
    ) -> Self {
        Self {
            market_index,
            curve,
            inputs,
        }
    }

    /// Returns the curve.
    #[must_use]
    pub const fn curve(&self) -> &DiscountTermStructure<ADReal> {
        &self.curve
    }

    /// Returns the market index.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }
}

impl Asset for InterestRateCurveAsset {
    fn asset_type(&self) -> AssetType {
        AssetType::InterestRateCurve(Arc::new(self.clone()))
    }

    fn asset_inputs(&self) -> Vec<(String, ADReal)> {
        self.inputs.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
