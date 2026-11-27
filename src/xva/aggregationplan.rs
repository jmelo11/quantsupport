//! Prepared path aggregation with setup-time credit and funding dependencies.

use crate::ad::scalar::Scalar;

/// Exposure transformation applied before bucket-weight integration.
#[derive(Clone, Copy, Debug)]
pub enum ExposureTransform {
    /// Positive exposure for CVA.
    Positive,
    /// Positive magnitude of negative exposure for DVA.
    Negative,
    /// Signed exposure for FVA.
    Signed,
}

/// Dated bucket weights and a post-integration scale, prepared once per setup.
#[derive(Clone)]
pub struct AggregationPlan<T: Scalar> {
    transform: ExposureTransform,
    weights: Vec<T>,
    scale: T,
}

impl<T: Scalar> AggregationPlan<T> {
    /// Creates an aggregation plan, with bucket zero normally weighted by zero.
    #[must_use]
    pub const fn new(transform: ExposureTransform, weights: Vec<T>, scale: T) -> Self {
        Self {
            transform,
            weights,
            scale,
        }
    }
    /// Coefficients ordered as bucket weights followed by the post-integration scale.
    #[must_use]
    pub fn coefficients(&self) -> Vec<T> {
        let mut result = self.weights.clone();
        result.push(self.scale);
        result
    }
    /// Replays aggregation with a compatible scalar and supplied numerical coefficients.
    ///
    /// # Panics
    /// Panics unless coefficients contain one weight per date followed by one scale.
    #[must_use]
    pub fn evaluate_with<U: Scalar>(&self, npvs: &[U], coefficients: &[U]) -> U {
        assert_eq!(
            coefficients.len(),
            self.weights.len() + 1,
            "aggregation coefficient layout"
        );
        let mut total = U::zero();
        for (index, &npv) in npvs.iter().enumerate().take(self.weights.len()).skip(1) {
            let exposure = match self.transform {
                ExposureTransform::Positive => npv.max_val(U::zero()),
                ExposureTransform::Negative => npv.neg_val().max_val(U::zero()),
                ExposureTransform::Signed => npv,
            };
            total = total.add_val(exposure.mul_val(coefficients[index]));
        }
        total.mul_val(coefficients[self.weights.len()])
    }
    /// Detaches coefficient values while retaining the integration convention.
    #[must_use]
    pub fn numerical(&self) -> AggregationPlan<f64> {
        AggregationPlan::new(
            self.transform,
            self.weights.iter().map(Scalar::value).collect(),
            self.scale.value(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ad::CompiledProgram,
        prelude::*,
        xva::aggregator::{CvaAggregator, DvaAggregator, FvaAggregator, XvaAggregator},
    };
    /// Checks native plans match all builtin adjustment transforms.
    #[test]
    fn native_plans_match_all_builtin_adjustment_transforms() -> Result<()> {
        let reference = Date::new(2025, 1, 1);
        let dates = [reference, Date::new(2026, 1, 1), Date::new(2027, 1, 1)];
        let aggregators: Vec<Box<dyn XvaAggregator<f64>>> = vec![
            Box::new(CvaAggregator::new(0.01, 0.4, 4, reference, &dates)),
            Box::new(DvaAggregator::new(0.02, 0.3, 4, reference, &dates)),
            Box::new(FvaAggregator::new(0.005, 4)),
        ];
        for aggregator in aggregators {
            let plan = aggregator
                .compiled_plan(&dates)
                .ok_or_else(|| QSError::NotFoundErr("compiled aggregator".into()))?;
            let coefficients = plan.coefficients();
            let mut samples = vec![0.0, 100.0, -30.0];
            samples.extend_from_slice(&coefficients);
            let program = CompiledProgram::trace(&samples, samples.len(), |inputs| {
                let value = plan.evaluate_with(&inputs[..3], &inputs[3..]);
                Ok::<_, &str>((value, vec![value]))
            })
            .map_err(|error| QSError::EvaluationErr(error.to_string()))?;
            for npvs in [[0.0, 100.0, -30.0], [0.0, -100.0, 30.0]] {
                samples[..3].copy_from_slice(&npvs);
                let mut value = [0.0];
                let mut gradient = vec![0.0; samples.len()];
                program
                    .evaluate(&samples, &mut value, &mut gradient)
                    .map_err(|error| QSError::EvaluationErr(error.to_string()))?;
                let expected = aggregator.aggregate_path(&npvs, &dates);
                assert!((value[0] - expected).abs() < 1e-12);
                for i in 1..3 {
                    let mut above = npvs;
                    let mut below = npvs;
                    above[i] += 1e-4;
                    below[i] -= 1e-4;
                    let expected = (aggregator.aggregate_path(&above, &dates)
                        - aggregator.aggregate_path(&below, &dates))
                        / 2e-4;
                    assert!((gradient[i] - expected).abs() < 1e-9);
                }
            }
        }
        Ok(())
    }
}
