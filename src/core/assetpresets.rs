use crate::{
    indices::marketindex::MarketIndex, math::interpolation::interpolator::Interpolator,
    time::period::Period,
};

/// InterestRateCurvePresets
pub struct InterestRateCurvePreset {
    market_index: MarketIndex,
    ois_pillars: Vec<Period>,
    swap_pillars: Vec<Period>,
    interpolation: Interpolator,
}

/// AssetPresets
#[derive(Default)]
pub struct AssetPresets {
    interest_rate_curves: Vec<InterestRateCurvePreset>,
}
