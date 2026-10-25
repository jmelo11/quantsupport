use std::collections::HashSet;

use crate::{
    ad::{dual::DualFwd, expr::FloatExt, tape::Tape},
    core::{
        collateral::{DiscountPolicy, Discountable},
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::{
        cashflows::cashflowtype::CashflowType,
        rates::europeanswaption::{EuropeanSwaptionTrade, SwaptionType},
    },
    math::probability::norm_cdf::norm_cdf,
    models::{
        hullwhite::parameters::{hull_white_piecewise_variance, HullWhiteParameters},
        modelconfiguration::{
            GaussianRateModelParameters, GaussianRateParameterSource, ModelConfiguration,
            ParameterSource,
        },
    },
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    time::date::Date,
    utils::errors::{QSError, Result},
    volatility::modelcalibration::CalibrationSource,
};

/// Prices European payer and receiver swaptions in the one-factor Hull-White model.
///
/// The pricer uses Jamshidian decomposition to express the option as a portfolio
/// of zero-coupon bond puts (payer) or calls (receiver). `alpha` is the
/// mean-reversion speed in inverse years. Sigma is absolute rate volatility
/// per square-root year. Volatility can be supplied directly or resolved by
/// the shared Hull-White calibration from a surface or cube.
///
/// Curve inputs remain on the AD tape throughout the decomposition, including
/// the implicit critical-rate solve, so [`Request::Sensitivities`] returns
/// sensitivities to the requested curve and calibration volatility pillars.
///
/// When a [`DiscountPolicy`] selects a curve different from the swap's forward
/// curve, each bond-option component receives the same deterministic
/// payment-date discount-factor ratio adjustment used by the Hull-White cap
/// and caplet pricers.
pub struct ClosedFormHullWhiteSwaptionPricer {
    alpha: f64,
    parameter_source: GaussianRateParameterSource,
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

/// One fixed-leg payment used by the Jamshidian decomposition.
struct JamshidianTerm {
    payment_date: Date,
    coefficient: f64,
    b: f64,
    affine_a: DualFwd,
    forward_df: DualFwd,
}

impl ClosedFormHullWhiteSwaptionPricer {
    /// Creates a Hull-White swaption pricer with constant model parameters.
    #[must_use]
    pub fn new(alpha: f64, sigma: f64) -> Self {
        Self {
            alpha,
            parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(sigma)),
            discount_policy: None,
        }
    }

    /// Builds the pricer from a fixed or calibrated Hull-White assignment.
    /// # Errors
    /// Rejects missing models, incompatible engines and invalid fixed parameters.
    pub fn from_configuration(
        configuration: &crate::core::pricerconfiguration::PricerConfiguration,
        index: &crate::indices::marketindex::MarketIndex,
    ) -> Result<Self> {
        use crate::core::pricerconfiguration::{InstrumentType, PricerImplementation};
        if configuration.instrument_type() != InstrumentType::EuropeanSwaption
            || !matches!(
                configuration.pricer(),
                PricerImplementation::HullWhiteClosedForm
            )
        {
            return Err(QSError::InvalidValueErr(
                "EuropeanSwaption requires HullWhiteClosedForm".into(),
            ));
        }
        let Some(ModelConfiguration::HullWhite {
            alpha,
            parameter_source,
        }) = configuration.model(index)
        else {
            return Err(QSError::InvalidValueErr(format!(
                "EuropeanSwaption requires a HullWhite model for {index}"
            )));
        };
        let pricer = Self {
            alpha: *alpha,
            parameter_source: parameter_source.clone(),
            discount_policy: None,
        };
        pricer.validate_parameters()?;
        Ok(pricer)
    }

    /// Validates positive finite mean reversion and any fixed volatility parameters.
    fn validate_parameters(&self) -> Result<()> {
        if !self.alpha.is_finite() || self.alpha <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Hull-White alpha must be finite and positive".into(),
            ));
        }
        if let ParameterSource::Fixed(parameters) = &self.parameter_source {
            parameters.validate()?;
        }
        Ok(())
    }

    /// Computes `B(t,T) = (1 - exp(-alpha * (T-t))) / alpha`.
    fn b(&self, option_time: f64, bond_time: f64) -> f64 {
        (1.0 - (-self.alpha * (bond_time - option_time)).exp()) / self.alpha
    }

    /// AD-enabled `A(t,T)` for `P(t,T|r) = A(t,T) * exp(-B(t,T) * r)`.
    fn affine_a(
        &self,
        option_time: f64,
        bond_time: f64,
        state_variance: DualFwd,
        curve: &dyn InterestRatesTermStructure<DualFwd>,
    ) -> Result<DualFwd> {
        let b = self.b(option_time, bond_time);
        let option_df = curve.discount_factor_from_time(option_time)?;
        let bond_df = curve.discount_factor_from_time(bond_time)?;

        let h = 1.0 / 365.0;
        let option_df_plus = curve.discount_factor_from_time(option_time + h)?;
        let forward_at_option: DualFwd = (-(option_df_plus / option_df).ln() / h).into();
        let variance_adjustment: DualFwd = (state_variance * (0.5 * b * b)).into();
        let log_a: DualFwd =
            ((bond_df / option_df).ln() + forward_at_option * b - variance_adjustment).into();
        Ok(log_a.exp())
    }

    /// Solves the Jamshidian exercise rate by bracketing and bisection.
    fn find_critical_rate(terms: &[JamshidianTerm]) -> Result<f64> {
        let evaluate = |rate: f64| -> f64 {
            terms
                .iter()
                .map(|term| term.coefficient * term.affine_a.value() * (-term.b * rate).exp())
                .sum()
        };

        let mut lower = -0.5;
        let mut upper = 0.5;
        for _ in 0..60 {
            if evaluate(lower) >= 1.0 {
                break;
            }
            lower *= 2.0;
        }
        for _ in 0..60 {
            if evaluate(upper) <= 1.0 {
                break;
            }
            upper *= 2.0;
        }

        if evaluate(lower) < 1.0 || evaluate(upper) > 1.0 {
            return Err(QSError::UnexpectedErr(
                "Could not bracket the Hull-White critical short rate".into(),
            ));
        }

        for _ in 0..200 {
            let midpoint = f64::midpoint(lower, upper);
            if evaluate(midpoint) > 1.0 {
                lower = midpoint;
            } else {
                upper = midpoint;
            }
        }
        Ok(f64::midpoint(lower, upper))
    }

    /// Adds the implicit curve dependence of the critical rate to the AD graph.
    fn implicit_critical_rate(terms: &[JamshidianTerm], rate: f64) -> Result<DualFwd> {
        let rate = DualFwd::new(rate);
        let mut objective = DualFwd::new(-1.0);
        let mut derivative = DualFwd::zero();

        for term in terms {
            let exponent: DualFwd = (rate * -term.b).into();
            let conditional_bond: DualFwd = (term.affine_a * exponent.exp()).into();
            objective = (objective + conditional_bond * term.coefficient).into();
            derivative = (derivative - conditional_bond * (term.coefficient * term.b)).into();
        }

        if derivative.value().abs() < 1e-14 {
            return Err(QSError::UnexpectedErr(
                "Hull-White critical-rate derivative is zero".into(),
            ));
        }

        // At the converged scalar root, one Newton correction supplies the
        // implicit derivative dr*/dq = -(df/dq) / (df/dr).
        Ok((rate - objective / derivative).into())
    }

    /// Prices the zero-coupon bond option used in payer or receiver swaption decomposition.
    fn bond_option(
        option_type: SwaptionType,
        bond_volatility: DualFwd,
        strike: DualFwd,
        option_df: DualFwd,
        bond_df: DualFwd,
    ) -> DualFwd {
        let strike_value: DualFwd = (strike * option_df).into();
        if bond_volatility.value() <= 1e-12 {
            let intrinsic: DualFwd = match option_type {
                SwaptionType::Payer => (strike_value - bond_df).into(),
                SwaptionType::Receiver => (bond_df - strike_value).into(),
            };
            return if intrinsic.value() > 0.0 {
                intrinsic
            } else {
                DualFwd::zero()
            };
        }

        let d1: DualFwd = (((bond_df / strike_value).ln()
            + bond_volatility * bond_volatility * 0.5)
            / bond_volatility)
            .into();
        let d2: DualFwd = (d1 - bond_volatility).into();

        match option_type {
            SwaptionType::Payer => {
                let negative_d1: DualFwd = (-d1).into();
                let negative_d2: DualFwd = (-d2).into();
                (strike_value * norm_cdf(negative_d2) - bond_df * norm_cdf(negative_d1)).into()
            }
            SwaptionType::Receiver => (bond_df * norm_cdf(d1) - strike_value * norm_cdf(d2)).into(),
        }
    }

    /// Prices the swaption through Jamshidian decomposition using the selected curves.
    #[allow(clippy::too_many_lines)]
    fn swaption_value(
        &self,
        trade: &EuropeanSwaptionTrade<DualFwd>,
        state: &HullWhiteSwaptionState,
        parameters: &HullWhiteParameters<DualFwd>,
    ) -> Result<DualFwd> {
        let swaption = trade.instrument();
        let forward_index = swaption.market_index();
        let discount_index = if let Some(policy) = &self.discount_policy {
            policy.accept(swaption)?
        } else {
            forward_index.clone()
        };
        let forward_curve = state.get_discount_curve_element(&forward_index)?.curve();
        let reference_date = forward_curve.reference_date();
        let rate_definition = forward_index.rate_index_details()?.rate_definition();
        let day_counter = parameters
            .day_counter()
            .or_else(|| forward_curve.day_counter())
            .unwrap_or_else(|| rate_definition.day_counter());
        let option_time = day_counter.year_fraction(reference_date, swaption.expiry_date());
        if option_time < 0.0 {
            return Err(QSError::InvalidValueErr(format!(
                "Swaption expiry {} is before curve reference date {reference_date}",
                swaption.expiry_date()
            )));
        }
        let state_variance = hull_white_piecewise_variance(
            parameters.alpha(),
            parameters.sigma_schedule(),
            0.0,
            option_time,
        );
        // Avoid a singular sqrt derivative for an expired or zero-volatility option.
        let state_std_dev = if state_variance.value() > 0.0 {
            state_variance.sqrt()
        } else {
            DualFwd::zero()
        };

        let fixed_coupons: Vec<_> = swaption
            .underlying()
            .fixed_leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::FixedRateCoupon(coupon) => Some(coupon),
                _ => None,
            })
            .collect();
        if fixed_coupons.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Swaption underlying fixed leg has no coupons".into(),
            ));
        }

        let last_coupon = fixed_coupons.len() - 1;
        let mut terms = Vec::with_capacity(fixed_coupons.len());
        for (index, coupon) in fixed_coupons.iter().enumerate() {
            let payment_date = coupon.payment_date();
            if payment_date <= swaption.expiry_date() {
                return Err(QSError::InvalidValueErr(format!(
                    "Swaption fixed-leg payment {payment_date} must be after expiry {}",
                    swaption.expiry_date()
                )));
            }
            let accrual = coupon
                .rate()
                .day_counter()
                .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date());
            let coefficient = if index == last_coupon {
                accrual.mul_add(swaption.strike(), 1.0)
            } else {
                accrual * swaption.strike()
            };
            let bond_time = day_counter.year_fraction(reference_date, payment_date);
            terms.push(JamshidianTerm {
                payment_date,
                coefficient,
                b: self.b(option_time, bond_time),
                affine_a: self.affine_a(option_time, bond_time, state_variance, &*forward_curve)?,
                forward_df: forward_curve.discount_factor(payment_date)?,
            });
        }

        let scalar_rate = Self::find_critical_rate(&terms)?;
        let critical_rate = Self::implicit_critical_rate(&terms, scalar_rate)?;
        let option_df = forward_curve.discount_factor(swaption.expiry_date())?;
        let dual_curve = discount_index != forward_index;
        let discount_curve = if dual_curve {
            Some(state.get_discount_curve_element(&discount_index)?.curve())
        } else {
            None
        };

        let mut value = DualFwd::zero();
        for term in &terms {
            let exponent: DualFwd = (critical_rate * -term.b).into();
            let bond_strike: DualFwd = (term.affine_a * exponent.exp()).into();
            let mut component = Self::bond_option(
                swaption.underlying_type(),
                (state_std_dev * term.b).into(),
                bond_strike,
                option_df,
                term.forward_df,
            );

            if let Some(curve) = &discount_curve {
                let discount_df = curve.discount_factor(term.payment_date)?;
                component = (component * discount_df / term.forward_df).into();
            }
            value = (value + component * term.coefficient).into();
        }

        Ok((value * trade.notional() * trade.side().sign()).into())
    }
}

#[derive(Default)]
struct HullWhiteSwaptionState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for HullWhiteSwaptionState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

impl HandleValue<EuropeanSwaptionTrade<DualFwd>, HullWhiteSwaptionState>
    for ClosedFormHullWhiteSwaptionPricer
{
    fn handle_value(
        &self,
        trade: &EuropeanSwaptionTrade<DualFwd>,
        state: &mut HullWhiteSwaptionState,
    ) -> Result<f64> {
        self.validate_parameters()?;
        let market = state
            .market_data
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Swaption market data".into()))?;
        let mut parameters = HullWhiteParameters::new(ModelConfiguration::HullWhite {
            alpha: self.alpha,
            parameter_source: self.parameter_source.clone(),
        })?;
        parameters.resolve_calibration(market, &trade.instrument().market_index())?;
        Tape::start_recording_fwd();
        let value: Result<DualFwd> = (|| {
            Tape::set_mark_fwd();
            state.put_pillars_on_tape()?;
            let market = state
                .market_data
                .as_mut()
                .ok_or_else(|| QSError::ValueNotSetErr("Swaption market data".into()))?;
            for cube in market
                .constructed_elements_mut()
                .volatility_cubes_mut()
                .values_mut()
            {
                cube.cube_mut().put_pillars_on_tape();
            }
            let parameters = parameters.ad_parameters(market)?;
            let value = self.swaption_value(trade, state, &parameters)?;
            let source = Discountable::currency(trade.instrument());
            if let Some(target) = state
                .get_market_data_reponse()
                .and_then(MarketData::base_currency)
                .filter(|target| *target != source)
            {
                Ok((value * state.get_exchange_rate(source, target)?).into())
            } else {
                Ok(value)
            }
        })();
        Tape::stop_recording_fwd();

        let value = value?;
        state.value = Some(value);
        Ok(value.value())
    }
}

impl HandleSensitivities<EuropeanSwaptionTrade<DualFwd>, HullWhiteSwaptionState>
    for ClosedFormHullWhiteSwaptionPricer
{
    fn handle_sensitivities(
        &self,
        trade: &EuropeanSwaptionTrade<DualFwd>,
        state: &mut HullWhiteSwaptionState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(value) = state.value {
            value
        } else {
            let _ = self.handle_value(trade, state)?;
            state.value.ok_or_else(|| {
                QSError::UnexpectedErr(
                    "State does not contain price after value computation".into(),
                )
            })?
        };
        value.backward_to_mark()?;

        let mut identifiers = Vec::new();
        let mut exposures = Vec::new();
        let elements = state
            .market_data
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Swaption market data".into()))?
            .constructed_elements();
        for curve in elements.discount_curves().values() {
            for (label, pillar) in curve.curve().pillars().into_iter().flatten() {
                identifiers.push(label);
                exposures.push(pillar.adjoint()?.value());
            }
        }
        for surface in elements.volatility_surfaces().values() {
            for (label, pillar) in surface.surface().pillars().into_iter().flatten() {
                identifiers.push(label);
                exposures.push(pillar.adjoint()?.value());
            }
        }
        for cube in elements.volatility_cubes().values() {
            for (label, pillar) in cube.cube().pillars().into_iter().flatten() {
                identifiers.push(label);
                exposures.push(pillar.adjoint()?.value());
            }
        }

        for (label, exposure) in state.get_fx_sensitivities()? {
            identifiers.push(label);
            exposures.push(exposure);
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&identifiers)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for ClosedFormHullWhiteSwaptionPricer {
    type Item = EuropeanSwaptionTrade<DualFwd>;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        context: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let market_data_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;
        let mut state = HullWhiteSwaptionState {
            value: None,
            market_data: Some(context.handle_request(&market_data_request)?),
        };
        let wants_value = requests
            .iter()
            .any(|request| matches!(request, Request::Value));
        let wants_sensitivities = requests
            .iter()
            .any(|request| matches!(request, Request::Sensitivities));

        let mut results =
            EvaluationResults::new(context.evaluation_date(), trade.instrument().identifier());
        if wants_value || wants_sensitivities {
            let value = self.handle_value(trade, &mut state)?;
            if wants_value {
                results = results.with_price(value);
            }
        }
        if wants_sensitivities {
            results = results.with_sensitivities(self.handle_sensitivities(trade, &mut state)?);
        }
        Ok(results)
    }

    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest> {
        let forward_index = trade.instrument().market_index();
        let mut seen = HashSet::new();
        seen.insert(forward_index.clone());
        let mut elements = vec![ConstructedElementRequest::DiscountCurve {
            market_index: forward_index,
        }];

        if let Some(policy) = &self.discount_policy {
            for discount_index in policy.discount_indices() {
                if seen.insert(discount_index.clone()) {
                    elements.push(ConstructedElementRequest::DiscountCurve {
                        market_index: discount_index,
                    });
                }
            }
        }

        let mut request = MarketDataRequest::default();
        if let ParameterSource::Calibrated(calibration) = &self.parameter_source {
            let (index, volatility) = match calibration.source() {
                CalibrationSource::Surface { market_index } => (
                    market_index,
                    ConstructedElementRequest::VolatilitySurface {
                        market_index: market_index.clone(),
                    },
                ),
                CalibrationSource::Cube { market_index } => (
                    market_index,
                    ConstructedElementRequest::VolatilityCube {
                        market_index: market_index.clone(),
                    },
                ),
            };
            elements.push(volatility);
            if seen.insert(index.clone()) {
                elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: index.clone(),
                });
            }
            request = request.with_calibration_request(calibration.clone());
        }
        request = request.with_constructed_elements_request(elements);
        let mut fx = request.fx_request().cloned().unwrap_or_default();
        fx.push(FxRequest::single(Discountable::currency(
            trade.instrument(),
        )));
        request = request.with_fx_request(fx);
        Some(request)
    }

    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use crate::{
        ad::dual::DualFwd,
        core::{
            collateral::SingleCurveCSADiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
            },
            pricer::Pricer,
            request::Request,
            trade::{Side, Trade},
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::{
            cashflows::cashflowtype::CashflowType,
            rates::{
                europeanswaption::{EuropeanSwaptionTrade, SwaptionType},
                makeeuropeanswaption::MakeSwaption,
            },
        },
        models::hullwhite::hullwhitemodel::HullWhite,
        pricers::rates::closedformhullwhiteswaptionpricer::ClosedFormHullWhiteSwaptionPricer,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
        },
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        utils::errors::Result,
    };

    #[test]
    fn configuration_accepts_shared_fixed_and_calibrated_models() -> Result<()> {
        use crate::prelude::PricerConfiguration;
        let parse = |model: serde_json::Value| -> Result<PricerConfiguration> {
            serde_json::from_value(serde_json::json!({"instrument_type":"EuropeanSwaption","pricer":"HullWhiteClosedForm",
                "models":[{"market_index":"SOFR","model":model}]})).map_err(|error|crate::prelude::QSError::InvalidValueErr(error.to_string()))
        };
        let fixed = parse(
            serde_json::json!({"HullWhite":{"alpha":0.1,"parameter_source":{"Fixed":{"sigma":0.01}}}}),
        )?;
        assert!(
            ClosedFormHullWhiteSwaptionPricer::from_configuration(&fixed, &MarketIndex::SOFR)
                .is_ok()
        );
        assert!(
            ClosedFormHullWhiteSwaptionPricer::from_configuration(&fixed, &MarketIndex::SONIA)
                .is_err()
        );
        let calibrated = parse(
            serde_json::json!({"HullWhite":{"alpha":0.1,"parameter_source":{"Calibrated":{
            "source":{"Surface":{"market_index":"SOFR"}},"calibration_basket":{"strike":"Atm"}}}}}),
        )?;
        assert!(ClosedFormHullWhiteSwaptionPricer::from_configuration(
            &calibrated,
            &MarketIndex::SOFR
        )
        .is_ok());
        Ok(())
    }

    const ALPHA: f64 = 0.1;
    const SIGMA: f64 = 0.01;
    const RATE: f64 = 0.04;
    const STRIKE: f64 = 0.04;
    const NOTIONAL: f64 = 1_000_000.0;

    /// Builds a surface- or cube-calibrated swaption market with an optional quote bump.
    fn calibrated_context(
        cube: bool,
        bump: Option<(&str, f64)>,
    ) -> Result<(
        crate::prelude::PricingContext,
        crate::prelude::PricerConfiguration,
    )> {
        use crate::prelude::*;
        let mut quotes = QuoteStore::new(reference_date());
        let mut identifiers = Vec::new();
        for (expiry, volatility) in [("1Y", 0.20), ("2Y", 0.30), ("3Y", 0.40)] {
            for strike in ["0.02", "0.06"] {
                let identifier = if cube {
                    format!("Swaption_USD_SOFR_{expiry}_5Y_Absolute_{strike}_Black")
                } else {
                    format!("CapletFloorlet_USD_SOFR_3M_{expiry}_Absolute_{strike}_Straddle_Black")
                };
                let amount = bump
                    .filter(|(label, _)| *label == identifier)
                    .map_or(0.0, |(_, amount)| amount);
                quotes.add_quote(Quote::new(
                    identifier.parse()?,
                    QuoteLevels::with_mid(volatility + amount),
                ));
                identifiers.push(identifier);
            }
        }
        let source = if cube {
            CalibrationSource::Cube {
                market_index: MarketIndex::SOFR,
            }
        } else {
            CalibrationSource::Surface {
                market_index: MarketIndex::SOFR,
            }
        };
        let calibration = ModelCalibrationConfiguration::new(source)
            .with_calibration_basket(CalibrationBasket::all().with_strike(Strike::Atm));
        let configuration = PricerConfiguration::new(
            InstrumentType::EuropeanSwaption,
            PricerImplementation::HullWhiteClosedForm,
            vec![PricingModelConfiguration::new(
                MarketIndex::SOFR,
                ModelConfiguration::HullWhite {
                    alpha: ALPHA,
                    parameter_source: ParameterSource::Calibrated(calibration),
                },
            )],
        );
        // Deliberately differs from the SOFR index clock: calibration and pricing
        // must both use the curve's time scale.
        let curve = FlatForwardTermStructure::new(
            reference_date(),
            DualFwd::from(RATE),
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        )
        .with_pillar_label("SOFR_flat".into());
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
        );
        let mut context = PricingContext::new()
            .with_quote_store(quotes)
            .with_constructed_elements(elements)
            .with_pricer_configurations(vec![configuration.clone()]);
        if cube {
            context = context.with_volatility_cube_configurations(vec![
                VolatilityCubeConfiguration::new(
                    MarketIndex::SOFR,
                    VolatilityType::Black,
                    SmileType::Strike,
                    identifiers,
                ),
            ]);
        } else {
            context = context.with_volatility_surface_configurations(vec![
                VolatilitySurfaceConfiguration::new(
                    MarketIndex::SOFR,
                    VolatilityType::Black,
                    SmileType::Strike,
                    identifiers,
                ),
            ]);
        }
        context.initialize()?;
        Ok((context, configuration))
    }

    /// Builds the payer or receiver swaption used by calibration-risk tests.
    fn calibrated_trade(option_type: SwaptionType) -> Result<EuropeanSwaptionTrade<DualFwd>> {
        let expiry = reference_date() + Period::new(30, TimeUnit::Months);
        let swaption = MakeSwaption::<DualFwd>::default()
            .with_identifier("CALIBRATED_SWAPTION".into())
            .with_expiry(expiry)
            .with_swap_tenor_date(expiry + Period::new(5, TimeUnit::Years))
            .with_strike(STRIKE)
            .with_notional(1.0)
            .with_rate_definition(RateDefinition::default())
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_swaption_type(option_type)
            .with_fixed_leg_frequency(Frequency::Annual)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .build()?;
        Ok(EuropeanSwaptionTrade::new(
            swaption,
            reference_date(),
            NOTIONAL,
            Side::LongReceive,
        ))
    }

    #[test]
    fn calibrated_surface_and_cube_use_shared_piecewise_variance() -> Result<()> {
        use crate::models::hullwhite::parameters::HullWhiteParameters;
        for cube in [false, true] {
            let (context, configuration) = calibrated_context(cube, None)?;
            let pricer = ClosedFormHullWhiteSwaptionPricer::from_configuration(
                &configuration,
                &MarketIndex::SOFR,
            )?;
            for option_type in [SwaptionType::Payer, SwaptionType::Receiver] {
                let trade = calibrated_trade(option_type)?;
                let request = pricer.market_data_request(&trade).unwrap();
                assert!(request.calibration_request().is_some());
                let market = context.handle_request(&request)?;
                let mut parameters = HullWhiteParameters::new(
                    configuration.model(&MarketIndex::SOFR).unwrap().clone(),
                )?;
                parameters.resolve_calibration(&market, &MarketIndex::SOFR)?;
                let schedule = parameters.sigma_schedule();
                assert_eq!(schedule.len(), 3);
                assert!((schedule[0].1 - schedule[1].1).abs() > 1e-3);
                let time = DayCounter::Actual365
                    .year_fraction(reference_date(), trade.instrument().expiry_date());
                // Independent interval integration, including the change at 2Y.
                let boundary = schedule[1].0;
                let variance = (schedule[0].1.powi(2)
                    * ((-2.0 * ALPHA * (time - boundary)).exp() - (-2.0 * ALPHA * time).exp())
                    + schedule[1].1.powi(2) * (1.0 - (-2.0 * ALPHA * (time - boundary)).exp()))
                    / (2.0 * ALPHA);
                let effective_sigma =
                    (variance * 2.0 * ALPHA / (1.0 - (-2.0 * ALPHA * time).exp())).sqrt();
                let actual = pricer
                    .evaluate(&trade, &[Request::Value], &context)?
                    .price()
                    .unwrap();
                let expected = ClosedFormHullWhiteSwaptionPricer::new(ALPHA, effective_sigma)
                    .evaluate(&trade, &[Request::Value], &context)?
                    .price()
                    .unwrap();
                assert!(
                    (actual - expected).abs() < 1e-8,
                    "piecewise {actual} != integrated {expected}"
                );
                let last_sigma_price = ClosedFormHullWhiteSwaptionPricer::new(ALPHA, schedule[2].1)
                    .evaluate(&trade, &[Request::Value], &context)?
                    .price()
                    .unwrap();
                assert!((actual - last_sigma_price).abs() > 100.0);
            }
        }
        Ok(())
    }

    #[test]
    fn calibration_quote_sensitivities_match_bump_and_recalibrate() -> Result<()> {
        for cube in [false, true] {
            let (context, configuration) = calibrated_context(cube, None)?;
            let pricer = ClosedFormHullWhiteSwaptionPricer::from_configuration(
                &configuration,
                &MarketIndex::SOFR,
            )?;
            let trade = calibrated_trade(SwaptionType::Payer)?;
            let results =
                pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &context)?;
            let risk = results.sensitivities().unwrap();
            let mut nonzero_volatility_pillars = 0;
            for (label, exposure) in risk.instrument_keys().iter().zip(risk.exposure()) {
                if !(label.starts_with("Swaption_") || label.starts_with("CapletFloorlet_")) {
                    continue;
                }
                let bump = 1e-5;
                let mut values = Vec::new();
                for amount in [bump, -bump] {
                    let (bumped, _) = calibrated_context(cube, Some((label, amount)))?;
                    values.push(
                        pricer
                            .evaluate(&trade, &[Request::Value], &bumped)?
                            .price()
                            .unwrap(),
                    );
                }
                let finite_difference = (values[0] - values[1]) / (2.0 * bump);
                assert!(
                    // The shared IFT uses numerical calibration derivatives.
                    (exposure - finite_difference).abs() < 1e-3 * finite_difference.abs().max(1.0),
                    "{label}: AD {exposure} != recalibrated {finite_difference}"
                );
                if exposure.abs() > 1.0 {
                    nonzero_volatility_pillars += 1;
                }
            }
            assert_eq!(
                nonzero_volatility_pillars, 4,
                "both strikes at 1Y and 2Y must receive risk"
            );
        }
        Ok(())
    }

    #[test]
    fn calibrated_model_reports_missing_market_data() -> Result<()> {
        let (_, configuration) = calibrated_context(false, None)?;
        let pricer = ClosedFormHullWhiteSwaptionPricer::from_configuration(
            &configuration,
            &MarketIndex::SOFR,
        )?;
        let error = pricer
            .evaluate(
                &calibrated_trade(SwaptionType::Payer)?,
                &[Request::Value],
                &provider(RATE),
            )
            .err()
            .unwrap();
        assert!(error.to_string().contains("calibration quote details"));
        Ok(())
    }

    struct SimpleMarketDataProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for SimpleMarketDataProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(MarketData::new(
                self.market_data.fixings().clone(),
                self.market_data.constructed_elements().clone(),
            ))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Returns the fixed reference date shared by the swaption test market.
    fn reference_date() -> Date {
        Date::new(2025, 1, 2)
    }

    /// Builds a payer or receiver swaption with the requested trade direction.
    fn build_trade(
        option_type: SwaptionType,
        side: Side,
    ) -> Result<EuropeanSwaptionTrade<DualFwd>> {
        let reference_date = reference_date();
        let expiry = reference_date + Period::new(1, TimeUnit::Years);
        let maturity = expiry + Period::new(3, TimeUnit::Years);
        let swaption = MakeSwaption::<DualFwd>::default()
            .with_identifier("HW_SWAPTION".to_string())
            .with_expiry(expiry)
            .with_swap_tenor_date(maturity)
            .with_strike(STRIKE)
            .with_notional(NOTIONAL)
            .with_rate_definition(RateDefinition::default())
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_swaption_type(option_type)
            .with_fixed_leg_frequency(Frequency::Annual)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .build()?;
        Ok(EuropeanSwaptionTrade::new(
            swaption,
            reference_date,
            NOTIONAL,
            side,
        ))
    }

    /// Builds the flat discount-curve inputs used by swaption valuation tests.
    fn market_data(rate: f64) -> MarketData {
        let curve = FlatForwardTermStructure::new(
            reference_date(),
            DualFwd::from(rate),
            RateDefinition::default(),
        )
        .with_pillar_label("SOFR_flat".to_string());
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
        );
        MarketData::new(HashMap::new(), elements)
    }

    /// Builds the single-curve market provider used by swaption price and risk checks.
    fn provider(rate: f64) -> SimpleMarketDataProvider {
        SimpleMarketDataProvider {
            evaluation_date: reference_date(),
            market_data: market_data(rate),
        }
    }

    /// Builds separate forward and discount curves for swaption pricing tests.
    fn dual_curve_provider(forward_rate: f64, discount_rate: f64) -> SimpleMarketDataProvider {
        let mut market_data = market_data(forward_rate);
        let discount_curve = FlatForwardTermStructure::new(
            reference_date(),
            DualFwd::from(discount_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("discount_flat".to_string());
        market_data
            .constructed_elements_mut()
            .discount_curves_mut()
            .insert(
                MarketIndex::ESTR,
                DiscountCurveElement::new(MarketIndex::ESTR, Rc::new(RefCell::new(discount_curve))),
            );
        SimpleMarketDataProvider {
            evaluation_date: reference_date(),
            market_data,
        }
    }

    /// Extracts the underlying swap's fixed coupon schedule for independent price checks.
    fn fixed_schedule(trade: &EuropeanSwaptionTrade<DualFwd>) -> Vec<(f64, f64)> {
        let day_counter = DayCounter::Actual360;
        trade
            .instrument()
            .underlying()
            .fixed_leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::FixedRateCoupon(coupon) => Some((
                    day_counter.year_fraction(reference_date(), coupon.payment_date()),
                    coupon
                        .rate()
                        .day_counter()
                        .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date()),
                )),
                _ => None,
            })
            .collect()
    }

    /// Evaluates the test swaption and extracts its scalar PV.
    fn price(rate: f64, option_type: SwaptionType, side: Side) -> Result<f64> {
        let trade = build_trade(option_type, side)?;
        let pricer = ClosedFormHullWhiteSwaptionPricer::new(ALPHA, SIGMA);
        Ok(pricer
            .evaluate(&trade, &[Request::Value], &provider(rate))?
            .price()
            .unwrap_or_default())
    }

    #[test]
    fn payer_price_matches_hull_white_model() -> Result<()> {
        let trade = build_trade(SwaptionType::Payer, Side::LongReceive)?;
        let schedule = fixed_schedule(&trade);
        let curve =
            FlatForwardTermStructure::new(reference_date(), RATE, RateDefinition::default());
        let model = HullWhite::new(ALPHA, &curve);
        let option_time =
            DayCounter::Actual360.year_fraction(reference_date(), trade.instrument().expiry_date());
        let expected =
            model.swaption_price(STRIKE, option_time, &schedule, SIGMA, &curve)? * NOTIONAL;

        let pricer = ClosedFormHullWhiteSwaptionPricer::new(ALPHA, SIGMA);
        let actual = pricer
            .evaluate(&trade, &[Request::Value], &provider(RATE))?
            .price()
            .unwrap_or_default();

        assert!(
            (actual - expected).abs() < 1e-8,
            "pricer {actual} != model {expected}"
        );
        Ok(())
    }

    #[test]
    fn receiver_payer_parity_holds() -> Result<()> {
        let payer = price(RATE, SwaptionType::Payer, Side::LongReceive)?;
        let receiver = price(RATE, SwaptionType::Receiver, Side::LongReceive)?;
        let trade = build_trade(SwaptionType::Payer, Side::LongReceive)?;
        let schedule = fixed_schedule(&trade);
        let curve =
            FlatForwardTermStructure::new(reference_date(), RATE, RateDefinition::default());
        let day_counter = DayCounter::Actual360;
        let option_time =
            day_counter.year_fraction(reference_date(), trade.instrument().expiry_date());
        let last_payment = schedule.len() - 1;
        let coupon_bond = schedule.iter().enumerate().try_fold(
            0.0,
            |value, (index, &(payment_time, accrual))| -> Result<f64> {
                let coefficient = if index == last_payment {
                    accrual.mul_add(STRIKE, 1.0)
                } else {
                    accrual * STRIKE
                };
                Ok(coefficient.mul_add(curve.discount_factor_from_time(payment_time)?, value))
            },
        )?;
        let expected_difference =
            (coupon_bond - curve.discount_factor_from_time(option_time)?) * NOTIONAL;

        assert!(
            (receiver - payer - expected_difference).abs() < 1e-8,
            "receiver-payer parity failed"
        );
        Ok(())
    }

    #[test]
    fn short_side_negates_price() -> Result<()> {
        let long = price(RATE, SwaptionType::Payer, Side::LongReceive)?;
        let short = price(RATE, SwaptionType::Payer, Side::PayShort)?;
        assert!((long + short).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn curve_sensitivity_matches_bump_and_reprice() -> Result<()> {
        let trade = build_trade(SwaptionType::Payer, Side::LongReceive)?;
        let pricer = ClosedFormHullWhiteSwaptionPricer::new(ALPHA, SIGMA);
        let results = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Sensitivities],
            &provider(RATE),
        )?;
        let sensitivity = results
            .sensitivities()
            .and_then(|risk| risk.exposure().first())
            .copied()
            .unwrap_or_default();

        let bump = 1e-5;
        let finite_difference = (price(RATE + bump, SwaptionType::Payer, Side::LongReceive)?
            - price(RATE - bump, SwaptionType::Payer, Side::LongReceive)?)
            / (2.0 * bump);
        let tolerance = 1e-5 * finite_difference.abs().max(1.0);
        assert!(
            (sensitivity - finite_difference).abs() < tolerance,
            "AAD {sensitivity} != finite difference {finite_difference}"
        );
        Ok(())
    }

    #[test]
    fn dual_curve_discounting_reports_both_curve_sensitivities() -> Result<()> {
        let trade = build_trade(SwaptionType::Payer, Side::LongReceive)?;
        let mut pricer = ClosedFormHullWhiteSwaptionPricer::new(ALPHA, SIGMA);
        pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
            MarketIndex::ESTR,
            Currency::USD,
        )));
        let results = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Sensitivities],
            &dual_curve_provider(RATE, 0.03),
        )?;
        let identifiers = results
            .sensitivities()
            .map(|risk| risk.instrument_keys())
            .unwrap_or_default();
        assert!(identifiers.iter().any(|label| label == "SOFR_flat"));
        assert!(identifiers.iter().any(|label| label == "discount_flat"));
        Ok(())
    }

    #[test]
    fn invalid_model_parameters_are_rejected() -> Result<()> {
        let trade = build_trade(SwaptionType::Payer, Side::LongReceive)?;
        assert!(ClosedFormHullWhiteSwaptionPricer::new(0.0, SIGMA)
            .evaluate(&trade, &[Request::Value], &provider(RATE))
            .is_err());
        assert!(ClosedFormHullWhiteSwaptionPricer::new(ALPHA, -SIGMA)
            .evaluate(&trade, &[Request::Value], &provider(RATE))
            .is_err());
        Ok(())
    }
}
