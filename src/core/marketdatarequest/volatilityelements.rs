use std::collections::HashMap;

use crate::{
    ad::adreal::ADReal,
    indices::marketindex::MarketIndex,
    math::interpolation::bilinear::{BilinearInterpolator, BilinearPoint},
    time::date::Date,
};

/// # `VolatilityAxis`
/// Smile axis used in volatility surfaces/cubes.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum VolatilityAxis {
    /// Strike axis point.
    Strike(u64),
    /// Delta axis point.
    Delta(u64),
    /// Log-moneyness axis point.
    LogMoneyness(u64),
}

impl VolatilityAxis {
    /// Creates a strike axis value.
    #[must_use]
    pub const fn strike(value: f64) -> Self {
        Self::Strike(value.to_bits())
    }

    /// Creates a delta axis value.
    #[must_use]
    pub const fn delta(value: f64) -> Self {
        Self::Delta(value.to_bits())
    }

    /// Creates a log-moneyness axis value.
    #[must_use]
    pub const fn log_moneyness(value: f64) -> Self {
        Self::LogMoneyness(value.to_bits())
    }

    #[must_use]
    const fn axis_type(&self) -> u8 {
        match self {
            Self::Strike(_) => 0,
            Self::Delta(_) => 1,
            Self::LogMoneyness(_) => 2,
        }
    }

    #[must_use]
    const fn bits(&self) -> u64 {
        match self {
            Self::Strike(bits) | Self::Delta(bits) | Self::LogMoneyness(bits) => *bits,
        }
    }

    /// Returns the numeric axis value.
    #[must_use]
    pub fn value(&self) -> f64 {
        f64::from_bits(self.bits())
    }
}

/// # `VolatilityNodeKey`
/// Surface node key made of market index, expiry date, and smile axis.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VolatilityNodeKey {
    market_index: MarketIndex,
    date: Date,
    axis: VolatilityAxis,
}

impl VolatilityNodeKey {
    /// Creates a new surface node key.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date, axis: VolatilityAxis) -> Self {
        Self {
            market_index,
            date,
            axis,
        }
    }

    /// Returns the market index.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns the expiry date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }

    /// Returns the smile axis.
    #[must_use]
    pub const fn axis(&self) -> VolatilityAxis {
        self.axis
    }
}

/// # `VolatilityCubeNodeKey`
/// Cube node key extends surface key with a tenor date.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VolatilityCubeNodeKey {
    market_index: MarketIndex,
    date: Date,
    tenor_date: Date,
    axis: VolatilityAxis,
}

impl VolatilityCubeNodeKey {
    /// Creates a new cube node key.
    #[must_use]
    pub const fn new(
        market_index: MarketIndex,
        date: Date,
        tenor_date: Date,
        axis: VolatilityAxis,
    ) -> Self {
        Self {
            market_index,
            date,
            tenor_date,
            axis,
        }
    }

    /// Returns expiry date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }

    /// Returns tenor date.
    #[must_use]
    pub const fn tenor_date(&self) -> Date {
        self.tenor_date
    }

    /// Returns smile axis.
    #[must_use]
    pub const fn axis(&self) -> VolatilityAxis {
        self.axis
    }
}

/// # `VolatilityNode`
/// Resolved volatility node plus interpolation provenance.
#[derive(Clone)]
pub struct VolatilityNode {
    value: ADReal,
    interpolation_keys: Vec<VolatilityNodeKey>,
    colliding_keys: Vec<VolatilityNodeKey>,
    interpolation_labels: Vec<String>,
    colliding_labels: Vec<String>,
}

impl VolatilityNode {
    /// Creates a resolved volatility node.
    #[must_use]
    pub fn new(
        value: ADReal,
        interpolation_keys: Vec<VolatilityNodeKey>,
        colliding_keys: Vec<VolatilityNodeKey>,
        interpolation_labels: Vec<String>,
        colliding_labels: Vec<String>,
    ) -> Self {
        Self {
            value,
            interpolation_keys,
            colliding_keys,
            interpolation_labels,
            colliding_labels,
        }
    }

    /// Returns resolved volatility value.
    #[must_use]
    pub const fn value(&self) -> ADReal {
        self.value
    }

    /// Returns mutable resolved volatility value.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut ADReal {
        &mut self.value
    }

    /// Returns keys used for interpolation.
    #[must_use]
    pub fn interpolation_keys(&self) -> &[VolatilityNodeKey] {
        &self.interpolation_keys
    }

    /// Returns keys that collide on identical coordinates.
    #[must_use]
    pub fn colliding_keys(&self) -> &[VolatilityNodeKey] {
        &self.colliding_keys
    }

    /// Returns quote identifiers used by interpolation keys.
    #[must_use]
    pub fn interpolation_labels(&self) -> &[String] {
        &self.interpolation_labels
    }

    /// Returns quote identifiers for colliding keys.
    #[must_use]
    pub fn colliding_labels(&self) -> &[String] {
        &self.colliding_labels
    }
}

/// # `VolatilitySurfaceElement`
/// Volatility surface container.
#[derive(Clone, Default)]
pub struct VolatilitySurfaceElement {
    market_index: MarketIndex,
    nodes: HashMap<VolatilityNodeKey, ADReal>,
    labels: HashMap<VolatilityNodeKey, String>,
}

impl VolatilitySurfaceElement {
    /// Creates a volatility surface element.
    #[must_use]
    pub fn new(market_index: MarketIndex, nodes: HashMap<VolatilityNodeKey, ADReal>) -> Self {
        Self {
            market_index,
            nodes,
            labels: HashMap::new(),
        }
    }

    /// Creates a volatility surface element with quote identifiers for each node.
    #[must_use]
    pub fn with_labels(
        market_index: MarketIndex,
        nodes: HashMap<VolatilityNodeKey, ADReal>,
        labels: HashMap<VolatilityNodeKey, String>,
    ) -> Self {
        Self {
            market_index,
            nodes,
            labels,
        }
    }

    /// Returns exact node or bilinear interpolation in `(date, axis)`.
    pub fn node(&self, date: Date, axis: VolatilityAxis) -> Option<VolatilityNode> {
        let exact_key = VolatilityNodeKey::new(self.market_index.clone(), date, axis);
        if let Some(value) = self.nodes.get(&exact_key) {
            let interpolation_labels = self
                .labels
                .get(&exact_key)
                .cloned()
                .map_or_else(Vec::new, |label| vec![label]);
            return Some(VolatilityNode::new(
                *value,
                vec![exact_key],
                Vec::new(),
                interpolation_labels,
                Vec::new(),
            ));
        }

        let points = self
            .nodes
            .iter()
            .filter(|(key, _)| key.axis.axis_type() == axis.axis_type())
            .map(|(key, value)| BilinearPoint {
                x: (key.date - Date::empty()) as f64,
                y: key.axis.value(),
                value: *value,
                key: key.clone(),
            })
            .collect::<Vec<_>>();

        let out =
            BilinearInterpolator::interpolate((date - Date::empty()) as f64, axis.value(), points)?;
        let interpolation_labels = out
            .interpolation_keys()
            .iter()
            .filter_map(|key| self.labels.get(key).cloned())
            .collect::<Vec<_>>();
        let colliding_labels = out
            .colliding_keys()
            .iter()
            .filter_map(|key| self.labels.get(key).cloned())
            .collect::<Vec<_>>();
        Some(VolatilityNode::new(
            out.value(),
            out.interpolation_keys().to_vec(),
            out.colliding_keys().to_vec(),
            interpolation_labels,
            colliding_labels,
        ))
    }

    /// Returns market index.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns all nodes.
    #[must_use]
    pub const fn nodes(&self) -> &HashMap<VolatilityNodeKey, ADReal> {
        &self.nodes
    }

    /// Returns mutable nodes.
    #[must_use]
    pub fn nodes_mut(&mut self) -> &mut HashMap<VolatilityNodeKey, ADReal> {
        &mut self.nodes
    }

    /// Returns quote identifiers for surface nodes.
    #[must_use]
    pub const fn labels(&self) -> &HashMap<VolatilityNodeKey, String> {
        &self.labels
    }

    /// Returns mutable quote identifiers for surface nodes.
    #[must_use]
    pub fn labels_mut(&mut self) -> &mut HashMap<VolatilityNodeKey, String> {
        &mut self.labels
    }
}

/// # `VolatilityCubeElement`
/// Volatility cube container.
#[derive(Clone, Default)]
pub struct VolatilityCubeElement {
    market_index: MarketIndex,
    nodes: HashMap<VolatilityCubeNodeKey, ADReal>,
    labels: HashMap<VolatilityCubeNodeKey, String>,
}

impl VolatilityCubeElement {
    /// Creates a volatility cube element.
    #[must_use]
    pub fn new(market_index: MarketIndex, nodes: HashMap<VolatilityCubeNodeKey, ADReal>) -> Self {
        Self {
            market_index,
            nodes,
            labels: HashMap::new(),
        }
    }

    /// Creates a volatility cube element with quote identifiers for each cube node.
    #[must_use]
    pub fn with_labels(
        market_index: MarketIndex,
        nodes: HashMap<VolatilityCubeNodeKey, ADReal>,
        labels: HashMap<VolatilityCubeNodeKey, String>,
    ) -> Self {
        Self {
            market_index,
            nodes,
            labels,
        }
    }

    /// Returns market index.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns all cube nodes.
    #[must_use]
    pub const fn nodes(&self) -> &HashMap<VolatilityCubeNodeKey, ADReal> {
        &self.nodes
    }

    /// Returns mutable cube nodes.
    #[must_use]
    pub fn nodes_mut(&mut self) -> &mut HashMap<VolatilityCubeNodeKey, ADReal> {
        &mut self.nodes
    }

    /// Returns quote identifiers for cube nodes.
    #[must_use]
    pub const fn labels(&self) -> &HashMap<VolatilityCubeNodeKey, String> {
        &self.labels
    }

    /// Returns mutable quote identifiers for cube nodes.
    #[must_use]
    pub fn labels_mut(&mut self) -> &mut HashMap<VolatilityCubeNodeKey, String> {
        &mut self.labels
    }

    /// Returns exact node or bilinear interpolation in `(date, axis)` for fixed tenor.
    pub fn node(
        &self,
        date: Date,
        tenor_date: Date,
        axis: VolatilityAxis,
    ) -> Option<VolatilityNode> {
        let exact_key =
            VolatilityCubeNodeKey::new(self.market_index.clone(), date, tenor_date, axis);
        if let Some(value) = self.nodes.get(&exact_key) {
            let key = VolatilityNodeKey::new(self.market_index.clone(), date, axis);
            let interpolation_labels = self
                .labels
                .get(&exact_key)
                .cloned()
                .map_or_else(Vec::new, |label| vec![label]);
            return Some(VolatilityNode::new(
                *value,
                vec![key],
                Vec::new(),
                interpolation_labels,
                Vec::new(),
            ));
        }

        let points = self
            .nodes
            .iter()
            .filter(|(key, _)| {
                key.tenor_date == tenor_date && key.axis.axis_type() == axis.axis_type()
            })
            .map(|(key, value)| BilinearPoint {
                x: (key.date - Date::empty()) as f64,
                y: key.axis.value(),
                value: *value,
                key: key.clone(),
            })
            .collect::<Vec<_>>();

        let out =
            BilinearInterpolator::interpolate((date - Date::empty()) as f64, axis.value(), points)?;

        let interpolation_keys = out
            .interpolation_keys()
            .iter()
            .map(|key| VolatilityNodeKey::new(self.market_index.clone(), key.date, key.axis))
            .collect::<Vec<_>>();
        let colliding_keys = out
            .colliding_keys()
            .iter()
            .map(|key| VolatilityNodeKey::new(self.market_index.clone(), key.date, key.axis))
            .collect::<Vec<_>>();
        let interpolation_labels = out
            .interpolation_keys()
            .iter()
            .filter_map(|key| self.labels.get(key).cloned())
            .collect::<Vec<_>>();
        let colliding_labels = out
            .colliding_keys()
            .iter()
            .filter_map(|key| self.labels.get(key).cloned())
            .collect::<Vec<_>>();

        Some(VolatilityNode::new(
            out.value(),
            interpolation_keys,
            colliding_keys,
            interpolation_labels,
            colliding_labels,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        ad::adreal::{ADReal, IsReal},
        core::marketdatarequest::volatilityelements::{
            VolatilityAxis, VolatilityCubeElement, VolatilityCubeNodeKey, VolatilityNodeKey,
            VolatilitySurfaceElement,
        },
        indices::marketindex::MarketIndex,
        time::date::Date,
    };

    #[test]
    fn surface_interpolation_reports_used_keys() {
        let index = MarketIndex::Equity("SPX".to_string());
        let d0 = Date::new(2025, 6, 1);
        let d1 = Date::new(2025, 8, 1);
        let mut nodes = HashMap::new();
        let mut labels = HashMap::new();
        nodes.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::strike(90.0)),
            ADReal::from(0.24),
        );
        labels.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::strike(90.0)),
            "VOL_1".to_string(),
        );
        nodes.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::strike(110.0)),
            ADReal::from(0.20),
        );
        labels.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::strike(110.0)),
            "VOL_2".to_string(),
        );
        nodes.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::strike(90.0)),
            ADReal::from(0.22),
        );
        labels.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::strike(90.0)),
            "VOL_3".to_string(),
        );
        nodes.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::strike(110.0)),
            ADReal::from(0.18),
        );
        labels.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::strike(110.0)),
            "VOL_4".to_string(),
        );
        let surface = VolatilitySurfaceElement::with_labels(index, nodes, labels);

        let node = surface
            .node(Date::new(2025, 7, 1), VolatilityAxis::strike(100.0))
            .expect("surface interpolation");
        assert!(node.value().value() > 0.19 && node.value().value() < 0.23);
        assert_eq!(node.interpolation_keys().len(), 4);
        assert_eq!(node.interpolation_labels().len(), 4);
        assert!(node.colliding_keys().is_empty());
    }

    #[test]
    fn cube_uses_extra_tenor_key_and_interpolates() {
        let index = MarketIndex::Equity("SPX".to_string());
        let tenor = Date::new(2026, 1, 1);
        let d0 = Date::new(2025, 6, 1);
        let d1 = Date::new(2025, 8, 1);
        let mut nodes = HashMap::new();
        let mut labels = HashMap::new();
        nodes.insert(
            VolatilityCubeNodeKey::new(index.clone(), d0, tenor, VolatilityAxis::strike(90.0)),
            ADReal::from(0.24),
        );
        labels.insert(
            VolatilityCubeNodeKey::new(index.clone(), d0, tenor, VolatilityAxis::strike(90.0)),
            "CUBE_1".to_string(),
        );
        nodes.insert(
            VolatilityCubeNodeKey::new(index.clone(), d0, tenor, VolatilityAxis::strike(110.0)),
            ADReal::from(0.20),
        );
        labels.insert(
            VolatilityCubeNodeKey::new(index.clone(), d0, tenor, VolatilityAxis::strike(110.0)),
            "CUBE_2".to_string(),
        );
        nodes.insert(
            VolatilityCubeNodeKey::new(index.clone(), d1, tenor, VolatilityAxis::strike(90.0)),
            ADReal::from(0.22),
        );
        labels.insert(
            VolatilityCubeNodeKey::new(index.clone(), d1, tenor, VolatilityAxis::strike(90.0)),
            "CUBE_3".to_string(),
        );
        nodes.insert(
            VolatilityCubeNodeKey::new(index.clone(), d1, tenor, VolatilityAxis::strike(110.0)),
            ADReal::from(0.18),
        );
        labels.insert(
            VolatilityCubeNodeKey::new(index.clone(), d1, tenor, VolatilityAxis::strike(110.0)),
            "CUBE_4".to_string(),
        );
        let cube = VolatilityCubeElement::with_labels(index, nodes, labels);

        let node = cube
            .node(Date::new(2025, 7, 1), tenor, VolatilityAxis::strike(100.0))
            .expect("cube interpolation");
        assert!(node.value().value() > 0.19 && node.value().value() < 0.23);
        assert_eq!(node.interpolation_keys().len(), 4);
        assert_eq!(node.interpolation_labels().len(), 4);
    }

    #[test]
    fn vol_surface_supports_delta_and_moneyness_axes() {
        let index = MarketIndex::Equity("SPX".to_string());
        let d0 = Date::new(2025, 6, 1);
        let d1 = Date::new(2025, 8, 1);

        let mut delta_nodes = HashMap::new();
        delta_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::delta(0.25)),
            ADReal::from(0.26),
        );
        delta_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::delta(0.50)),
            ADReal::from(0.22),
        );
        delta_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::delta(0.25)),
            ADReal::from(0.24),
        );
        delta_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::delta(0.50)),
            ADReal::from(0.20),
        );
        let delta_surface = VolatilitySurfaceElement::new(index.clone(), delta_nodes);
        let delta_node = delta_surface
            .node(Date::new(2025, 7, 1), VolatilityAxis::delta(0.40))
            .expect("delta interpolation");
        assert!(delta_node.value().value() > 0.21 && delta_node.value().value() < 0.24);

        let mut moneyness_nodes = HashMap::new();
        moneyness_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::log_moneyness(-0.1)),
            ADReal::from(0.24),
        );
        moneyness_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d0, VolatilityAxis::log_moneyness(0.1)),
            ADReal::from(0.20),
        );
        moneyness_nodes.insert(
            VolatilityNodeKey::new(index.clone(), d1, VolatilityAxis::log_moneyness(-0.1)),
            ADReal::from(0.22),
        );
        moneyness_nodes.insert(
            VolatilityNodeKey::new(index, d1, VolatilityAxis::log_moneyness(0.1)),
            ADReal::from(0.18),
        );
        let moneyness_surface =
            VolatilitySurfaceElement::new(MarketIndex::Equity("SPX".to_string()), moneyness_nodes);
        let moneyness_node = moneyness_surface
            .node(Date::new(2025, 7, 1), VolatilityAxis::log_moneyness(0.0))
            .expect("moneyness interpolation");
        assert!(moneyness_node.value().value() > 0.19 && moneyness_node.value().value() < 0.23);
    }
}
