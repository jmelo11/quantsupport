use crate::{
    ad::adreal::{ADReal, IsReal},
    core::pillars::Pillars,
    indices::marketindex::MarketIndex,
    time::{date::Date, period::Period},
    volatility::volatilityindexing::F64Key,
};
use std::collections::BTreeMap;

type SurfaceMap<T: IsReal> = BTreeMap<Period, BTreeMap<F64Key, T>>;

/// # `InterpolatedVolatilitySurface`
///
/// Represents if the volatility surface.
///
/// ## Generics
/// - `T`: Numeric type for the volatility values (e.g., `f64`, `ADReal`).
pub struct InterpolatedVolatilitySurface<T: IsReal> {
    reference_date: Date,
    market_index: MarketIndex,
    points: SurfaceMap<T>,
    labels: Option<Vec<String>>,
}

impl<T: IsReal> InterpolatedVolatilitySurface<T> {
    /// Creates a new `VolatilitySurface`.
    #[must_use]
    pub const fn new(
        reference_date: Date,
        market_index: MarketIndex,
        points: SurfaceMap<T>,
    ) -> Self {
        Self {
            reference_date,
            market_index,
            points,
            labels: None,
        }
    }

    pub fn with_labels(mut self, labels: &[String]) -> Self {
        self.labels = Some(labels.to_owned());
        self
    }

    /// Returns the market index associated with the volatility surface.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }
}

impl Pillars<ADReal> for InterpolatedVolatilitySurface<ADReal> {
    fn pillars(&self) -> Option<Vec<(String, &ADReal)>> {
        self.labels.as_ref().map(|labels| {
            labels
                .iter()
                .zip(self.points.values().flat_map(|m| m.values()))
                .map(|(label, value)| (label.clone(), value))
                .collect()
        })
    }

    fn pillar_labels(&self) -> Option<Vec<String>> {
        self.labels.clone()
    }

    fn put_pillars_on_tape(&mut self) {
        for m in self.points.values_mut() {
            for value in m.values_mut() {
                value.put_on_tape();
            }
        }
    }
}
