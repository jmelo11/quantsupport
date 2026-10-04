//! Callable-bond pricing with a curve-fitted recombining trinomial tree.

use std::collections::{BTreeMap, BTreeSet};

use super::bondpricer::{
    bond_market_data_request, evaluate_bond, impl_bond_pricer, transition_variances,
    BondPricerSettings, BondPricingAnchor, BondPricingEngine, BondState, BondTerms, LatticeTerms,
    NumericalMethod,
};
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketDataProvider, MarketDataRequest},
        pricer::Pricer,
        request::{HandleCashflows, Request},
    },
    instruments::fixedincome::steppedcouponbond::SteppedCouponBondTrade,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// Callable-bond pricer using a curve-fitted recombining trinomial tree.
///
/// The tree matches the first two moments of the Hull-White transition and
/// fits each state-price layer to the initial discount curve.
pub struct TreeBasedBondPricer {
    pub(super) settings: BondPricerSettings,
}

impl TreeBasedBondPricer {
    /// Creates a tree-based bond pricer using context-date settlement.
    #[must_use]
    pub const fn new(anchor: BondPricingAnchor) -> Self {
        Self {
            settings: BondPricerSettings::new(anchor),
        }
    }

    /// Uses an explicit settlement date, equal to evaluation for callable bonds.
    #[must_use]
    pub fn with_settlement_date(mut self, settlement_date: Date) -> Self {
        self.settings = self.settings.with_settlement_date(settlement_date);
        self
    }

    /// Overrides the spread day counter.
    #[must_use]
    pub fn with_spread_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.settings = self.settings.with_spread_day_counter(day_counter);
        self
    }

    /// Returns the configured price or spread anchor.
    #[must_use]
    pub const fn anchor(&self) -> BondPricingAnchor {
        self.settings.anchor()
    }
}

impl Default for TreeBasedBondPricer {
    fn default() -> Self {
        Self::new(BondPricingAnchor::OAS(0.0))
    }
}
#[derive(Clone, Copy)]
struct Transition<T: Scalar> {
    down: i32,
    middle: i32,
    up: i32,
    probability_down: T,
    probability_middle: T,
    probability_up: T,
}

type TransitionLayers<T> = Vec<BTreeMap<i32, Transition<T>>>;

/// Builds a recombining trinomial transition graph matching the conditional
/// mean and variance of the Hull-White state over each time interval.
fn lattice_transitions<T: Scalar>(
    alpha: f64,
    variances: &[T],
    time_step: f64,
) -> Result<(TransitionLayers<T>, Vec<T>)> {
    let steps = variances.len();
    if variances
        .iter()
        .all(|variance| variance.value() <= f64::EPSILON)
    {
        let transition = Transition {
            down: 0,
            middle: 0,
            up: 0,
            probability_down: T::zero(),
            probability_middle: T::one(),
            probability_up: T::zero(),
        };
        return Ok((
            (0..steps)
                .map(|_| BTreeMap::from([(0, transition)]))
                .collect(),
            vec![T::zero(); steps + 1],
        ));
    }

    let decay = (-alpha * time_step).exp();
    let mut spacings = vec![variances[0].mul_val(T::scalar(3.0)).sqrt()];
    let mut layers = Vec::with_capacity(steps);
    let mut nodes = BTreeSet::from([0_i32]);

    for &variance in variances {
        if variance.value() <= f64::EPSILON {
            return Err(QSError::InvalidValueErr(
                "Hull-White tree requires positive variance on each stochastic interval".into(),
            ));
        }
        let current_spacing = *spacings
            .last()
            .ok_or_else(|| QSError::UnexpectedErr("Missing tree state spacing".into()))?;
        let state_spacing = variance.mul_val(T::scalar(3.0)).sqrt();
        let denominator = variance.mul_val(T::scalar(6.0));
        let probability_scale = state_spacing.mul_val(T::scalar(2.0));
        let mut transitions = BTreeMap::new();
        let mut next_nodes = BTreeSet::new();
        for node in nodes.iter().copied() {
            let state = current_spacing.mul_val(T::scalar(f64::from(node)));
            let expectation = state.mul_val(T::scalar(decay));
            #[allow(clippy::cast_possible_truncation)]
            let middle = expectation.div_val(state_spacing).value().round() as i32;
            let error = expectation.sub_val(state_spacing.mul_val(T::scalar(f64::from(middle))));
            let square_term = error.mul_val(error).div_val(denominator);
            let mut probability_down = T::scalar(1.0 / 6.0)
                .add_val(square_term)
                .sub_val(error.div_val(probability_scale));
            let mut probability_middle =
                T::scalar(2.0 / 3.0).sub_val(square_term.mul_val(T::scalar(2.0)));
            let mut probability_up = T::scalar(1.0 / 6.0)
                .add_val(square_term)
                .add_val(error.div_val(probability_scale));
            if probability_down.value() < -1.0e-12
                || probability_middle.value() < -1.0e-12
                || probability_up.value() < -1.0e-12
            {
                return Err(QSError::InvalidValueErr(
                    "Hull-White lattice produced a negative transition probability".into(),
                ));
            }
            probability_down = probability_down.max_val(T::zero());
            probability_middle = probability_middle.max_val(T::zero());
            probability_up = probability_up.max_val(T::zero());
            let total = probability_down
                .add_val(probability_middle)
                .add_val(probability_up);
            probability_down = probability_down.div_val(total);
            probability_middle = probability_middle.div_val(total);
            probability_up = probability_up.div_val(total);
            let transition = Transition {
                down: middle - 1,
                middle,
                up: middle + 1,
                probability_down,
                probability_middle,
                probability_up,
            };
            next_nodes.insert(transition.down);
            next_nodes.insert(transition.middle);
            next_nodes.insert(transition.up);
            transitions.insert(node, transition);
        }
        layers.push(transitions);
        spacings.push(state_spacing);
        nodes = next_nodes;
    }
    Ok((layers, spacings))
}

#[allow(clippy::too_many_lines)]
/// Fits the tree to the discount curve through state prices and performs
/// backward induction with OAS discounting and issuer-call obstacles.
pub(super) fn lattice_value<T: Scalar>(
    mut discount_factor: impl FnMut(f64) -> Result<T>,
    terms: &LatticeTerms<T>,
    alpha: f64,
    sigma_schedule: &[(f64, T)],
    spread: T,
) -> Result<T> {
    let time_step =
        terms.maturity_time
            / f64::from(u32::try_from(terms.steps).map_err(|_| {
                QSError::InvalidValueErr("Bond lattice step count exceeds u32".into())
            })?);
    let variances = transition_variances(alpha, sigma_schedule, terms.steps, time_step)?;
    let (transitions, spacings) = lattice_transitions(alpha, &variances, time_step)?;

    let mut state_prices = BTreeMap::from([(0_i32, T::one())]);
    let mut shifts = Vec::with_capacity(terms.steps);
    for (step, layer) in transitions.iter().enumerate() {
        let state_spacing = spacings[step];
        let weighted_state_price = state_prices.iter().fold(T::zero(), |total, (node, value)| {
            let state_discount = state_spacing
                .mul_val(T::scalar(-f64::from(*node) * time_step))
                .exp();
            total.add_val(value.mul_val(state_discount))
        });
        let next_time = time_step
            * f64::from(u32::try_from(step + 1).map_err(|_| {
                QSError::InvalidValueErr("Bond lattice time index exceeds u32".into())
            })?);
        let target_discount = discount_factor(next_time)?;
        if target_discount.value() <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Bond lattice requires positive discount factors".into(),
            ));
        }
        let shift = weighted_state_price
            .div_val(target_discount)
            .ln()
            .div_val(T::scalar(time_step));
        shifts.push(shift);

        let mut next_state_prices = BTreeMap::<i32, T>::new();
        for (node, state_price) in &state_prices {
            let transition = layer.get(node).ok_or_else(|| {
                QSError::NotFoundErr("Missing Hull-White lattice transition".into())
            })?;
            let short_rate = shift.add_val(state_spacing.mul_val(T::scalar(f64::from(*node))));
            let discount = short_rate.mul_val(T::scalar(-time_step)).exp();
            let discounted_state_price = state_price.mul_val(discount);
            for (destination, probability) in [
                (transition.down, transition.probability_down),
                (transition.middle, transition.probability_middle),
                (transition.up, transition.probability_up),
            ] {
                let contribution = discounted_state_price.mul_val(probability);
                next_state_prices
                    .entry(destination)
                    .and_modify(|value| *value = value.add_val(contribution))
                    .or_insert(contribution);
            }
        }
        state_prices = next_state_prices;
    }

    let final_cashflow = terms.cashflows[terms.steps];
    let final_nodes = transitions.last().map_or_else(
        || BTreeSet::from([0]),
        |layer| {
            layer
                .values()
                .flat_map(|transition| [transition.down, transition.middle, transition.up])
                .collect::<BTreeSet<_>>()
        },
    );
    let mut values = final_nodes
        .into_iter()
        .map(|node| (node, final_cashflow))
        .collect::<BTreeMap<_, _>>();

    for step in (0..terms.steps).rev() {
        let mut previous_values = BTreeMap::new();
        for (node, transition) in &transitions[step] {
            let expected = [
                (transition.down, transition.probability_down),
                (transition.middle, transition.probability_middle),
                (transition.up, transition.probability_up),
            ]
            .into_iter()
            .try_fold(T::zero(), |total, (destination, probability)| {
                let value = values.get(&destination).ok_or_else(|| {
                    QSError::NotFoundErr("Missing Hull-White lattice node value".into())
                })?;
                Ok::<T, QSError>(total.add_val(value.mul_val(probability)))
            })?;
            let short_rate = shifts[step]
                .add_val(spacings[step].mul_val(T::scalar(f64::from(*node))))
                .add_val(spread);
            let continuation = expected.mul_val(short_rate.mul_val(T::scalar(-time_step)).exp());
            let event = terms.calls[step];
            let mut ex_coupon_value = continuation;
            if let Some(call_price) = event.call_price {
                ex_coupon_value = ex_coupon_value.min_val(T::scalar(call_price));
            }
            previous_values.insert(*node, ex_coupon_value.add_val(terms.cashflows[step]));
        }
        values = previous_values;
    }
    values
        .get(&0)
        .copied()
        .ok_or_else(|| QSError::NotFoundErr("Hull-White lattice root is missing".into()))
}

impl BondPricingEngine for TreeBasedBondPricer {
    fn settings(&self) -> &BondPricerSettings {
        &self.settings
    }

    fn numerical_method(&self) -> NumericalMethod {
        NumericalMethod::Tree
    }
}

impl<T: BondTerms> HandleCashflows<T, BondState> for TreeBasedBondPricer {}

impl_bond_pricer!(TreeBasedBondPricer, SteppedCouponBondTrade<DualFwd>);
