use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    indices::marketindex::MarketIndex,
    quotes::calibrationinstrument::CalibrationInstrument,
    rates::bootstrapping::{
        bootstrapdiscountpolicy::BootstrapDiscountPolicy, curveconfiguration::CurveConfiguration,
    },
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// Computes the pillar time grid from instruments.
#[must_use]
pub fn get_pillar_times(
    reference_date: Date,
    day_counter: DayCounter,
    instruments: &[CalibrationInstrument],
) -> Vec<f64> {
    let mut times = vec![0.0_f64];
    for instr in instruments {
        times.push(day_counter.year_fraction(reference_date, instr.pillar_date()));
    }
    times
}

/// Computes a topological ordering of curves respecting dependencies.
///
/// # Errors
/// Returns an error if a circular dependency is detected among the curve specifications, which would prevent successful
/// bootstrapping. The error message will indicate the presence of a circular dependency to aid in debugging curve configuration issues.
pub fn dependency_order<S: ::std::hash::BuildHasher>(
    curve_configs: &HashMap<MarketIndex, CurveConfiguration, S>,
    policy: &BootstrapDiscountPolicy,
) -> Result<Vec<MarketIndex>> {
    let mut dep_map: HashMap<MarketIndex, HashSet<MarketIndex>> = HashMap::new();

    for (idx, spec) in curve_configs {
        let mut deps = spec.dependencies(policy)?;
        deps.remove(idx);

        // Only keep dependencies that are actually configured for bootstrapping.
        deps.retain(|dep| curve_configs.contains_key(dep));

        dep_map.insert(idx.clone(), deps);
    }

    let mut indegree: HashMap<MarketIndex, usize> = curve_configs
        .keys()
        .map(|k| (k.clone(), dep_map.get(k).map_or(0, HashSet::len)))
        .collect();

    let mut reverse: HashMap<MarketIndex, Vec<MarketIndex>> = HashMap::new();
    for (idx, deps) in &dep_map {
        for dep in deps {
            reverse.entry(dep.clone()).or_default().push(idx.clone());
        }
    }

    let mut queue: VecDeque<MarketIndex> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(idx, _)| idx.clone())
        .collect();

    let mut order = Vec::new();
    while let Some(node) = queue.pop_front() {
        order.push(node.clone());
        if let Some(children) = reverse.get(&node) {
            for child in children {
                if let Some(value) = indegree.get_mut(child) {
                    *value = value.saturating_sub(1);
                    if *value == 0 {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
    }

    if order.len() < curve_configs.len() {
        return Err(QSError::InvalidValueErr(
            "Circular dependency detected among curve specifications".into(),
        ));
    }

    Ok(order)
}

/// Cross-curve dependency: how a child curve's DFs depend on a parent curve.
#[derive(Clone)]
pub struct CrossCurveDep {
    /// Child curve's sensitivity matrix.
    pub cross_df_sens: Vec<Vec<f64>>,
    /// Parent curve's IFT sensitivity matrix.
    pub parent_ift_sens: Vec<Vec<f64>>,
    /// Parent curve's quote values (for AD linkage).
    pub parent_quote_values: Vec<f64>,
    /// Parent curve's pillar labels.
    pub parent_pillar_labels: Vec<String>,
}
