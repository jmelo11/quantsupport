use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    ad::adreal::{ADReal, IsReal},
    currencies::{currency::Currency, exchangeratestore::ExchangeRateStore},
    indices::marketindex::MarketIndex,
    instruments::cashflows::leg::Leg,
    math::interpolation::interpolator::{Interpolate, Interpolator},
    rates::{
        bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            curveconfiguration::CurveConfiguration, resolvedinstrument::ResolvedInstrument,
        },
        interestrate::{InterestRate, RateDefinition},
    },
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// Computes the pillar time grid from instruments.
pub fn get_pillar_times(
    reference_date: Date,
    day_counter: DayCounter,
    instruments: &[ResolvedInstrument],
) -> Vec<f64> {
    let mut times = vec![0.0_f64];
    for instr in instruments {
        times.push(day_counter.year_fraction(reference_date, instr.pillar_date()));
    }
    times
}

/// Computes a topological ordering of curves respecting dependencies.
pub fn dependency_order(
    curve_configs: &HashMap<MarketIndex, CurveConfiguration>,
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

/// Internally constructed curve representation used during bootstrapping.
#[derive(Clone)]
pub struct SolvedCurve {
    market_index: MarketIndex,
    reference_date: Date,
    times: Vec<f64>,
    discount_factors: Vec<f64>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    pillar_values: Option<Vec<ADReal>>,
    output_discount_factors: Option<Vec<ADReal>>,
    ift_sensitivities: Option<Vec<Vec<f64>>>,
}

impl SolvedCurve {
    /// Creates a new solved curve from raw data.
    pub fn new(
        market_index: MarketIndex,
        reference_date: Date,
        times: Vec<f64>,
        discount_factors: Vec<f64>,
        day_counter: DayCounter,
        interpolator: Interpolator,
    ) -> Self {
        Self {
            market_index,
            reference_date,
            times,
            discount_factors,
            day_counter,
            interpolator,
            pillar_values: None,
            output_discount_factors: None,
            ift_sensitivities: None,
        }
    }

    /// Returns the market index of this curve.
    pub fn market_index(&self) -> MarketIndex {
        self.market_index.clone()
    }

    /// Attaches AD-tracked pillar values (typically market quotes).
    pub fn with_pillar_values(mut self, pillar_values: Vec<ADReal>) -> Self {
        self.pillar_values = Some(pillar_values);
        self
    }

    pub(crate) fn with_output_discount_factors(
        mut self,
        output_discount_factors: Vec<ADReal>,
    ) -> Self {
        self.output_discount_factors = Some(output_discount_factors);
        self
    }

    /// Attaches the IFT sensitivity matrix: `sens[i][j]` = ∂DF(i+1)/∂q(j).
    pub(crate) fn with_ift_sensitivities(mut self, sensitivities: Vec<Vec<f64>>) -> Self {
        self.ift_sensitivities = Some(sensitivities);
        self
    }

    /// Returns the IFT sensitivity matrix, if available.
    pub(crate) fn ift_sensitivities(&self) -> Option<&Vec<Vec<f64>>> {
        self.ift_sensitivities.as_ref()
    }

    /// Returns the AD-tracked pillar values.
    pub fn pillar_values(&self) -> Result<&[ADReal]> {
        self.pillar_values
            .as_ref()
            .map(|v| v.as_slice())
            .ok_or_else(|| QSError::InvalidValueErr("Pillar values not set".into()))
    }

    /// Returns the raw discount factors.
    pub fn discount_factors(&self) -> &[f64] {
        &self.discount_factors
    }

    /// Returns the discount factor at the given date.
    pub fn discount_factor(&self, date: Date) -> Result<f64> {
        let year_fraction = self.day_counter.year_fraction(self.reference_date, date);
        self.interpolator
            .interpolate(year_fraction, &self.times, &self.discount_factors, true)
    }

    /// Returns the AD-tracked discount factors at the pillar dates.
    pub fn output_discount_factors(&self) -> Result<&[ADReal]> {
        self.output_discount_factors
            .as_ref()
            .map(|values| values.as_slice())
            .ok_or_else(|| QSError::InvalidValueErr("Output discount factors not set".into()))
    }

    /// Computes the forward rate between two dates.
    pub fn forward_rate(
        &self,
        start_date: Date,
        end_date: Date,
        rate_definition: RateDefinition,
    ) -> Result<f64> {
        let discount_factor_to_start = self.discount_factor(start_date)?;
        let discount_factor_to_end = self.discount_factor(end_date)?;
        let comp_factor = discount_factor_to_start / discount_factor_to_end;
        let tenor = self.day_counter.year_fraction(start_date, end_date);

        Ok(InterestRate::<f64>::implied_rate(
            comp_factor,
            self.day_counter,
            rate_definition.compounding(),
            rate_definition.frequency(),
            tenor,
        )?
        .rate())
    }
}

/// Set of curves available during a single Newton iteration.
///
/// Merges the trial curve (the one being solved) with all previously-solved
/// curves and provides per-leg curve resolution via the
/// [`BootstrapDiscountPolicy`].
pub struct BootstrapCurveSet<'a> {
    curves: HashMap<MarketIndex, &'a SolvedCurve>,
    discount_policy: &'a BootstrapDiscountPolicy,
    exchange_rate_store: &'a ExchangeRateStore,
}

impl<'a> BootstrapCurveSet<'a> {
    /// Builds a curve set from the trial curve and the already-solved curves.
    pub fn new(
        trial: &'a SolvedCurve,
        other_curves: &'a HashMap<MarketIndex, SolvedCurve>,
        discount_policy: &'a BootstrapDiscountPolicy,
        exchange_rate_store: &'a ExchangeRateStore,
    ) -> Self {
        let mut curves: HashMap<MarketIndex, &SolvedCurve> =
            other_curves.iter().map(|(k, v)| (k.clone(), v)).collect();
        curves.insert(trial.market_index(), trial);
        Self {
            curves,
            discount_policy,
            exchange_rate_store,
        }
    }

    /// Looks up a curve by market index.
    pub fn get(&self, index: &MarketIndex) -> Option<&SolvedCurve> {
        self.curves.get(index).copied()
    }

    /// Returns the discount policy.
    pub fn discount_policy(&self) -> &BootstrapDiscountPolicy {
        self.discount_policy
    }

    /// Resolves the discount curve for the given leg via the discount policy.
    pub fn discount_curve_for_leg(&self, leg: &Leg<f64>) -> Result<&SolvedCurve> {
        let index = self.discount_policy.discount_index(leg)?;
        self.curves
            .get(&index)
            .copied()
            .ok_or_else(|| QSError::NotFoundErr(format!("Missing discount curve {index}")))
    }

    /// Resolves the forward/projection curve for the given leg, if it has one.
    pub fn forward_curve_for_leg(&self, leg: &Leg<f64>) -> Result<Option<&SolvedCurve>> {
        match leg.forward_index() {
            Some(idx) => {
                let curve =
                    self.curves.get(idx).copied().ok_or_else(|| {
                        QSError::NotFoundErr(format!("Missing forward curve {idx}"))
                    })?;
                Ok(Some(curve))
            }
            None => Ok(None),
        }
    }

    /// Looks up the FX spot rate.
    pub fn fx_spot(&self, base: Currency, quote: Currency) -> Result<f64> {
        Ok(self
            .exchange_rate_store
            .get_exchange_rate(base, quote)?
            .value())
    }
}
