use crate::ad::scalar::Scalar;
use crate::time::date::Date;
use crate::time::daycounter::DayCounter;
use crate::utils::errors::Result;
use crate::xva::visitors::exposureevaluator::NpvCube;

pub trait ValuationAdjustment<T: Scalar> {
    fn value(&self) -> Result<T>;
}

pub struct CVA<'a, T: Scalar> {
    credit_spread: T,
    recovery: T,
    epe: Vec<f64>,
    dates: &'a [Date],
}

impl<'a, T> CVA<'a, T>
where
    T: Scalar,
{
    pub fn new(credit_spread: T, recovery: T, cube: &'a NpvCube) -> Self {
        Self {
            credit_spread,
            recovery,
            epe: cube.epe(),
            dates: &cube.dates,
        }
    }
}

impl<'a, T> ValuationAdjustment<T> for CVA<'a, T>
where
    T: Scalar,
{
    /// Computes unilateral CVA:
    ///
    /// ```text
    /// CVA = (1 - R) * sum_i EPE(t_i) * [S(t_{i-1}) - S(t_i)]
    /// ```
    ///
    /// where the hazard rate is `spread / (1 - R)` and the survival
    /// probability is `S(t) = exp(-hazard * t)`.
    fn value(&self) -> Result<T> {
        let dc = DayCounter::Actual365;
        let lgd = T::one().sub_val(self.recovery);
        let hazard_rate = self.credit_spread.div_val(lgd);
        let dates = self.dates;
        let ref_date = dates[0];

        let res = dates
            .windows(2)
            .enumerate()
            .fold(T::zero(), |mut acc, (pos, w)| {
                let t_prev = dc.year_fraction(ref_date, w[0]);
                let t_curr = dc.year_fraction(ref_date, w[1]);
                let surv_prev = hazard_rate.neg_val().mul_val(T::scalar(t_prev)).exp();
                let surv_curr = hazard_rate.neg_val().mul_val(T::scalar(t_curr)).exp();
                let default_prob = surv_prev.sub_val(surv_curr);
                let exposure_t = *self.epe.get(pos + 1).unwrap_or(&0.0);
                acc = acc.add_val(default_prob.mul_val(T::scalar(exposure_t)));
                acc
            });

        Ok(res.mul_val(lgd))
    }
}
