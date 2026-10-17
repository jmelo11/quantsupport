use serde::{Deserialize, Serialize};

use crate::{
    ad::scalar::Scalar,
    time::date::Date,
    utils::errors::{QSError, Result},
};

use super::{prepaymentmodel::PrepaymentModel, prepaymentmodelstate::PrepaymentModelState};

/// Portable prepayment definitions shared by Rust, Python and Excel.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum PrepaymentModelConfiguration {
    /// No unscheduled principal payments.
    #[default]
    None,
    /// Constant annual conditional prepayment rate.
    Cpr(f64),
    /// Annual CPR by effective date; zero before the first date, flat afterwards.
    CprCurve(Vec<(Date, f64)>),
    /// Base annual CPR plus sensitivity to coupon minus refinancing benchmark.
    Refinancing {
        /// Annual CPR absent a positive refinancing incentive.
        base_cpr: f64,
        /// CPR increase per unit of positive rate incentive.
        incentive_multiplier: f64,
        /// Maximum annual CPR.
        maximum_cpr: f64,
    },
}

impl PrepaymentModelConfiguration {
    /// Validates annual CPR parameters and chronological curve dates.
    ///
    /// # Errors
    /// Rejects nonfinite values, CPRs outside [0, 1], unsorted or duplicate
    /// curve dates, and invalid refinancing incentive parameters.
    pub fn validate_parameters(&self) -> Result<()> {
        let valid_cpr = |rate: f64| rate.is_finite() && (0.0..=1.0).contains(&rate);
        let valid = match self {
            Self::None => true,
            Self::Cpr(rate) => valid_cpr(*rate),
            Self::CprCurve(points) => {
                points.iter().all(|(_, rate)| valid_cpr(*rate))
                    && points.windows(2).all(|pair| pair[0].0 < pair[1].0)
            }
            Self::Refinancing {
                base_cpr,
                incentive_multiplier,
                maximum_cpr,
            } => {
                valid_cpr(*base_cpr)
                    && valid_cpr(*maximum_cpr)
                    && base_cpr <= maximum_cpr
                    && incentive_multiplier.is_finite()
                    && *incentive_multiplier >= 0.0
            }
        };
        if !valid {
            return Err(QSError::InvalidValueErr(
                "Invalid CPR parameters or curve dates".into(),
            ));
        }
        Ok(())
    }
}

impl<T: Scalar> PrepaymentModel<T> for PrepaymentModelConfiguration {
    /// Requests a market refinancing observation only for the incentive model.
    fn requires_refinancing_benchmark(&self) -> bool {
        matches!(self, Self::Refinancing { .. })
    }
    /// Converts the applicable annual CPR to this interval's conditional probability.
    ///
    /// Uses `1 - (1 - CPR)^year_fraction`, returning zero for a zero-length year
    /// fraction and one for CPR 1 over a positive interval. Dated CPRs use the
    /// latest point at/before the interval start, or zero before the first point.
    /// Refinancing CPR is capped base CPR plus the multiplier times positive
    /// contractual coupon minus refinancing benchmark; AD dependencies are retained.
    ///
    /// # Errors
    /// Rejects invalid configuration, nonfinite/negative year fractions,
    /// non-increasing interval dates, and nonfinite calculated probabilities.
    fn conditional_prepayment_probability(&self, state: &PrepaymentModelState<T>) -> Result<T> {
        self.validate_parameters()?;
        if !state.interval_year_fraction().is_finite()
            || state.interval_year_fraction() < 0.0
            || state.interval_end_date() <= state.interval_start_date()
        {
            return Err(QSError::InvalidValueErr(
                "Invalid prepayment interval".into(),
            ));
        }
        let cpr = match self {
            Self::None => T::zero(),
            Self::Cpr(rate) => T::scalar(*rate),
            Self::CprCurve(points) => T::scalar(
                points
                    .iter()
                    .rev()
                    .find(|(date, _)| *date <= state.interval_start_date())
                    .map_or(0.0, |(_, rate)| *rate),
            ),
            Self::Refinancing {
                base_cpr,
                incentive_multiplier,
                maximum_cpr,
            } => T::scalar(*base_cpr)
                .add_val(
                    state
                        .contractual_coupon_rate()
                        .sub_val(state.refinancing_benchmark_rate())
                        .max_val(T::zero())
                        .mul_val(T::scalar(*incentive_multiplier)),
                )
                .min_val(T::scalar(*maximum_cpr)),
        };
        let probability = if state.interval_year_fraction() == 0.0 {
            T::zero()
        } else if cpr.value() >= 1.0 {
            T::one()
        } else {
            T::one().sub_val(T::one().sub_val(cpr).powf(state.interval_year_fraction()))
        };
        if !probability.value().is_finite() {
            return Err(QSError::InvalidValueErr(
                "Nonfinite prepayment probability".into(),
            ));
        }
        Ok(probability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample_monthly_prepayment_state() -> PrepaymentModelState<f64> {
        PrepaymentModelState::new(
            Date::new(2026, 1, 1),
            Date::new(2026, 2, 1),
            1.0 / 12.0,
            100.0,
            0.06,
            0.03,
        )
    }
    #[test]
    fn annual_cpr_converts_to_conditional_monthly_probability() -> Result<()> {
        let probability = PrepaymentModelConfiguration::Cpr(0.12)
            .conditional_prepayment_probability(&sample_monthly_prepayment_state())?;
        assert!((probability - (1.0 - 0.88_f64.powf(1.0 / 12.0))).abs() < 1e-14);
        Ok(())
    }
    #[test]
    fn dated_cpr_uses_interval_start_and_validates_dates() -> Result<()> {
        let date = Date::new(2026, 1, 1);
        let curve = PrepaymentModelConfiguration::CprCurve(vec![
            (date, 0.12),
            (Date::new(2026, 2, 1), 0.24),
        ]);
        assert_eq!(
            curve.conditional_prepayment_probability(&sample_monthly_prepayment_state())?,
            PrepaymentModelConfiguration::Cpr(0.12)
                .conditional_prepayment_probability(&sample_monthly_prepayment_state())?
        );
        assert!(
            PrepaymentModelConfiguration::CprCurve(vec![(date, 0.1), (date, 0.2)])
                .conditional_prepayment_probability(&sample_monthly_prepayment_state())
                .is_err()
        );
        assert!(PrepaymentModelConfiguration::Cpr(f64::NAN)
            .conditional_prepayment_probability(&sample_monthly_prepayment_state())
            .is_err());
        Ok(())
    }
    #[test]
    fn refinancing_incentive_increases_prepayment_and_caps_at_one() -> Result<()> {
        let model = PrepaymentModelConfiguration::Refinancing {
            base_cpr: 0.05,
            incentive_multiplier: 10.0,
            maximum_cpr: 1.0,
        };
        assert!(
            model.conditional_prepayment_probability(&sample_monthly_prepayment_state())?
                > PrepaymentModelConfiguration::Cpr(0.05)
                    .conditional_prepayment_probability(&sample_monthly_prepayment_state())?
        );
        assert_eq!(
            PrepaymentModelConfiguration::Cpr(1.0)
                .conditional_prepayment_probability(&sample_monthly_prepayment_state())?,
            1.0
        );
        Ok(())
    }
}
