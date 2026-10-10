//! Backward Fourier evolution valuation of callable bond trades.
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
    core::pricerconfiguration::FourierGridConfiguration,
    math::{
        fft::{fft, Complex},
        ifft::ifft,
    },
};
use crate::{
    core::pricerconfiguration::PricerImplementation,
    instruments::fixedincome::steppedcouponbond::SteppedCouponBondTrade,
    models::hullwhite::parameters::transition_variances, utils::errors::QSError,
};
/// Callable bond pricer with backward Fourier evolution.
#[derive(Default)]
pub struct BackwardFourierEvolutionBondPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}
impl BackwardFourierEvolutionBondPricer {
    /// Creates a reusable engine; quote and settlement terms belong to each trade.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
/// Applies the Gaussian transition semigroup to values on a periodic uniform
/// grid by multiplying their Fourier coefficients by the Gaussian
/// characteristic function.
#[allow(clippy::cast_precision_loss)]
fn gaussian_fourier_evolution<T: Scalar>(
    values: &[T],
    transition_variance: T,
    spacing: T,
) -> Result<Vec<T>> {
    let mut spectrum = values
        .iter()
        .copied()
        .map(|value| Complex::new(value, T::zero()))
        .collect::<Vec<_>>();
    fft(&mut spectrum)?;
    let points = spectrum.len();
    let domain = spacing.mul_val(T::scalar(points as f64));
    for (index, coefficient) in spectrum.iter_mut().enumerate() {
        let signed_index = if index <= points / 2 {
            index as f64
        } else {
            index as f64 - points as f64
        };
        let wave_number = T::scalar(2.0 * std::f64::consts::PI * signed_index).div_val(domain);
        let multiplier = transition_variance
            .mul_val(wave_number)
            .mul_val(wave_number)
            .mul_val(T::scalar(-0.5))
            .exp();
        *coefficient = *coefficient * multiplier;
    }
    ifft(&mut spectrum)?;
    Ok(spectrum.into_iter().map(|value| value.re).collect())
}

/// Linearly interpolates a scalar value on an ordered uniform grid, clamping
/// only points that fall outside the configured truncation domain.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn interpolate_uniform<T: Scalar>(values: &[T], lower: T, spacing: T, state: T) -> T {
    let position = state.sub_val(lower).div_val(spacing);
    if position.value() <= 0.0 {
        return values[0];
    }
    let upper_index = values.len() - 1;
    if position.value() >= upper_index as f64 {
        return values[upper_index];
    }
    let left_index = position.value().floor() as usize;
    let weight = position.sub_val(T::scalar(left_index as f64));
    values[left_index]
        .mul_val(T::one().sub_val(weight))
        .add_val(values[left_index + 1].mul_val(weight))
}

/// Fits the deterministic discrete Hull-White shift to all curve discount
/// factors on the evolution grid.
///
/// With `I_n = dt * sum_{i < n} x_i`, the Gaussian identity
/// `E[exp(-I_n)] = exp(0.5 Var[I_n])` gives
/// `sum(phi_i dt) = -log(P(0,t_n)) + 0.5 Var[I_n]`.
fn fourier_curve_shifts<T: Scalar>(
    mut discount_factor: impl FnMut(f64) -> Result<T>,
    time_step: f64,
    alpha: f64,
    variances: &[T],
) -> Result<Vec<T>> {
    let decay = (-alpha * time_step).exp();
    let mut state_variance = T::zero();
    let mut integral_variance = T::zero();
    let mut integral_state_covariance = T::zero();
    let mut previous_cumulative_shift = T::zero();
    let mut shifts = Vec::with_capacity(variances.len());

    for (step, &transition_variance) in variances.iter().enumerate() {
        let next_integral_variance = integral_variance
            .add_val(integral_state_covariance.mul_val(T::scalar(2.0 * time_step)))
            .add_val(state_variance.mul_val(T::scalar(time_step * time_step)));
        let next_covariance = integral_state_covariance
            .add_val(state_variance.mul_val(T::scalar(time_step)))
            .mul_val(T::scalar(decay));
        let next_state_variance = state_variance
            .mul_val(T::scalar(decay * decay))
            .add_val(transition_variance);
        let next_time =
            time_step
                * f64::from(u32::try_from(step + 1).map_err(|_| {
                    QSError::InvalidValueErr("Fourier time index exceeds u32".into())
                })?);
        let discount = discount_factor(next_time)?;
        if discount.value() <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Backward Fourier evolution requires positive discount factors".into(),
            ));
        }
        let cumulative_shift = discount
            .ln()
            .mul_val(T::scalar(-1.0))
            .add_val(next_integral_variance.mul_val(T::scalar(0.5)));
        shifts.push(
            cumulative_shift
                .sub_val(previous_cumulative_shift)
                .div_val(T::scalar(time_step)),
        );
        previous_cumulative_shift = cumulative_shift;
        state_variance = next_state_variance;
        integral_variance = next_integral_variance;
        integral_state_covariance = next_covariance;
    }
    Ok(shifts)
}

/// Prices an issuer-call bond by backward Fourier evolution under the
/// curve-fitted one-factor Hull-White model.
///
/// The Gaussian expectation is evaluated spectrally. The mean-reversion map
/// `x -> exp(-alpha dt) x` is then evaluated by linear interpolation before
/// discounting and application of the issuer-call obstacle.
#[allow(clippy::cast_precision_loss)]
pub(super) fn backward_fourier_value<T: Scalar>(
    discount_factor: impl FnMut(f64) -> Result<T>,
    terms: &BondEventGrid<T>,
    alpha: f64,
    sigma_schedule: &[(f64, T)],
    spread: T,
    grid: FourierGridConfiguration,
) -> Result<T> {
    grid.validate()?;
    let time_step =
        terms.maturity_time
            / f64::from(u32::try_from(terms.steps).map_err(|_| {
                QSError::InvalidValueErr("Fourier time-step count exceeds u32".into())
            })?);
    let decay = (-alpha * time_step).exp();
    let variances = transition_variances(alpha, sigma_schedule, terms.steps, time_step)?;
    let mut state_variance = T::zero();
    let mut maximum_variance = T::zero();
    let mut maximum_innovation = T::zero();
    for &variance in &variances {
        state_variance = state_variance
            .mul_val(T::scalar(decay * decay))
            .add_val(variance);
        maximum_variance = maximum_variance.max_val(state_variance);
        maximum_innovation = maximum_innovation.max_val(variance);
    }
    let half_width = maximum_variance
        .sqrt()
        .mul_val(T::scalar(grid.standard_deviations()))
        .max_val(maximum_innovation.sqrt().mul_val(T::scalar(6.0)))
        .max_val(T::scalar(1.0e-4));
    let points = grid.points();
    let spacing = half_width.mul_val(T::scalar(2.0 / points as f64));
    let lower = half_width.neg_val();
    let states = (0..points)
        .map(|index| spacing.mul_val(T::scalar(index as f64)).add_val(lower))
        .collect::<Vec<_>>();
    let shifts = fourier_curve_shifts(discount_factor, time_step, alpha, &variances)?;

    let mut values = vec![terms.cashflows[terms.steps]; points];
    for step in (0..terms.steps).rev() {
        let evolved = gaussian_fourier_evolution(&values, variances[step], spacing)?;
        let event = terms.calls[step];
        let cashflow = terms.cashflows[step];
        let mut previous = Vec::with_capacity(points);
        for state in &states {
            let expectation =
                interpolate_uniform(&evolved, lower, spacing, state.mul_val(T::scalar(decay)));
            let short_rate = shifts[step].add_val(*state).add_val(spread);
            let mut value = expectation.mul_val(short_rate.mul_val(T::scalar(-time_step)).exp());
            if let Some(call_price) = event.call_price {
                value = value.min_val(T::scalar(call_price));
            }
            previous.push(value.add_val(cashflow));
        }
        values = previous;
    }
    Ok(interpolate_uniform(&values, lower, spacing, T::zero()))
}

impl BondPricingEngine for BackwardFourierEvolutionBondPricer {
    type Model<T: Scalar> = (Option<CallableBondModel<T>>, FourierGridConfiguration);
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
                (None, FourierGridConfiguration::default()),
            ));
        }
        let configuration = pricer_configuration(trade, context)?;
        let (time_grid, grid) = match configuration.pricer() {
            PricerImplementation::BackwardFourierEvolution { time_grid, fourier_grid } => (*time_grid, *fourier_grid),
            actual => return Err(QSError::InvalidValueErr(format!("BackwardFourierEvolutionBondPricer requires a BackwardFourierEvolution context configuration, got {actual:?}"))),
        };
        let (market, model) =
            prepare_callable_bond(trade, index, context, &configuration, time_grid)?;
        Ok((market, (Some(model), grid)))
    }
    fn ad_model(
        &self,
        model: &Self::Model<f64>,
        market: &MarketData,
    ) -> Result<Self::Model<DualFwd>> {
        Ok((
            model
                .0
                .as_ref()
                .map(|model| model.ad_model(market))
                .transpose()?,
            model.1,
        ))
    }
    fn value<T: Scalar>(
        &self,
        model: &Self::Model<T>,
        valuation: &BondValuation<'_, T>,
        spread: T,
    ) -> Result<T> {
        if let Some(callable) = &model.0 {
            let terms = callable.events(valuation)?;
            backward_fourier_value(
                |time| valuation.curve.discount_factor_from_time(time),
                &terms,
                callable.parameters.alpha,
                &callable.parameters.sigma_schedule,
                spread,
                model.1,
            )
        } else {
            discounted_value(valuation, spread)
        }
    }
}
impl<T: BondTerms> HandleCashflows<T, BondState> for BackwardFourierEvolutionBondPricer {}
impl_bond_pricer!(
    BackwardFourierEvolutionBondPricer,
    SteppedCouponBondTrade<DualFwd>
);
