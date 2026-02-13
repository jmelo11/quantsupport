use crate::{
    indices::marketindex::MarketIndex, math::interpolation::interpolator::Interpolator,
    time::period::Period,
};

/// InterestRateCurvePresets
#[derive(Clone)]
pub struct InterestRateCurvePreset {
    market_index: MarketIndex,
    deposit_tenors: Vec<Period>,
    swap_tenors: Vec<Period>,
    interpolation: Interpolator,
    enable_extrapolation: bool,
}

impl InterestRateCurvePreset {
    /// Creates a new interest rate curve preset.
    #[must_use]
    pub fn new(
        market_index: MarketIndex,
        deposit_tenors: Vec<Period>,
        swap_tenors: Vec<Period>,
        interpolation: Interpolator,
        enable_extrapolation: bool,
    ) -> Self {
        Self {
            market_index,
            deposit_tenors,
            swap_tenors,
            interpolation,
            enable_extrapolation,
        }
    }

    /// Returns the market index tied to the curve.
    #[must_use]
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Returns the interpolation type.
    #[must_use]
    pub const fn interpolation(&self) -> Interpolator {
        self.interpolation
    }

    /// Returns whether extrapolation is enabled.
    #[must_use]
    pub const fn enable_extrapolation(&self) -> bool {
        self.enable_extrapolation
    }
}

/// AssetPresets
#[derive(Default)]
pub struct AssetPresets {
    interest_rate_curves: Vec<InterestRateCurvePreset>,
}

impl AssetPresets {
    /// Returns the interest rate curve presets.
    #[must_use]
    pub const fn interest_rate_curves(&self) -> &Vec<InterestRateCurvePreset> {
        &self.interest_rate_curves
    }

    /// Adds an interest rate curve preset.
    pub fn add_interest_rate_curve(&mut self, preset: InterestRateCurvePreset) {
        self.interest_rate_curves.push(preset);
    }
}
