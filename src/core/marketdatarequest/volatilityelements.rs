use std::collections::HashMap;

use crate::{ad::adreal::ADReal, indices::marketindex::MarketIndex, time::date::Date};

/// `VolatilityAxis`
///
/// Axis used to address volatility surfaces.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum VolatilityAxis {
    /// Strike axis.
    Strike(u64),
    /// Delta axis.
    Delta(u64),
    /// Log-moneyness axis.
    LogMoneyness(u64),
}

impl VolatilityAxis {
    /// Creates strike axis.
    #[must_use]
    pub const fn strike(value: f64) -> Self {
        Self::Strike(value.to_bits())
    }

    /// Creates delta axis.
    #[must_use]
    pub const fn delta(value: f64) -> Self {
        Self::Delta(value.to_bits())
    }

    /// Creates log-moneyness axis.
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

    #[must_use]
    fn value(&self) -> f64 {
        f64::from_bits(self.bits())
    }
}

/// `VolatilityNodeKey`
///
/// Struct representing a key for identifying a specific node on a volatility surface,
/// based on market index, date, and axis value.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VolatilityNodeKey {
    market_index: MarketIndex,
    date: Date,
    axis: VolatilityAxis,
}

impl VolatilityNodeKey {
    /// Creates a new [`VolatilityNodeKey`] with the specified market index, date, and axis value.
    #[must_use]
    pub const fn new(market_index: MarketIndex, date: Date, axis: VolatilityAxis) -> Self {
        Self {
            market_index,
            date,
            axis,
        }
    }
}


pub trait VolatilityNodeProvider {
    fn node(&self, date: Date, axis: VolatilityAxis) -> Option<VolatilityNode>;
}

/// `VolatilityNode`
///
/// Resolved volatility node with interpolation provenance.
#[derive(Clone)]
pub struct VolatilityNode {
    value: ADReal,
    interpolation_keys: Vec<VolatilityNodeKey>,
}

impl VolatilityNode {
    /// Creates a new resolved volatility node.
    #[must_use]
    pub fn new(value: ADReal, interpolation_keys: Vec<VolatilityNodeKey>) -> Self {
        Self {
            value,
            interpolation_keys,
        }
    }

    /// Returns the resolved volatility value.
    #[must_use]
    pub const fn value(&self) -> ADReal {
        self.value
    }

    /// Returns mutable access to the resolved volatility value.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut ADReal {
        &mut self.value
    }

    /// Returns the source keys used to produce this node.
    #[must_use]
    pub fn interpolation_keys(&self) -> &[VolatilityNodeKey] {
        &self.interpolation_keys
    }
}

/// Represents a volatility surface/cube container for a market index.
#[derive(Clone, Default)]
pub struct VolatilitySurfaceElement {
    market_index: MarketIndex,
    nodes: HashMap<VolatilityNodeKey, ADReal>,
}

impl VolatilitySurfaceElement {
    /// Creates a new volatility surface/cube element.
    #[must_use]
    pub fn new(market_index: MarketIndex, nodes: HashMap<VolatilityNodeKey, ADReal>) -> Self {
        Self {
            market_index,
            nodes,
        }
    }

    /// Returns an exact or interpolated node at date/axis.
    pub fn node(&self, date: Date, axis: VolatilityAxis) -> Option<VolatilityNode> {
        let exact_key = VolatilityNodeKey::new(self.market_index.clone(), date, axis);
        if let Some(value) = self.nodes.get(&exact_key) {
            return Some(VolatilityNode::new(*value, vec![exact_key]));
        }

        let mut points = self
            .nodes
            .iter()
            .filter_map(|(key, value)| {
                if key.date == date {
                    Some((key.axis, key.clone(), *value))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if points.len() < 2 {
            return None;
        }

        points.retain(|point| point.0.axis_type() == axis.axis_type());
        if points.len() < 2 {
            return None;
        }

        points.sort_by(|a, b| a.0.value().total_cmp(&b.0.value()));
        let upper = points.partition_point(|p| p.0.value() < axis.value());
        if upper == 0 || upper >= points.len() {
            return None;
        }

        let (x0, k0, v0) = points[upper - 1].clone();
        let (x1, k1, v1) = points[upper].clone();
        if (x1.value() - x0.value()).abs() < f64::EPSILON {
            return Some(VolatilityNode::new(v0, vec![k0]));
        }

        let w = (axis.value() - x0.value()) / (x1.value() - x0.value());
        Some(VolatilityNode::new(
            (v0 + (v1 - v0) * w).into(),
            vec![k0, k1],
        ))
    }

    /// Returns the market index for this surface/cube.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns all stored raw nodes.
    #[must_use]
    pub const fn nodes(&self) -> &HashMap<VolatilityNodeKey, ADReal> {
        &self.nodes
    }

    /// Returns mutable access to raw nodes.
    #[must_use]
    pub const fn nodes_mut(&mut self) -> &mut HashMap<VolatilityNodeKey, ADReal> {
        &mut self.nodes
    }
}

/// `VolatilityCubeElement`
///
/// Represents a volatility cube container for a market index.
#[derive(Clone, Default)]
pub struct VolatilityCubeElement {
    market_index: MarketIndex,
    nodes: HashMap<VolatilityNodeKey, ADReal>,
}

impl VolatilityCubeElement {
    /// Creates a new volatility cube element.
    #[must_use]
    pub fn new(market_index: MarketIndex, nodes: HashMap<VolatilityNodeKey, ADReal>) -> Self {
        Self {
            market_index,
            nodes,
        }
    }

    /// Returns the market index for this cube.
    #[must_use]
    pub const fn market_index(&self) -> &MarketIndex {
        &self.market_index
    }

    /// Returns mutable access to raw nodes.
    #[must_use]
    pub const fn nodes_mut(&mut self) -> &mut HashMap<VolatilityNodeKey, ADReal> {
        &mut self.nodes
    }

    /// Returns an exact or interpolated node at date/axis.
    pub fn node(&self, date: Date, axis: VolatilityAxis) -> Option<VolatilityNode> {
        VolatilitySurfaceElement::new(self.market_index.clone(), self.nodes.clone())
            .node(date, axis)
    }
}
