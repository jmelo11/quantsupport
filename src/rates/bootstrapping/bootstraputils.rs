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

        // Validate that every required dependency has a CurveConfiguration.
        // A missing dependency means the user has not provided market data
        // (pillars) for that curve.
        for dep in &deps {
            if !curve_configs.contains_key(dep) {
                return Err(QSError::NotFoundErr(format!(
                    "Curve {idx} requires {dep} for discounting but no \
                     curve configuration was provided for it. Add market data \
                     (pillars) and configuration for {dep}."
                )));
            }
        }

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        quotes::{
            quote::{Level, Quote, QuoteDetails, QuoteLevels},
            quoteselector::QuoteSelector,
        },
        rates::bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy, bootstraputils::dependency_order,
            curveconfiguration::CurveConfiguration,
        },
        time::{date::Date, daycounter::DayCounter},
        utils::errors::Result,
    };

    struct MapSelector {
        reference_date: Date,
        quotes: HashMap<String, f64>,
    }

    impl MapSelector {
        fn new(reference_date: Date) -> Self {
            Self {
                reference_date,
                quotes: HashMap::new(),
            }
        }
        fn add(&mut self, id: &str, rate: f64) {
            self.quotes.insert(id.to_string(), rate);
        }
    }

    impl QuoteSelector for MapSelector {
        fn select(&self, identifier: &str) -> Option<Quote> {
            let rate = self.quotes.get(identifier)?;
            let det: QuoteDetails = identifier.parse().ok()?;
            let q = Quote::new(det, QuoteLevels::with_mid(*rate));
            if q.build_instrument(self.reference_date, Level::Mid, None)
                .is_ok()
            {
                Some(q)
            } else {
                None
            }
        }
        fn reference_date(&self) -> Date {
            self.reference_date
        }
    }

    #[test]
    fn test_dependency_graph() -> Result<()> {
        let dc = DayCounter::Actual360;
        let interp = Interpolator::Linear;
        let enable_extrapolation = true;
        let ref_date = Date::new(2024, 1, 2);

        let mut selector = MapSelector::new(ref_date);
        selector.add("OIS_USD_SOFR_1Y", 0.05);
        selector.add("OIS_EUR_EURIBOR1m_1Y", 0.03);
        selector.add("FxForwardPoints_USDEUR_1M", 0.03);

        let quotes = vec!["OIS_USD_SOFR_1Y".to_string()];
        let index_a = MarketIndex::SOFR;
        let mut curve_a =
            CurveConfiguration::new(index_a.clone(), dc, interp, enable_extrapolation, quotes);
        curve_a.resolve(&selector, Level::Mid, None)?;

        let quotes = vec!["OIS_EUR_EURIBOR1m_1Y".to_string()];
        let index_b = MarketIndex::EURIBOR1m;
        let mut curve_b =
            CurveConfiguration::new(index_b.clone(), dc, interp, enable_extrapolation, quotes);
        curve_b.resolve(&selector, Level::Mid, None)?;

        // Collateral curve required by the discount policy for EUR cashflows under USD CSA.
        let index_c = MarketIndex::Collateral(Currency::EUR, Currency::USD);
        let quotes = vec!["FxForwardPoints_USDEUR_1M".to_string()];
        let mut curve_c =
            CurveConfiguration::new(index_c.clone(), dc, interp, enable_extrapolation, quotes);
        curve_c.resolve(&selector, Level::Mid, None)?;

        let configs = vec![
            (index_a.clone(), curve_a),
            (index_b, curve_b),
            (index_c, curve_c),
        ]
        .into_iter()
        .collect::<HashMap<_, _>>();
        let policy = BootstrapDiscountPolicy::new(index_a, Currency::USD);
        let dependencies = dependency_order(&configs, &policy)?;
        assert_eq!(
            dependencies,
            vec![
                MarketIndex::SOFR,
                MarketIndex::Collateral(Currency::EUR, Currency::USD),
                MarketIndex::EURIBOR1m
            ]
        );
        Ok(())
    }
}
