//! Implied calibration quotes evaluated against existing curves.

use crate::{
    calibration::calibrationpricer::CalibrationInstrumentPricer,
    core::{elements::curveelement::DiscountCurveElement, pricingcontext::PricingContext},
    indices::marketindex::MarketIndex,
    quotes::{
        calibrationinstrument::CalibrationInstrument,
        quote::{
            BondCalibrationStrategy, Level, Quote, QuoteDetails, QuoteInstrument, QuoteLevels,
        },
    },
    rates::bootstrapping::{
        bootstrapcalibrationinstrument::BootstrapStepEvaluation,
        bootstrapdiscountpolicy::BootstrapDiscountPolicy, bootstrappedcurve::BootstrappedCurve,
        bootstrapstep::BootstrapStep,
    },
    utils::errors::{QSError, Result},
};
use std::{collections::HashMap, hash::BuildHasher};

/// Prices native calibration quotes against existing curves at the context date.
pub struct ImpliedQuotePricer<'a, S> {
    context: &'a PricingContext,
    curves: &'a HashMap<MarketIndex, DiscountCurveElement, S>,
}

impl<'a, S: BuildHasher> ImpliedQuotePricer<'a, S> {
    /// Binds implied quotation to the context and its constructed curves.
    #[must_use]
    pub const fn new(
        context: &'a PricingContext,
        curves: &'a HashMap<MarketIndex, DiscountCurveElement, S>,
    ) -> Self {
        Self { context, curves }
    }

    /// Solves native quotes without changing or bootstrapping the supplied curves.
    ///
    /// # Errors
    /// Returns an error for missing dependencies, incompatible dates, or failed convergence.
    pub fn quotes(&self, instruments: &[QuoteDetails]) -> Result<Vec<f64>> {
        let context = self.context;
        let curves = self.curves;
        if curves
            .values()
            .any(|element| element.curve().reference_date() != context.evaluation_date())
        {
            return Err(QSError::InvalidValueErr(
                "Implied quotation curves must share the context evaluation date".into(),
            ));
        }
        let snapshots: HashMap<_, _> = curves
            .iter()
            .map(|(index, curve)| {
                Ok((
                    index.clone(),
                    BootstrappedCurve::from_runtime_curve(
                        index.clone(),
                        curve.curve().clone_curve()?,
                    ),
                ))
            })
            .collect::<Result<_>>()?;
        let trial = snapshots
            .values()
            .next()
            .ok_or_else(|| QSError::InvalidValueErr("No curves for implied quotation".into()))?;
        let policy =
            BootstrapDiscountPolicy::new(context.base_index().clone(), context.base_currency());
        let step = BootstrapStep::new(trial, &snapshots, &policy, context.fx_store())
            .with_fixing_store(Some(context.fixing_store()));
        let evaluator = BootstrapStepEvaluation::new(&step);
        instruments
            .iter()
            .map(|details| {
                let fx = details
                    .pay_currency()
                    .zip(details.receive_currency())
                    .map(|(pay, receive)| {
                        context
                            .fx_store()
                            .get_fx_rate(pay, receive)
                            .map(|value| value.value())
                    })
                    .transpose()?;
                let mut value = if *details.instrument() == QuoteInstrument::Future {
                    95.0
                } else if *details.instrument() == QuoteInstrument::FixedRateBond
                    && details.bond_calibration_strategy()
                        == Some(BondCalibrationStrategy::AnchorPrice)
                {
                    100.0
                } else {
                    0.03
                };
                for _ in 0..80 {
                    let quote = Quote::new(details.clone(), QuoteLevels::with_mid(value));
                    let built =
                        quote.build_instrument(context.evaluation_date(), Level::Mid, fx)?;
                    let pillar = quote.calibration_pillar_date(&built)?;
                    let instrument =
                        CalibrationInstrument::new(quote, Level::Mid, built, value, pillar);
                    let residual = evaluator.price(&instrument)?;
                    let derivative = evaluator.sensitivity(&instrument)?;
                    if !residual.is_finite() || !derivative.is_finite() || derivative.abs() < 1e-14
                    {
                        break;
                    }
                    let delta = residual / derivative;
                    if delta.abs() < 1e-12 {
                        return Ok(value);
                    }
                    value -= delta;
                }
                Err(QSError::InvalidValueErr(format!(
                    "Cannot solve implied quote {}",
                    details.identifier()
                )))
            })
            .collect()
    }
}
