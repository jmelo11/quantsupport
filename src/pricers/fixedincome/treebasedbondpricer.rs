//! Trinomial-tree valuation of callable bond trades.
use super::bondanalytics::{
    bond_market_data_request, contractual_market_request, discounted_value, evaluate_bond,
    impl_bond_pricer, BondPricingEngine, BondState, BondTerms, BondValuation,
};
use super::{
    bondevents::BondEventGrid,
    callablebond::{prepare_callable_bond, pricer_configuration, CallableBondModel},
};
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::DiscountPolicy,
        evaluationresults::EvaluationResults,
        marketdatahandling::marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        pricer::Pricer,
        request::{HandleCashflows, Request},
    },
    indices::marketindex::MarketIndex,
    utils::errors::Result,
};
use crate::{
    core::pricerconfiguration::PricerImplementation,
    instruments::fixedincome::steppedcouponbond::SteppedCouponBondTrade,
    models::hullwhite::parameters::transition_variances, utils::errors::QSError,
};
use std::collections::{BTreeMap, BTreeSet};
/// Callable bond pricer with a recombining trinomial tree.
#[derive(Default)]
pub struct TreeBasedBondPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}
impl TreeBasedBondPricer {
    /// Creates a reusable engine; quote and settlement terms belong to each trade.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    terms: &BondEventGrid<T>,
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
    type Model<T: Scalar> = Option<CallableBondModel<T>>;
    fn policy(&self) -> Option<&dyn DiscountPolicy> {
        self.discount_policy.as_deref()
    }
    fn prepare(
        &self,
        trade: &impl BondTerms,
        index: &MarketIndex,
        context: &impl MarketDataProvider,
    ) -> Result<(MarketData, Self::Model<f64>)> {
        if trade.call_schedule().is_empty() {
            return Ok((
                context.handle_request(&contractual_market_request(
                    trade,
                    index,
                    context.evaluation_date(),
                ))?,
                None,
            ));
        }
        let configuration = pricer_configuration(trade, context)?;
        let time_grid = match configuration.pricer() {
            PricerImplementation::TreeBased { time_grid } => *time_grid,
            actual => {
                return Err(QSError::InvalidValueErr(format!(
                "TreeBasedBondPricer requires a TreeBased context configuration, got {actual:?}"
            )))
            }
        };
        let (market, model) =
            prepare_callable_bond(trade, index, context, &configuration, time_grid)?;
        Ok((market, Some(model)))
    }
    fn ad_model(
        &self,
        model: &Self::Model<f64>,
        market: &MarketData,
    ) -> Result<Self::Model<DualFwd>> {
        model
            .as_ref()
            .map(|model| model.ad_model(market))
            .transpose()
    }
    fn value<T: Scalar>(
        &self,
        model: &Self::Model<T>,
        valuation: &BondValuation<'_, T>,
        spread: T,
    ) -> Result<T> {
        if let Some(callable) = model {
            let terms = callable.events(valuation)?;
            lattice_value(
                |time| valuation.curve.discount_factor_from_time(time),
                &terms,
                callable.parameters.alpha,
                &callable.parameters.sigma_schedule,
                spread,
            )
        } else {
            discounted_value(valuation, spread)
        }
    }
}
impl<T: BondTerms> HandleCashflows<T, BondState> for TreeBasedBondPricer {}
impl_bond_pricer!(TreeBasedBondPricer, SteppedCouponBondTrade<DualFwd>);

#[cfg(test)]
pub(super) mod test_support {
    use super::super::bondpricer::test_support::{policy, TestProvider, FACE};
    use super::TreeBasedBondPricer;
    use crate::prelude::*;
    use crate::{
        currencies::currency::Currency,
        instruments::{
            cashflows::makeleg::PaymentStructure,
            fixedincome::{
                callschedule::CallScheduleEntry, makesteppedcouponbond::MakeSteppedCouponBond,
            },
        },
        time::enums::Frequency,
    };

    pub fn tree_pricer() -> TreeBasedBondPricer {
        let mut pricer = TreeBasedBondPricer::new();
        pricer.set_discount_policy(policy());
        pricer
    }

    pub fn bond(
        evaluation_date: Date,
        maturity_date: Date,
        coupon: f64,
        _structure: PaymentStructure,
        call_schedule: Vec<CallScheduleEntry>,
    ) -> SteppedCouponBondTrade<DualFwd> {
        let pricing_anchor = if call_schedule.is_empty() {
            BondPricingAnchor::ZSpread(0.0)
        } else {
            BondPricingAnchor::OAS(0.0)
        };
        let instrument = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("TEST-BOND".into())
            .with_start_date(evaluation_date)
            .with_maturity_date(maturity_date)
            .with_units(FACE)
            .with_notional(FACE)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_coupons(vec![CashflowType::fixed(coupon)])
            .with_notional_percentages(vec![1.0])
            .with_call_schedule(call_schedule)
            .build()
            .unwrap_or_else(|error| panic!("test bond: {error}"));
        SteppedCouponBondTrade::new(
            instrument,
            evaluation_date,
            FACE,
            crate::core::trade::Side::LongReceive,
            pricing_anchor,
        )
    }

    pub fn value(
        pricer: &TreeBasedBondPricer,
        trade: &SteppedCouponBondTrade<DualFwd>,
        provider: &TestProvider,
    ) -> f64 {
        pricer
            .evaluate(trade, &[Request::Value], provider)
            .unwrap_or_else(|error| panic!("bond value: {error}"))
            .price()
            .unwrap_or_else(|| panic!("bond value must be present"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        backwardfourierevolutionbondpricer::{
            backward_fourier_value,
            test_support::{fourier_pricer, fourier_provider, fourier_value},
            BackwardFourierEvolutionBondPricer,
        },
        bondevents::{BondEventGrid, CallStep},
        bondpricer::{
            test_support::{
                fixed_bond, fixed_pricer, fixed_value, provider, provider_with_method,
                TestProvider, ALPHA, FACE, RATE, SIGMA,
            },
            BondPricer,
        },
    };
    use super::{lattice_value, test_support::*, TreeBasedBondPricer};
    use crate::prelude::*;
    use crate::{
        core::{
            collateral::FixedIncomeDiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore,
            pricerconfiguration::{PricerConfiguration, PricingModelConfiguration},
            pricingcontext::PricingContext,
        },
        currencies::currency::Currency,
        instruments::{
            cashflows::makeleg::PaymentStructure,
            fixedincome::{
                callschedule::CallScheduleEntry, makesteppedcouponbond::MakeSteppedCouponBond,
            },
        },
        models::{
            hullwhite::{
                hullwhitemodel::HullWhite,
                parameters::{hull_white_piecewise_variance, HullWhiteParameters},
            },
            lgm::lgmcomponents::LgmRateModel,
            modelconfiguration::ParameterSource,
        },
        quotes::{
            quote::{Level, Quote, QuoteLevels},
            quotestore::{QuoteStore, QuoteStoreRecords},
        },
        rates::{
            compounding::Compounding,
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure, primalcurve::PrimalCurve,
            },
        },
        time::{
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        volatility::{
            modelcalibration::{CalibrationBasket, ModelCalibrationConfiguration},
            volatilitycubeconfiguration::VolatilityCubeConfiguration,
            volatilityindexing::{SmileType, Strike, VolatilityType},
            volatilitysurfaceconfiguration::VolatilitySurfaceConfiguration,
        },
    };
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    fn example_calibrated_context(
        use_fourier: bool,
        bump: Option<(&str, f64)>,
    ) -> Result<PricingContext> {
        let mut records: QuoteStoreRecords =
            serde_json::from_str(include_str!("../../../examples/hullwhite/data/quotes.json"))
                .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        if let Some((identifier, amount)) = bump {
            let quote = records
                .quotes
                .iter_mut()
                .find(|quote| quote.identifier == identifier)
                .ok_or_else(|| QSError::NotFoundErr(identifier.into()))?;
            quote.mid = quote.mid.map(|value| value + amount);
        }
        let quotes = QuoteStore::try_from(records)?;
        let curve_json: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/hullwhite/data/curve_specs.json"
        ))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let curves = serde_json::from_value(curve_json["curve_specs"].clone())
            .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let surface_json: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/hullwhite/data/vol_specs.json"
        ))
        .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let surfaces: Vec<VolatilitySurfaceConfiguration> =
            serde_json::from_value(surface_json["volatility_surfaces"].clone())
                .map_err(|error| QSError::InvalidValueErr(error.to_string()))?;
        let time_grid = TimeGridConfiguration::new(48, 4096, DayCounter::Actual365);
        let pricer = if use_fourier {
            PricerImplementation::BackwardFourierEvolution {
                time_grid,
                fourier_grid: FourierGridConfiguration::new(512, 10.0),
            }
        } else {
            PricerImplementation::TreeBased { time_grid }
        };
        let configuration = PricerConfiguration::new(
            InstrumentType::SteppedCouponBond,
            pricer,
            vec![PricingModelConfiguration::new(
                MarketIndex::SOFR,
                ModelConfiguration::HullWhite {
                    alpha: 0.05,
                    parameter_source: ParameterSource::Calibrated(
                        ModelCalibrationConfiguration::new(CalibrationSource::Surface {
                            market_index: MarketIndex::SOFR,
                        })
                        .with_strike(Strike::Absolute(0.045)),
                    ),
                },
            )],
        );
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
            .with_curve_configurations(curves)
            .with_volatility_surface_configurations(surfaces)
            .with_pricer_configurations(vec![configuration]);
        context.initialize()?;
        Ok(context)
    }

    struct RecordingProvider<'a> {
        context: &'a PricingContext,
        calibrated_request_seen: RefCell<bool>,
    }

    impl MarketDataProvider for RecordingProvider<'_> {
        fn evaluation_date(&self) -> Date {
            self.context.evaluation_date()
        }
        fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
            if request.calibration_request().is_some() {
                let elements = request.constructed_elements_request().ok_or_else(|| {
                    QSError::ValueNotSetErr("Calibration elements request".into())
                })?;
                assert!(elements.iter().any(|element| matches!(
                    element,
                    ConstructedElementRequest::VolatilitySurface {
                        market_index: MarketIndex::SOFR
                    }
                )));
                assert!(elements.iter().any(|element| matches!(
                    element,
                    ConstructedElementRequest::DiscountCurve {
                        market_index: MarketIndex::SOFR
                    }
                )));
                *self.calibrated_request_seen.borrow_mut() = true;
            }
            self.context.handle_request(request)
        }
    }

    fn callable_zero_coupon_value(steps_per_year: usize) -> (f64, f64) {
        let evaluation_date = Date::new(2025, 1, 1);
        let call_date = Date::new(2026, 1, 1);
        let maturity_date = Date::new(2027, 1, 1);
        let (provider, curve) = provider(evaluation_date, steps_per_year);
        let strike = 0.95;
        let trade = bond(
            evaluation_date,
            maturity_date,
            0.0,
            PaymentStructure::Zero,
            vec![CallScheduleEntry::new(call_date, strike * FACE)],
        );
        let lattice = value(
            &tree_pricer(),
            &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &provider,
        );
        let model = HullWhite::new(ALPHA, &curve).with_constant_volatility(SIGMA);
        let option = model
            .bond_call_price(1.0, 2.0, strike, SIGMA, &curve)
            .unwrap_or_else(|error| panic!("closed-form bond call: {error}"));
        let benchmark = FACE
            * (curve
                .discount_factor_from_time(2.0)
                .unwrap_or_else(|error| panic!("discount factor: {error}"))
                - option);
        (lattice, benchmark)
    }

    #[test]
    fn piecewise_native_engines_match_bond_option_and_straight_curve_fit() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let curve = FlatForwardTermStructure::new(
            evaluation,
            RATE,
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        );
        let schedule = [(0.5, 0.01), (1.0, 0.02), (2.5, 0.005)];
        let mut terms = BondEventGrid {
            maturity_time: 3.0,
            steps: 384,
            cashflows: vec![0.0; 385],
            calls: vec![CallStep::new(); 385],
        };
        terms.cashflows[384] = FACE;
        terms.calls[256].call_price = Some(95.0);
        let variance = hull_white_piecewise_variance(ALPHA, &schedule, 0.0, 2.0);
        let unit_variance = hull_white_piecewise_variance(ALPHA, &[(0.0, 1.0)], 0.0, 2.0);
        let equivalent_sigma = (variance / unit_variance).sqrt();
        let benchmark = FACE
            * (curve.discount_factor_from_time(3.0)?
                - HullWhite::new(ALPHA, &curve).bond_call_price(
                    2.0,
                    3.0,
                    0.95,
                    equivalent_sigma,
                    &curve,
                )?);
        let tree = lattice_value(
            |time| curve.discount_factor_from_time(time),
            &terms,
            ALPHA,
            &schedule,
            0.0,
        )?;
        let fourier = backward_fourier_value(
            |time| curve.discount_factor_from_time(time),
            &terms,
            ALPHA,
            &schedule,
            0.0,
            FourierGridConfiguration::new(1024, 10.0),
        )?;
        assert!(
            (tree - benchmark).abs() < 0.08,
            "piecewise tree {tree} vs bond-option benchmark {benchmark}"
        );
        assert!(
            (fourier - benchmark).abs() < 0.10,
            "piecewise Fourier {fourier} vs bond-option benchmark {benchmark}"
        );
        terms.calls[256].call_price = None;
        let straight = FACE * curve.discount_factor_from_time(3.0)?;
        assert!(
            (lattice_value(
                |time| curve.discount_factor_from_time(time),
                &terms,
                ALPHA,
                &schedule,
                0.0
            )? - straight)
                .abs()
                < 1.0e-10
        );
        assert!(
            (backward_fourier_value(
                |time| curve.discount_factor_from_time(time),
                &terms,
                ALPHA,
                &schedule,
                0.0,
                FourierGridConfiguration::new(1024, 10.0)
            )? - straight)
                .abs()
                < 0.10
        );
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn calibrated_surface_native_engines_preserve_schedule_and_quote_risk() -> Result<()> {
        let quote = "CapletFloorlet_USD_SOFR_3M_2Y_Absolute_0.045_Straddle_Black";
        let bump = 1.0e-4;
        let mut prices = Vec::new();
        for use_fourier in [false, true] {
            Tape::rewind_to_init_fwd();
            let context = example_calibrated_context(use_fourier, None)?;
            let market = MarketData::new(HashMap::new(), context.constructed_elements().clone())
                .with_calibration_quotes(context.quote_store().clone())
                .with_pricer_configurations(HashMap::from([(
                    InstrumentType::SteppedCouponBond,
                    context.pricer_configurations()[0].clone(),
                )]));
            let configuration = market
                .pricer_configurations()
                .get(&InstrumentType::SteppedCouponBond)
                .ok_or_else(|| QSError::NotFoundErr("Stepped bond configuration".into()))?
                .model(&MarketIndex::SOFR)
                .ok_or_else(|| QSError::NotFoundErr("Hull-White model".into()))?
                .clone();
            let mut resolved = HullWhiteParameters::new(configuration.clone())?;
            resolved.resolve_calibration(&market, &MarketIndex::SOFR)?;
            assert_eq!(resolved.sigma_schedule.len(), 9);
            assert!(resolved
                .sigma_schedule
                .windows(2)
                .any(|entries| (entries[0].1 - entries[1].1).abs() > 1.0e-5));
            let curve = market
                .constructed_elements()
                .discount_curve(&MarketIndex::SOFR)
                .ok_or_else(|| QSError::NotFoundErr("SOFR curve".into()))?
                .curve();
            let primal = PrimalCurve(&*curve);
            let ModelConfiguration::HullWhite {
                parameter_source, ..
            } = &configuration
            else {
                return Err(QSError::UnexpectedErr(
                    "Expected Hull-White configuration".into(),
                ));
            };
            let lgm = LgmRateModel::from_configuration(
                &ModelConfiguration::Lgm {
                    lambda: 0.05,
                    parameter_source: parameter_source.clone(),
                },
                &primal,
                market.constructed_elements(),
                context.quote_store(),
                Level::Mid,
            )?;
            assert_eq!(resolved.sigma_schedule, lgm.sigma_schedule());
            drop(curve);
            let evaluation = context.evaluation_date();
            let trade = bond(
                evaluation,
                evaluation + 6 * 365,
                0.05,
                PaymentStructure::Bullet,
                vec![CallScheduleEntry::window(
                    evaluation + 3 * 365,
                    evaluation + 5 * 365,
                    100.0,
                )],
            );
            let provider = RecordingProvider {
                context: &context,
                calibrated_request_seen: RefCell::new(false),
            };
            let evaluate = |provider: &RecordingProvider<'_>, requests: &[Request]| {
                if use_fourier {
                    fourier_pricer().evaluate(
                        &trade
                            .clone()
                            .with_pricing_anchor(BondPricingAnchor::OAS(0.002)),
                        requests,
                        provider,
                    )
                } else {
                    tree_pricer().evaluate(
                        &trade
                            .clone()
                            .with_pricing_anchor(BondPricingAnchor::OAS(0.002)),
                        requests,
                        provider,
                    )
                }
            };
            let results = evaluate(&provider, &[Request::Value, Request::Sensitivities])?;
            assert!(*provider.calibrated_request_seen.borrow());
            let price = results
                .price()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibrated bond price".into()))?;
            assert!(price.is_finite() && price > 0.0);
            prices.push(price);
            let risk = results
                .sensitivities()
                .ok_or_else(|| QSError::ValueNotSetErr("Calibrated bond sensitivities".into()))?;
            assert!(risk
                .instrument_keys()
                .iter()
                .any(|label| label.starts_with("OIS_")));
            assert!(risk
                .instrument_keys()
                .iter()
                .any(|label| label == "TEST-BOND.OAS"));
            let adjoint = risk
                .instrument_keys()
                .iter()
                .zip(risk.exposure())
                .find_map(|(label, value)| (label == quote).then_some(*value))
                .ok_or_else(|| QSError::NotFoundErr("Calibrated caplet quote risk".into()))?;
            assert!(
                adjoint.abs() > 1.0e-4,
                "vol quote sensitivity must not be discarded"
            );
            let mut bumped_prices = Vec::new();
            for amount in [bump, -bump] {
                Tape::rewind_to_init_fwd();
                let bumped = example_calibrated_context(use_fourier, Some((quote, amount)))?;
                let bumped_provider = RecordingProvider {
                    context: &bumped,
                    calibrated_request_seen: RefCell::new(false),
                };
                bumped_prices.push(
                    evaluate(&bumped_provider, &[Request::Value])?
                        .price()
                        .ok_or_else(|| QSError::ValueNotSetErr("Bumped bond price".into()))?,
                );
            }
            let finite_difference = (bumped_prices[0] - bumped_prices[1]) / (2.0 * bump);
            eprintln!("Calibrated HullWhite Fourier={use_fourier}: PV={price:.8}, caplet AD={adjoint:.8}, rebuilt-market bump={finite_difference:.8}");
            assert!(
                (adjoint - finite_difference).abs() < 0.03 * finite_difference.abs().max(0.01),
                "Fourier={use_fourier}: AD {adjoint} vs rebuilt surface bump {finite_difference}"
            );
        }
        assert!(
            (prices[0] - prices[1]).abs() < 0.20,
            "calibrated tree {} vs Fourier {}",
            prices[0],
            prices[1]
        );
        Ok(())
    }

    #[test]
    fn calibrated_cube_native_engines_report_swaption_risk() -> Result<()> {
        for use_fourier in [false, true] {
            Tape::rewind_to_init_fwd();
            let evaluation = Date::new(2025, 1, 1);
            let (base, _) = provider_with_method(evaluation, 24, use_fourier);
            let mut quotes = QuoteStore::new(evaluation);
            let mut identifiers = Vec::new();
            for expiry in ["1Y", "2Y", "3Y"] {
                for tenor in ["5Y", "10Y"] {
                    for strike in ["0.02", "0.06"] {
                        let identifier =
                            format!("Swaption_USD_SOFR_{expiry}_{tenor}_Absolute_{strike}_Black");
                        quotes.add_quote(Quote::new(
                            identifier.parse()?,
                            QuoteLevels::with_mid(if expiry == "1Y" { 0.20 } else { 0.30 }),
                        ));
                        identifiers.push(identifier);
                    }
                }
            }
            let time_grid = TimeGridConfiguration::new(24, 4096, DayCounter::Actual365);
            let implementation = if use_fourier {
                PricerImplementation::BackwardFourierEvolution {
                    time_grid,
                    fourier_grid: FourierGridConfiguration::new(256, 10.0),
                }
            } else {
                PricerImplementation::TreeBased { time_grid }
            };
            let calibration = ModelCalibrationConfiguration::new(CalibrationSource::Cube {
                market_index: MarketIndex::SOFR,
            })
            .with_calibration_basket(
                CalibrationBasket::all()
                    .with_expiries(vec![
                        Period::new(1, TimeUnit::Years),
                        Period::new(2, TimeUnit::Years),
                    ])
                    .with_tenors(vec![Period::new(5, TimeUnit::Years)])
                    .with_strike(Strike::Atm),
            );
            let configuration = PricerConfiguration::new(
                InstrumentType::SteppedCouponBond,
                implementation,
                vec![PricingModelConfiguration::new(
                    MarketIndex::SOFR,
                    ModelConfiguration::HullWhite {
                        alpha: ALPHA,
                        parameter_source: ParameterSource::Calibrated(calibration),
                    },
                )],
            );
            let mut context = PricingContext::new()
                .with_quote_store(quotes)
                .with_constructed_elements(base.market_data.constructed_elements().clone())
                .with_volatility_cube_configurations(vec![VolatilityCubeConfiguration::new(
                    MarketIndex::SOFR,
                    VolatilityType::Black,
                    SmileType::Strike,
                    identifiers,
                )])
                .with_pricer_configurations(vec![configuration]);
            context.initialize()?;
            let trade = bond(
                evaluation,
                evaluation + 4 * 365,
                0.05,
                PaymentStructure::Bullet,
                vec![CallScheduleEntry::new(evaluation + 3 * 365, 100.0)],
            );
            let results = if use_fourier {
                fourier_pricer().evaluate(
                    &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
                    &[Request::Value, Request::Sensitivities],
                    &context,
                )?
            } else {
                tree_pricer().evaluate(
                    &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
                    &[Request::Value, Request::Sensitivities],
                    &context,
                )?
            };
            assert!(results
                .price()
                .is_some_and(|price| price.is_finite() && price > 0.0));
            let risk = results
                .sensitivities()
                .ok_or_else(|| QSError::ValueNotSetErr("Cube bond sensitivities".into()))?;
            assert!(
                risk.instrument_keys()
                    .iter()
                    .zip(risk.exposure())
                    .any(|(label, exposure)| label.starts_with("Swaption_")
                        && exposure.abs() > 1.0e-4)
            );
        }
        Tape::rewind_to_init_fwd();
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn discount_policy_controls_curve_requests_and_bond_values() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let maturity = Date::new(2027, 1, 1);
        let selected_index = MarketIndex::Other("USD-TREASURY".into());
        let definition = RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        );
        let mut elements = ConstructedElementStore::default();
        for (index, rate) in [(MarketIndex::SOFR, RATE), (selected_index.clone(), 0.08)] {
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(
                    index,
                    Rc::new(RefCell::new(FlatForwardTermStructure::new(
                        evaluation,
                        DualFwd::from(rate),
                        definition,
                    ))),
                ),
            );
        }
        let market = TestProvider {
            evaluation_date: evaluation,
            market_data: MarketData::new(HashMap::new(), elements),
        };
        let selected_policy = || -> Box<dyn DiscountPolicy> {
            Box::new(
                FixedIncomeDiscountPolicy::new(false)
                    .with_risk_free_index(Currency::USD, selected_index.clone()),
            )
        };
        let expected = FACE * (-0.08_f64 * 2.0).exp();
        let straight = fixed_bond(evaluation, maturity, 0.0);
        let stepped = bond(evaluation, maturity, 0.0, PaymentStructure::Zero, vec![]);
        let mut fixed = BondPricer::new();
        let mut tree = TreeBasedBondPricer::new();
        let mut fourier = BackwardFourierEvolutionBondPricer::new();
        assert!(fixed
            .evaluate(
                &straight
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &[Request::Value],
                &market
            )
            .is_err());
        assert!(tree
            .evaluate(
                &stepped
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &[Request::Value],
                &market
            )
            .is_err());
        assert!(fourier
            .evaluate(
                &stepped
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &[Request::Value],
                &market
            )
            .is_err());
        fixed.set_discount_policy(selected_policy());
        tree.set_discount_policy(selected_policy());
        fourier.set_discount_policy(selected_policy());
        for request in [
            fixed.market_data_request(&straight),
            tree.market_data_request(&stepped),
            fourier.market_data_request(&stepped),
        ] {
            let request =
                request.ok_or_else(|| QSError::NotFoundErr("bond curve request".into()))?;
            assert!(
                matches!(request.constructed_elements_request().and_then(|elements| elements.first()),
                Some(ConstructedElementRequest::DiscountCurve { market_index }) if market_index == &selected_index)
            );
        }
        for price in [
            fixed_value(
                &fixed,
                &straight
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &market,
            ),
            value(
                &tree,
                &stepped
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &market,
            ),
            fourier_value(
                &fourier,
                &stepped.with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                &market,
            ),
        ] {
            assert!(
                (price - expected).abs() < 1.0e-10,
                "policy-selected {price} vs analytic {expected}"
            );
        }
        assert!(
            expected
                < fixed_value(
                    &fixed_pricer(),
                    &straight.with_pricing_anchor(BondPricingAnchor::ZSpread(0.0)),
                    &market
                )
        );
        Ok(())
    }

    #[test]
    fn callable_zero_coupon_matches_hull_white_bond_option_formula() {
        let (lattice, benchmark) = callable_zero_coupon_value(192);
        assert!(
            (lattice - benchmark).abs() < 0.08,
            "{lattice} vs {benchmark}"
        );
    }

    #[test]
    fn interior_window_exercise_reduces_price_in_both_engines() {
        let evaluation = Date::new(2025, 1, 1);
        let first = Date::new(2026, 1, 1);
        let last = Date::new(2028, 1, 1);
        let maturity = Date::new(2030, 1, 1);
        let endpoints = bond(
            evaluation,
            maturity,
            0.045,
            PaymentStructure::Bullet,
            vec![
                CallScheduleEntry::new(first, 100.0),
                CallScheduleEntry::new(last, 100.0),
            ],
        );
        let window = bond(
            evaluation,
            maturity,
            0.045,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::window(first, last, 100.0)],
        );
        let (tree_market, _) = provider(evaluation, 96);
        let (fourier_market, _) = fourier_provider(evaluation, 96);
        let tree = tree_pricer();
        let fourier = fourier_pricer();
        let tree_window = value(
            &tree,
            &window
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &tree_market,
        );
        let tree_endpoints = value(
            &tree,
            &endpoints
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &tree_market,
        );
        let fourier_window = fourier_value(
            &fourier,
            &window.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &fourier_market,
        );
        let fourier_endpoints = fourier_value(
            &fourier,
            &endpoints.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &fourier_market,
        );
        assert!(
            tree_window < tree_endpoints - 1.0e-4,
            "{tree_window} vs {tree_endpoints}"
        );
        assert!(
            fourier_window < fourier_endpoints - 1.0e-4,
            "{fourier_window} vs {fourier_endpoints}"
        );
        assert!(
            (tree_window - fourier_window).abs() < 0.15,
            "window tree {tree_window} vs Fourier {fourier_window}"
        );
    }

    #[test]
    fn tree_and_backward_fourier_prices_agree() {
        let evaluation_date = Date::new(2025, 1, 1);
        let trade = bond(
            evaluation_date,
            Date::new(2032, 1, 1),
            0.045,
            PaymentStructure::Bullet,
            vec![
                CallScheduleEntry::new(Date::new(2028, 1, 1), 101.0),
                CallScheduleEntry::new(Date::new(2030, 1, 1), 100.0),
            ],
        );
        let (tree_provider, _) = provider(evaluation_date, 96);
        let (fourier_provider, _) = fourier_provider(evaluation_date, 96);
        let tree = value(
            &tree_pricer(),
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(0.006)),
            &tree_provider,
        );
        let fourier = fourier_value(
            &fourier_pricer(),
            &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.006)),
            &fourier_provider,
        );
        assert!((tree - fourier).abs() < 0.12, "{tree} vs {fourier}");
    }

    #[test]
    fn engines_agree_for_positional_mixed_coupons_and_amortization() {
        let evaluation_date = Date::new(2025, 1, 1);
        let instrument = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("MIXED-STEP-BOND".into())
            .with_start_date(evaluation_date)
            .with_maturity_date(Date::new(2031, 1, 1))
            .with_units(FACE)
            .with_notional(FACE)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Semiannual)
            .with_coupons(vec![
                CashflowType::fixed(0.045),
                CashflowType::floating(MarketIndex::SOFR, 0.006, Some(0.0), Some(0.06)),
            ])
            .with_notional_percentages(vec![1.0, 0.9, 0.75])
            .with_call_schedule(vec![
                CallScheduleEntry::new(Date::new(2028, 1, 1), 101.0),
                CallScheduleEntry::new(Date::new(2029, 1, 1), 100.0),
            ])
            .build()
            .unwrap_or_else(|error| panic!("mixed stepped bond: {error}"));
        let trade = SteppedCouponBondTrade::new(
            instrument,
            evaluation_date,
            FACE,
            crate::core::trade::Side::LongReceive,
            BondPricingAnchor::OAS(0.0),
        );
        let (tree_provider, _) = provider(evaluation_date, 96);
        let (fourier_provider, _) = fourier_provider(evaluation_date, 96);
        let tree = value(
            &tree_pricer(),
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(0.0075)),
            &tree_provider,
        );
        let fourier = fourier_value(
            &fourier_pricer(),
            &trade.with_pricing_anchor(BondPricingAnchor::OAS(0.0075)),
            &fourier_provider,
        );
        assert!(tree.is_finite() && fourier.is_finite());
        assert!((tree - fourier).abs() < 0.15, "{tree} vs {fourier}");
    }

    #[test]
    fn stepped_coupon_bond_rejects_yield_analytics() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 24);
        let trade = bond(
            evaluation_date,
            Date::new(2030, 1, 1),
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0)],
        );
        let pricer = tree_pricer();
        for request in [Request::YieldToMaturity, Request::ModifiedDuration] {
            let Err(error) = pricer.evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
                &[request],
                &provider,
            ) else {
                panic!("stepped bond yield analytics must be rejected");
            };
            assert!(error
                .to_string()
                .contains("not defined for SteppedCouponBond"));
        }
    }

    #[test]
    fn lattice_ladder_converges_towards_closed_form() {
        let (coarse, benchmark) = callable_zero_coupon_value(12);
        let (fine, _) = callable_zero_coupon_value(192);
        assert!((fine - benchmark).abs() < (coarse - benchmark).abs());
        assert!((fine - benchmark).abs() < 0.15);
    }

    #[test]
    fn call_boundaries_and_extreme_oas_are_monotone() {
        let evaluation_date = Date::new(2025, 1, 1);
        let call_date = Date::new(2027, 1, 1);
        let maturity_date = Date::new(2030, 1, 1);
        let (provider, _) = provider(evaluation_date, 48);
        let low_call = bond(
            evaluation_date,
            maturity_date,
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(call_date, 70.0)],
        );
        let high_call = bond(
            evaluation_date,
            maturity_date,
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(call_date, 200.0)],
        );
        let low_call_value = value(
            &tree_pricer(),
            &low_call.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &provider,
        );
        let high_call_value = value(
            &tree_pricer(),
            &high_call
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
            &provider,
        );
        assert!(low_call_value < high_call_value);

        let negative = value(
            &tree_pricer(),
            &high_call
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(-0.05)),
            &provider,
        );
        let zero = high_call_value;
        let positive = value(
            &tree_pricer(),
            &high_call.with_pricing_anchor(BondPricingAnchor::OAS(0.20)),
            &provider,
        );
        assert!(negative.is_finite() && zero.is_finite() && positive.is_finite());
        assert!(negative > zero && zero > positive);
    }

    #[test]
    fn oas_price_anchor_round_trip_and_sensitivity_share_the_risk_table() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 24);
        let trade = bond(
            evaluation_date,
            Date::new(2030, 1, 1),
            0.05,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 100.0)],
        );
        let oas = 0.0075;
        let direct = tree_pricer();
        let results = direct
            .evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::OAS(oas)),
                &[Request::Value, Request::OAS, Request::Sensitivities],
                &provider,
            )
            .unwrap_or_else(|error| panic!("callable bond: {error}"));
        assert_eq!(results.oas(), Some(oas));
        let direct_value = results.price().unwrap_or_default();
        let implied = tree_pricer();
        let implied_results = implied
            .evaluate(
                &trade
                    .clone()
                    .with_pricing_anchor(BondPricingAnchor::DirtyPrice(direct_value)),
                &[Request::OAS],
                &provider,
            )
            .unwrap_or_else(|error| panic!("OAS solve: {error}"));
        assert!((implied_results.oas().unwrap_or_default() - oas).abs() < 1.0e-9);

        let risk = results
            .sensitivities()
            .unwrap_or_else(|| panic!("callable sensitivity result"));
        let oas_risk = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .find_map(|(label, exposure)| (label == "TEST-BOND.OAS").then_some(*exposure))
            .unwrap_or_else(|| panic!("OAS sensitivity"));
        let bump = 1.0e-5;
        let finite_difference = (value(
            &tree_pricer(),
            &trade
                .clone()
                .with_pricing_anchor(BondPricingAnchor::OAS(oas + bump)),
            &provider,
        ) - value(
            &tree_pricer(),
            &trade.with_pricing_anchor(BondPricingAnchor::OAS(oas - bump)),
            &provider,
        )) / (2.0 * bump);
        assert!((oas_risk - finite_difference).abs() < 0.02);
    }

    #[test]
    fn rejects_mismatched_spread_family_and_bad_call_terms() {
        let evaluation_date = Date::new(2025, 1, 1);
        let (provider, _) = provider(evaluation_date, 24);
        let straight = bond(
            evaluation_date,
            Date::new(2028, 1, 1),
            0.04,
            PaymentStructure::Bullet,
            Vec::new(),
        );
        assert!(tree_pricer()
            .evaluate(
                &straight.with_pricing_anchor(BondPricingAnchor::OAS(0.0)),
                &[Request::Value],
                &provider
            )
            .is_err());
        assert!(CallScheduleEntry::new(Date::new(2026, 1, 1), 0.0)
            .validate()
            .is_err());
    }

    #[test]
    fn callable_pricers_reuse_trade_terms_and_apply_side_once() -> Result<()> {
        let evaluation = Date::new(2025, 1, 1);
        let long = bond(
            evaluation,
            Date::new(2027, 1, 1),
            0.04,
            PaymentStructure::Bullet,
            vec![CallScheduleEntry::new(Date::new(2026, 1, 1), 99.0)],
        )
        .with_pricing_anchor(BondPricingAnchor::OAS(0.003));
        let short = SteppedCouponBondTrade::new(
            long.instrument().clone(),
            evaluation,
            FACE,
            Side::PayShort,
            long.pricing_anchor(),
        );
        let scalar: SteppedCouponBondTrade<f64> = short.clone().into();
        assert!(scalar
            .into_contingent_claims("SHORT-CALLABLE")?
            .iter()
            .all(|claim| claim.side() == Side::PayShort));
        let converted: SteppedCouponBondTrade<DualFwd> = scalar.into();
        assert_eq!(converted.pricing_anchor(), short.pricing_anchor());
        for use_fourier in [false, true] {
            let (provider, _) = provider_with_method(evaluation, 12, use_fourier);
            let tree = tree_pricer();
            let fourier = fourier_pricer();
            let price = |trade: &SteppedCouponBondTrade<DualFwd>| -> Result<f64> {
                let result = if use_fourier {
                    fourier.evaluate(trade, &[Request::Value], &provider)?
                } else {
                    tree.evaluate(trade, &[Request::Value], &provider)?
                };
                result
                    .price()
                    .ok_or_else(|| QSError::ValueNotSetErr("Bond value".into()))
            };
            let long_value = price(&long)?;
            assert!((long_value + price(&converted)?).abs() < 1.0e-9);
            assert!(
                (price(
                    &long
                        .clone()
                        .with_pricing_anchor(BondPricingAnchor::DirtyPrice(long_value))
                )? - long_value)
                    .abs()
                    < 1.0e-8
            );
        }
        Ok(())
    }
}
