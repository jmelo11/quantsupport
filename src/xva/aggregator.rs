//! Pfe aggregator trait and implementations.
//!
//! An [`PfeAggregator`] takes per-date NPVs from a single MC path and
//! combines them into a single Pfe contribution scalar that can be used
//! as the backward-pass root.

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    time::{date::Date, daycounter::DayCounter},
};

/// Aggregates per-date NPVs from one MC path into a single Pfe contribution.
///
/// Implementations are constructed **before** `set_mark` so that their
/// `T`-typed fields (credit spread, recovery, survival probabilities, etc.)
/// become pre-mark tape leaves.  [`aggregate_path`](Self::aggregate_path) is
/// called **after** the mark inside each path iteration.
pub trait PfeAggregator<T: Scalar>: Send + Sync {
    /// Human-readable name for this measure (e.g. `"CVA"`, `"DVA"`).
    fn name(&self) -> &'static str;

    /// Combine per-date NPVs into a single Pfe contribution for one path.
    ///
    /// `npvs[d]` is the portfolio NPV at `dates[d]`.
    /// The returned `T` is the root for `backward_to_mark()`.
    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T;
}

/// Bundle returned by an [`PfeAggregatorFactory`]: the aggregator together
/// with tracked `DualFwd` leaves whose adjoints carry sensitivities after
/// the backward pass.
pub struct AggregatorBundle {
    /// The aggregator instance (lives on the current thread's tape).
    pub aggregator: Box<dyn PfeAggregator<DualFwd>>,
    /// Tracked leaves: `(label, leaf)` pairs whose `.adjoint()` can be read
    /// after `propagate_mark_to_start`.
    pub leaves: Vec<(String, DualFwd)>,
}

/// Factory for creating per-thread [`PfeAggregator`] instances.
///
/// Each rayon thread calls [`create_aggregator`](Self::create_aggregator) to
/// build its own aggregator with `DualFwd` leaves on the thread-local tape
/// (pre-mark).
pub trait PfeAggregatorFactory: Send + Sync {
    /// Human-readable name for the Pfe measure (e.g. `"CVA"`).
    fn name(&self) -> &'static str;

    /// Creates an aggregator and its tracked leaves on the current thread's
    /// tape.  Must be called **before** `set_mark`.
    fn create_aggregator(&self, ref_date: Date, dates: &[Date]) -> AggregatorBundle;
}

/// Unilateral CVA aggregator.
pub struct CvaAggregator<T: Scalar> {
    /// Loss-given-default: `1 − R`.
    lgd: T,
    /// Survival probabilities at each simulation date: `S(t_d) = exp(−λ t_d)`.
    survival_probs: Vec<T>,
    /// System-curve discount factors `DF(0, t_d)` at each simulation date.
    /// When absent, exposures are aggregated undiscounted.
    system_discounts: Option<Vec<f64>>,
    /// `1 / n_paths`.
    inv_n: f64,
}

impl<T: Scalar> CvaAggregator<T> {
    /// Creates a new CVA aggregator.
    ///
    /// If `credit_spread` and `recovery` are `DualFwd` leaves on the tape,
    /// credit sensitivities propagate automatically.  For no credit
    /// sensitivities, pass `T::scalar(val)` constants.
    pub fn new(
        credit_spread: T,
        recovery: T,
        n_paths: usize,
        ref_date: Date,
        dates: &[Date],
    ) -> Self {
        let lgd = T::one().sub_val(recovery);
        let hazard_rate = credit_spread.div_val(lgd);
        let dc = DayCounter::Actual365;
        let survival_probs: Vec<T> = dates
            .iter()
            .map(|d| {
                let t = dc.year_fraction(ref_date, *d);
                hazard_rate.neg_val().mul_val(T::scalar(t)).exp()
            })
            .collect();
        Self {
            lgd,
            survival_probs,
            system_discounts: None,
            inv_n: 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX)),
        }
    }

    /// Creates a CVA aggregator directly from survival probabilities at each
    /// simulation date (e.g. interpolated from a bootstrapped credit curve).
    pub fn from_survival_probs(recovery: T, survival_probs: Vec<T>, n_paths: usize) -> Self {
        Self {
            lgd: T::one().sub_val(recovery),
            survival_probs,
            system_discounts: None,
            inv_n: 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX)),
        }
    }

    /// Sets the system-curve discount factors `DF(0, t_d)` applied to the
    /// exposures so that CVA is expressed in present-value terms.
    #[must_use]
    pub fn with_system_discounts(mut self, discounts: Vec<f64>) -> Self {
        self.system_discounts = Some(discounts);
        self
    }
}

impl<T: Scalar> PfeAggregator<T> for CvaAggregator<T> {
    fn name(&self) -> &'static str {
        "CVA"
    }

    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T {
        let mut c_p = T::zero();
        let n = dates.len().min(npvs.len());
        for (d, npv) in npvs.iter().enumerate().take(n).skip(1) {
            let mut exposure = npv.max_val(T::zero());
            if let Some(dfs) = &self.system_discounts {
                exposure = exposure.mul_val(T::scalar(dfs[d]));
            }
            let delta_pd = self.survival_probs[d - 1].sub_val(self.survival_probs[d]);
            c_p = c_p.add_val(exposure.mul_val(delta_pd));
        }
        c_p.mul_val(self.lgd).mul_val(T::scalar(self.inv_n))
    }
}

/// Unilateral DVA aggregator (own-default).
pub struct DvaAggregator<T: Scalar> {
    lgd: T,
    survival_probs: Vec<T>,
    system_discounts: Option<Vec<f64>>,
    inv_n: f64,
}

impl<T: Scalar> DvaAggregator<T> {
    /// Creates a unilateral DVA estimator from a flat own-credit spread and recovery rate.
    pub fn new(
        own_spread: T,
        own_recovery: T,
        n_paths: usize,
        ref_date: Date,
        dates: &[Date],
    ) -> Self {
        let lgd = T::one().sub_val(own_recovery);
        let hazard_rate = own_spread.div_val(lgd);
        let dc = DayCounter::Actual365;
        let survival_probs: Vec<T> = dates
            .iter()
            .map(|d| {
                let t = dc.year_fraction(ref_date, *d);
                hazard_rate.neg_val().mul_val(T::scalar(t)).exp()
            })
            .collect();
        Self {
            lgd,
            survival_probs,
            system_discounts: None,
            inv_n: 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX)),
        }
    }

    /// Sets the system-curve discount factors `DF(0, t_d)` applied to the
    /// exposures so that DVA is expressed in present-value terms.
    #[must_use]
    pub fn with_system_discounts(mut self, discounts: Vec<f64>) -> Self {
        self.system_discounts = Some(discounts);
        self
    }
}

impl<T: Scalar> PfeAggregator<T> for DvaAggregator<T> {
    fn name(&self) -> &'static str {
        "DVA"
    }

    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T {
        let mut d_p = T::zero();
        let n = dates.len().min(npvs.len());
        for (d, npv) in npvs.iter().enumerate().take(n).skip(1) {
            let mut exposure = npv.neg_val().max_val(T::zero());
            if let Some(dfs) = &self.system_discounts {
                exposure = exposure.mul_val(T::scalar(dfs[d]));
            }
            let delta_pd = self.survival_probs[d - 1].sub_val(self.survival_probs[d]);
            d_p = d_p.add_val(exposure.mul_val(delta_pd));
        }
        d_p.mul_val(self.lgd).mul_val(T::scalar(self.inv_n))
    }
}

/// Funding spread term used by the [`FvaAggregator`]: either a single flat
/// spread or one spread per simulation date.
enum FundingSpreadTerm<T> {
    /// Flat spread applied to every accrual bucket.
    Flat(T),
    /// `spreads[d]` applies over the bucket ending at simulation date `d`.
    PerDate(Vec<T>),
}

/// Funding valuation adjustment aggregator.
pub struct FvaAggregator<T: Scalar> {
    spread: FundingSpreadTerm<T>,
    system_discounts: Option<Vec<f64>>,
    inv_n: f64,
}

impl<T: Scalar> FvaAggregator<T> {
    /// Creates an FVA aggregator with a flat funding spread.
    pub fn new(funding_spread: T, n_paths: usize) -> Self {
        Self {
            spread: FundingSpreadTerm::Flat(funding_spread),
            system_discounts: None,
            inv_n: 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX)),
        }
    }

    /// Creates an FVA aggregator with a time-dependent funding spread.
    ///
    /// `spreads[d]` is the spread applied over the accrual bucket ending at
    /// simulation date `d` (one entry per simulation date, e.g. interpolated
    /// from a funding spread curve).
    #[must_use]
    pub fn from_spreads(spreads: Vec<T>, n_paths: usize) -> Self {
        Self {
            spread: FundingSpreadTerm::PerDate(spreads),
            system_discounts: None,
            inv_n: 1.0 / f64::from(u32::try_from(n_paths).unwrap_or(u32::MAX)),
        }
    }

    /// Sets the system-curve discount factors `DF(0, t_d)` applied to the
    /// funding cost so that FVA is expressed in present-value terms.
    #[must_use]
    pub fn with_system_discounts(mut self, discounts: Vec<f64>) -> Self {
        self.system_discounts = Some(discounts);
        self
    }
}

impl<T: Scalar> PfeAggregator<T> for FvaAggregator<T> {
    fn name(&self) -> &'static str {
        "FVA"
    }

    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T {
        let dc = DayCounter::Actual365;
        let mut f_p = T::zero();
        for d in 1..dates.len().min(npvs.len()) {
            let dt = dc.year_fraction(dates[d - 1], dates[d]);
            let spread = match &self.spread {
                FundingSpreadTerm::Flat(s) => *s,
                FundingSpreadTerm::PerDate(v) => v[d],
            };
            let mut term = npvs[d].mul_val(spread).mul_val(T::scalar(dt));
            if let Some(dfs) = &self.system_discounts {
                term = term.mul_val(T::scalar(dfs[d]));
            }
            f_p = f_p.add_val(term);
        }
        f_p.mul_val(T::scalar(self.inv_n))
    }
}

/// Factory for [`CvaAggregator`].
pub struct CvaFactory {
    /// Flat counterparty credit spread used to derive default probabilities.
    pub credit_spread: f64,
    /// Counterparty recovery rate.
    pub recovery: f64,
    /// Number of Monte Carlo paths used to normalize the aggregate.
    pub n_paths: usize,
    /// System-curve discount factors `DF(0, t_d)` at the simulation dates.
    pub system_dfs: Option<Vec<f64>>,
}

impl PfeAggregatorFactory for CvaFactory {
    fn name(&self) -> &'static str {
        "CVA"
    }

    fn create_aggregator(&self, ref_date: Date, dates: &[Date]) -> AggregatorBundle {
        let cs = DualFwd::new(self.credit_spread);
        let rec = DualFwd::new(self.recovery);
        let mut agg = CvaAggregator::new(cs, rec, self.n_paths, ref_date, dates);
        if let Some(dfs) = &self.system_dfs {
            agg = agg.with_system_discounts(dfs.clone());
        }
        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves: vec![
                ("CVA.credit_spread".to_string(), cs),
                ("CVA.recovery".to_string(), rec),
            ],
        }
    }
}

/// Factory for a CVA aggregator driven by a bootstrapped credit curve.
///
/// Survival probabilities at the simulation dates are log-linearly
/// interpolated between the curve pillars (flat-hazard extrapolation beyond
/// the last pillar). The pillar survivals become tracked tape leaves so that
/// the backward pass yields CVA sensitivities to the original CDS quotes
/// (labels are prefixed with `"CVA."`).
pub struct CreditCurveCvaFactory {
    /// Pillar dates of the credit curve (strictly after the reference date).
    pub pillar_dates: Vec<Date>,
    /// Survival probabilities at the pillar dates.
    pub pillar_survivals: Vec<f64>,
    /// Labels of the pillar quotes (e.g. CDS quote identifiers).
    pub pillar_labels: Vec<String>,
    /// Counterparty recovery rate.
    pub recovery: f64,
    /// Number of Monte Carlo paths.
    pub n_paths: usize,
    /// Day counter of the credit curve.
    pub day_counter: DayCounter,
    /// System-curve discount factors `DF(0, t_d)` at the simulation dates.
    pub system_dfs: Option<Vec<f64>>,
}

impl CreditCurveCvaFactory {
    /// Log-linear survival interpolation at time `t` (year fraction from the
    /// reference date), with flat-hazard extrapolation beyond the last pillar.
    fn survival_at(t: f64, pillar_times: &[f64], leaves: &[DualFwd]) -> DualFwd {
        let n = pillar_times.len();
        if t <= 0.0 || n == 0 {
            return DualFwd::scalar(1.0);
        }
        // Before or at the first pillar: interpolate between (0, S=1) and t_1.
        if t <= pillar_times[0] {
            let w = t / pillar_times[0];
            return leaves[0].ln().mul_val(DualFwd::scalar(w)).exp();
        }
        // Between pillars.
        for k in 1..n {
            if t <= pillar_times[k] {
                let w = (t - pillar_times[k - 1]) / (pillar_times[k] - pillar_times[k - 1]);
                let ln_s = leaves[k - 1].ln().add_val(
                    leaves[k]
                        .ln()
                        .sub_val(leaves[k - 1].ln())
                        .mul_val(DualFwd::scalar(w)),
                );
                return ln_s.exp();
            }
        }
        // Beyond the last pillar: flat hazard from the last bucket.
        let last = n - 1;
        let (t_prev, ln_prev) = if n >= 2 {
            (pillar_times[last - 1], leaves[last - 1].ln())
        } else {
            (0.0, DualFwd::scalar(0.0))
        };
        let dt_bucket = pillar_times[last] - t_prev;
        let hazard = ln_prev
            .sub_val(leaves[last].ln())
            .div_val(DualFwd::scalar(dt_bucket));
        leaves[last]
            .ln()
            .sub_val(hazard.mul_val(DualFwd::scalar(t - pillar_times[last])))
            .exp()
    }
}

impl PfeAggregatorFactory for CreditCurveCvaFactory {
    fn name(&self) -> &'static str {
        "CVA"
    }

    fn create_aggregator(&self, ref_date: Date, dates: &[Date]) -> AggregatorBundle {
        // Tracked leaves: one per pillar survival + recovery.
        let pillar_leaves: Vec<DualFwd> = self
            .pillar_survivals
            .iter()
            .map(|s| DualFwd::new(*s))
            .collect();
        let rec = DualFwd::new(self.recovery);

        let pillar_times: Vec<f64> = self
            .pillar_dates
            .iter()
            .map(|d| self.day_counter.year_fraction(ref_date, *d))
            .collect();

        let survival_probs: Vec<DualFwd> = dates
            .iter()
            .map(|d| {
                let t = self.day_counter.year_fraction(ref_date, *d);
                Self::survival_at(t, &pillar_times, &pillar_leaves)
            })
            .collect();

        let mut agg = CvaAggregator::from_survival_probs(rec, survival_probs, self.n_paths);
        if let Some(dfs) = &self.system_dfs {
            agg = agg.with_system_discounts(dfs.clone());
        }

        let mut leaves: Vec<(String, DualFwd)> = self
            .pillar_labels
            .iter()
            .zip(&pillar_leaves)
            .map(|(label, leaf)| (format!("CVA.{label}"), *leaf))
            .collect();
        leaves.push(("CVA.recovery".to_string(), rec));

        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves,
        }
    }
}

/// Factory for [`DvaAggregator`].
pub struct DvaFactory {
    /// Flat own-credit spread used to derive default probabilities.
    pub own_spread: f64,
    /// Own recovery rate.
    pub own_recovery: f64,
    /// Number of Monte Carlo paths used to normalize the aggregate.
    pub n_paths: usize,
}

impl PfeAggregatorFactory for DvaFactory {
    fn name(&self) -> &'static str {
        "DVA"
    }

    fn create_aggregator(&self, ref_date: Date, dates: &[Date]) -> AggregatorBundle {
        let sp = DualFwd::new(self.own_spread);
        let rec = DualFwd::new(self.own_recovery);
        let agg = DvaAggregator::new(sp, rec, self.n_paths, ref_date, dates);
        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves: vec![
                ("DVA.own_spread".to_string(), sp),
                ("DVA.own_recovery".to_string(), rec),
            ],
        }
    }
}

/// Factory for [`FvaAggregator`].
pub struct FvaFactory {
    /// Flat funding spread applied to the simulated exposure profile.
    pub funding_spread: f64,
    /// Number of Monte Carlo paths used to normalize the aggregate.
    pub n_paths: usize,
    /// System-curve discount factors `DF(0, t_d)` at the simulation dates.
    pub system_dfs: Option<Vec<f64>>,
}

impl PfeAggregatorFactory for FvaFactory {
    fn name(&self) -> &'static str {
        "FVA"
    }

    fn create_aggregator(&self, _ref_date: Date, _dates: &[Date]) -> AggregatorBundle {
        let fs = DualFwd::new(self.funding_spread);
        let mut agg = FvaAggregator::new(fs, self.n_paths);
        if let Some(dfs) = &self.system_dfs {
            agg = agg.with_system_discounts(dfs.clone());
        }
        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves: vec![("FVA.funding_spread".to_string(), fs)],
        }
    }
}

/// Factory for an FVA aggregator driven by a funding spread term structure.
///
/// Spreads at the simulation dates are linearly interpolated between the
/// pillars (flat extrapolation on both sides). The pillar spreads become
/// tracked tape leaves so that the backward pass yields FVA sensitivities
/// to the individual pillars (labels are prefixed with `"FVA."`).
pub struct FundingCurveFvaFactory {
    /// Pillar dates of the funding spread curve.
    pub pillar_dates: Vec<Date>,
    /// Funding spreads at the pillar dates.
    pub pillar_spreads: Vec<f64>,
    /// Labels of the pillar spreads (e.g. funding curve quote identifiers).
    pub pillar_labels: Vec<String>,
    /// Pillar dates of an additive overlay spread curve (e.g. an explicit
    /// funding spread over the funding index). Empty when unused.
    pub overlay_dates: Vec<Date>,
    /// Overlay spreads at the overlay pillar dates.
    pub overlay_spreads: Vec<f64>,
    /// Labels of the overlay pillar spreads.
    pub overlay_labels: Vec<String>,
    /// Number of Monte Carlo paths.
    pub n_paths: usize,
    /// Day counter used to convert dates into year fractions.
    pub day_counter: DayCounter,
    /// System-curve discount factors `DF(0, t_d)` at the simulation dates.
    pub system_dfs: Option<Vec<f64>>,
}

impl FundingCurveFvaFactory {
    /// Linear spread interpolation at time `t` (year fraction from the
    /// reference date), with flat extrapolation on both sides.
    fn spread_at(t: f64, pillar_times: &[f64], leaves: &[DualFwd]) -> DualFwd {
        let n = pillar_times.len();
        if n == 0 {
            return DualFwd::scalar(0.0);
        }
        if t <= pillar_times[0] {
            return leaves[0];
        }
        for k in 1..n {
            if t <= pillar_times[k] {
                let w = (t - pillar_times[k - 1]) / (pillar_times[k] - pillar_times[k - 1]);
                return leaves[k - 1]
                    .add_val(leaves[k].sub_val(leaves[k - 1]).mul_val(DualFwd::scalar(w)));
            }
        }
        leaves[n - 1]
    }
}

impl PfeAggregatorFactory for FundingCurveFvaFactory {
    fn name(&self) -> &'static str {
        "FVA"
    }

    fn create_aggregator(&self, ref_date: Date, dates: &[Date]) -> AggregatorBundle {
        // Tracked leaves: one per pillar spread (base curve + overlay).
        let pillar_leaves: Vec<DualFwd> = self
            .pillar_spreads
            .iter()
            .map(|s| DualFwd::new(*s))
            .collect();
        let overlay_leaves: Vec<DualFwd> = self
            .overlay_spreads
            .iter()
            .map(|s| DualFwd::new(*s))
            .collect();

        let pillar_times: Vec<f64> = self
            .pillar_dates
            .iter()
            .map(|d| self.day_counter.year_fraction(ref_date, *d))
            .collect();
        let overlay_times: Vec<f64> = self
            .overlay_dates
            .iter()
            .map(|d| self.day_counter.year_fraction(ref_date, *d))
            .collect();

        let spreads: Vec<DualFwd> = dates
            .iter()
            .map(|d| {
                let t = self.day_counter.year_fraction(ref_date, *d);
                let base = Self::spread_at(t, &pillar_times, &pillar_leaves);
                if overlay_leaves.is_empty() {
                    base
                } else {
                    base.add_val(Self::spread_at(t, &overlay_times, &overlay_leaves))
                }
            })
            .collect();

        let mut agg = FvaAggregator::from_spreads(spreads, self.n_paths);
        if let Some(dfs) = &self.system_dfs {
            agg = agg.with_system_discounts(dfs.clone());
        }

        let leaves: Vec<(String, DualFwd)> = self
            .pillar_labels
            .iter()
            .zip(&pillar_leaves)
            .chain(self.overlay_labels.iter().zip(&overlay_leaves))
            .map(|(label, leaf)| (format!("FVA.{label}"), *leaf))
            .collect();

        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ad::tape::Tape, time::enums::TimeUnit};

    const DC: DayCounter = DayCounter::Actual365;

    fn sim_dates(ref_date: Date, months: &[i32]) -> Vec<Date> {
        std::iter::once(ref_date)
            .chain(
                months
                    .iter()
                    .map(|m| ref_date.advance(*m, TimeUnit::Months)),
            )
            .collect()
    }

    /// For constant positive exposure `E`, the CVA sum telescopes exactly to
    /// `E · LGD · (1 − S(t_last))` with `S(t) = exp(−(s/LGD)·t)`.
    #[test]
    fn cva_constant_exposure_telescopes() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[3, 6, 9, 12, 24, 36]);
        let (spread, recovery, exposure) = (0.02_f64, 0.4_f64, 1_000.0_f64);
        let lgd = 1.0 - recovery;

        let agg = CvaAggregator::<f64>::new(spread, recovery, 1, ref_date, &dates);
        let npvs = vec![exposure; dates.len()];
        let cva = agg.aggregate_path(&npvs, &dates);

        let t_last = DC.year_fraction(ref_date, dates[dates.len() - 1]);
        let expected = exposure * lgd * (1.0 - (-(spread / lgd) * t_last).exp());
        assert!(
            (cva - expected).abs() < 1e-12 * expected,
            "telescoping CVA {cva} vs analytic {expected}"
        );
    }

    /// Negative exposure never contributes to CVA.
    #[test]
    fn cva_negative_exposure_is_zero() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[6, 12, 24]);
        let agg = CvaAggregator::<f64>::new(0.02, 0.4, 1, ref_date, &dates);
        let npvs = vec![-500.0; dates.len()];
        assert!(agg.aggregate_path(&npvs, &dates).abs() < 1e-15);
    }

    /// DVA of `−E` must equal CVA of `+E` for the same spread/recovery.
    #[test]
    fn dva_mirrors_cva() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[3, 12, 30, 60]);
        let cva = CvaAggregator::<f64>::new(0.015, 0.35, 4, ref_date, &dates);
        let dva = DvaAggregator::<f64>::new(0.015, 0.35, 4, ref_date, &dates);
        let pos = vec![750.0; dates.len()];
        let neg: Vec<f64> = pos.iter().map(|v| -v).collect();
        let c = cva.aggregate_path(&pos, &dates);
        let d = dva.aggregate_path(&neg, &dates);
        assert!(
            (c - d).abs() < 1e-14 * c.abs(),
            "CVA {c} vs mirrored DVA {d}"
        );
    }

    /// Constant NPV `E`: FVA telescopes to `E·f·(t_last − t_0)`, and all-ones
    /// system discounts must be a no-op.
    #[test]
    fn fva_constant_npv_analytic() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[6, 12, 24, 48]);
        let (funding_spread, exposure) = (0.005_f64, 2_000.0_f64);
        let npvs = vec![exposure; dates.len()];

        let agg = FvaAggregator::<f64>::new(funding_spread, 1);
        let fva = agg.aggregate_path(&npvs, &dates);
        let t_last = DC.year_fraction(ref_date, dates[dates.len() - 1]);
        let expected = exposure * funding_spread * t_last;
        assert!(
            (fva - expected).abs() < 1e-12 * expected,
            "FVA {fva} vs analytic {expected}"
        );

        let discounted = FvaAggregator::<f64>::new(funding_spread, 1)
            .with_system_discounts(vec![1.0; dates.len()])
            .aggregate_path(&npvs, &dates);
        assert!((discounted - fva).abs() < 1e-15 * fva.abs());
    }

    /// System discounting scales each bucket by `DF(t_d)`; verify against a
    /// direct hand-computed sum.
    #[test]
    fn cva_system_discounts_apply_per_bucket() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[12, 24, 36]);
        let (spread, recovery, exposure, r) = (0.02_f64, 0.4_f64, 1_000.0_f64, 0.03_f64);
        let lgd = 1.0 - recovery;
        let hazard = spread / lgd;

        let dfs: Vec<f64> = dates
            .iter()
            .map(|d| (-r * DC.year_fraction(ref_date, *d)).exp())
            .collect();
        let agg = CvaAggregator::<f64>::new(spread, recovery, 1, ref_date, &dates)
            .with_system_discounts(dfs.clone());
        let npvs = vec![exposure; dates.len()];
        let cva = agg.aggregate_path(&npvs, &dates);

        let mut expected = 0.0;
        let mut s_prev = 1.0;
        for (d, date) in dates.iter().enumerate().skip(1) {
            let t = DC.year_fraction(ref_date, *date);
            let s = (-hazard * t).exp();
            expected += exposure * dfs[d] * (s_prev - s);
            s_prev = s;
        }
        expected *= lgd;
        assert!(
            (cva - expected).abs() < 1e-12 * expected,
            "discounted CVA {cva} vs hand-computed {expected}"
        );
    }

    /// The curve-driven factory with flat-hazard pillar survivals must agree
    /// with the flat-spread [`CvaFactory`] (log-linear interpolation is exact
    /// for a flat hazard), covering interpolation before the first pillar,
    /// between pillars and flat-hazard extrapolation beyond the last pillar.
    #[test]
    fn credit_curve_factory_matches_flat_hazard() {
        let ref_date = Date::new(2025, 1, 2);
        // Sim dates: before first pillar (3M), between pillars, beyond last (7Y).
        let dates = sim_dates(ref_date, &[3, 9, 18, 30, 54, 84]);
        let (lambda, recovery, n_paths) = (0.04_f64, 0.4_f64, 2_usize);
        let lgd = 1.0 - recovery;

        let pillar_dates: Vec<Date> = [1, 3, 5]
            .iter()
            .map(|y| ref_date.advance(*y, TimeUnit::Years))
            .collect();
        let pillar_survivals: Vec<f64> = pillar_dates
            .iter()
            .map(|d| (-lambda * DC.year_fraction(ref_date, *d)).exp())
            .collect();

        let curve_factory = CreditCurveCvaFactory {
            pillar_dates,
            pillar_survivals,
            pillar_labels: vec!["1Y".into(), "3Y".into(), "5Y".into()],
            recovery,
            n_paths,
            day_counter: DC,
            system_dfs: None,
        };
        let flat_factory = CvaFactory {
            credit_spread: lambda * lgd,
            recovery,
            n_paths,
            system_dfs: None,
        };

        Tape::start_recording_fwd();
        let curve_bundle = curve_factory.create_aggregator(ref_date, &dates);
        let flat_bundle = flat_factory.create_aggregator(ref_date, &dates);
        Tape::set_mark_fwd();

        let npvs: Vec<DualFwd> = dates.iter().map(|_| DualFwd::scalar(1_000.0)).collect();
        let from_curve = curve_bundle
            .aggregator
            .aggregate_path(&npvs, &dates)
            .value();
        let from_flat = flat_bundle.aggregator.aggregate_path(&npvs, &dates).value();
        Tape::stop_recording_fwd();

        assert!(from_curve > 0.0);
        assert!(
            (from_curve - from_flat).abs() < 1e-10 * from_flat,
            "curve-driven CVA {from_curve} vs flat-spread CVA {from_flat}"
        );
        assert!(
            curve_bundle.leaves.iter().any(|(l, _)| l == "CVA.5Y"),
            "pillar leaves must be labeled with the quote ids"
        );
    }

    /// Single-pillar curve: extrapolation beyond the pillar must use the flat
    /// hazard implied from `(0, 1) → (t_1, S_1)`.
    #[test]
    fn single_pillar_flat_hazard_extrapolation() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[6, 12, 36]); // 3Y is beyond the 1Y pillar
        let (lambda, recovery) = (0.05_f64, 0.4_f64);

        let pillar_date = ref_date.advance(1, TimeUnit::Years);
        let s1 = (-lambda * DC.year_fraction(ref_date, pillar_date)).exp();
        let factory = CreditCurveCvaFactory {
            pillar_dates: vec![pillar_date],
            pillar_survivals: vec![s1],
            pillar_labels: vec!["1Y".into()],
            recovery,
            n_paths: 1,
            day_counter: DC,
            system_dfs: None,
        };
        let flat = CvaFactory {
            credit_spread: lambda * (1.0 - recovery),
            recovery,
            n_paths: 1,
            system_dfs: None,
        };

        Tape::start_recording_fwd();
        let curve_bundle = factory.create_aggregator(ref_date, &dates);
        let flat_bundle = flat.create_aggregator(ref_date, &dates);
        Tape::set_mark_fwd();
        let npvs: Vec<DualFwd> = dates.iter().map(|_| DualFwd::scalar(500.0)).collect();
        let from_curve = curve_bundle
            .aggregator
            .aggregate_path(&npvs, &dates)
            .value();
        let from_flat = flat_bundle.aggregator.aggregate_path(&npvs, &dates).value();
        Tape::stop_recording_fwd();

        assert!(
            (from_curve - from_flat).abs() < 1e-10 * from_flat,
            "single-pillar extrapolated CVA {from_curve} vs flat {from_flat}"
        );
    }

    /// A flat funding spread term structure must reproduce the flat-spread
    /// FVA exactly, and the pillar leaves must be labeled.
    #[test]
    fn funding_curve_flat_spreads_match_flat_fva() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[6, 12, 24, 48]);
        let spread = 0.005_f64;

        let curve_factory = FundingCurveFvaFactory {
            pillar_dates: vec![
                ref_date.advance(1, TimeUnit::Years),
                ref_date.advance(3, TimeUnit::Years),
            ],
            pillar_spreads: vec![spread, spread],
            pillar_labels: vec!["1Y".into(), "3Y".into()],
            overlay_dates: vec![],
            overlay_spreads: vec![],
            overlay_labels: vec![],
            n_paths: 1,
            day_counter: DC,
            system_dfs: None,
        };
        let flat_factory = FvaFactory {
            funding_spread: spread,
            n_paths: 1,
            system_dfs: None,
        };

        Tape::start_recording_fwd();
        let curve_bundle = curve_factory.create_aggregator(ref_date, &dates);
        let flat_bundle = flat_factory.create_aggregator(ref_date, &dates);
        Tape::set_mark_fwd();
        let npvs: Vec<DualFwd> = dates.iter().map(|_| DualFwd::scalar(2_000.0)).collect();
        let from_curve = curve_bundle
            .aggregator
            .aggregate_path(&npvs, &dates)
            .value();
        let from_flat = flat_bundle.aggregator.aggregate_path(&npvs, &dates).value();
        Tape::stop_recording_fwd();

        assert!(
            (from_curve - from_flat).abs() < 1e-12 * from_flat.abs(),
            "curve-driven FVA {from_curve} vs flat FVA {from_flat}"
        );
        assert!(
            curve_bundle.leaves.iter().any(|(l, _)| l == "FVA.1Y"),
            "pillar leaves must be labeled with the pillar ids"
        );
    }

    /// Time-dependent spreads: each accrual bucket must use the linearly
    /// interpolated spread at the bucket's right endpoint.
    #[test]
    fn funding_curve_time_dependent_spreads_analytic() {
        let ref_date = Date::new(2025, 1, 2);
        let dates = sim_dates(ref_date, &[12, 24, 36]);
        let exposure = 1_000.0_f64;

        // Pillars at 1Y and 3Y: spread ramps from 0.002 to 0.006.
        let pillar_dates = vec![dates[1], dates[3]];
        let (s1, s3) = (0.002_f64, 0.006_f64);
        let factory = FundingCurveFvaFactory {
            pillar_dates: pillar_dates.clone(),
            pillar_spreads: vec![s1, s3],
            pillar_labels: vec!["1Y".into(), "3Y".into()],
            overlay_dates: vec![],
            overlay_spreads: vec![],
            overlay_labels: vec![],
            n_paths: 1,
            day_counter: DC,
            system_dfs: None,
        };

        Tape::start_recording_fwd();
        let bundle = factory.create_aggregator(ref_date, &dates);
        Tape::set_mark_fwd();
        let npvs: Vec<DualFwd> = dates.iter().map(|_| DualFwd::scalar(exposure)).collect();
        let fva = bundle.aggregator.aggregate_path(&npvs, &dates).value();
        Tape::stop_recording_fwd();

        // Expected: sum over buckets of E · s(t_d) · Δt with s linearly
        // interpolated between the pillars (flat outside).
        let times: Vec<f64> = dates
            .iter()
            .map(|d| DC.year_fraction(ref_date, *d))
            .collect();
        let (t1, t3) = (
            DC.year_fraction(ref_date, pillar_dates[0]),
            DC.year_fraction(ref_date, pillar_dates[1]),
        );
        let spread_at = |t: f64| {
            if t <= t1 {
                s1
            } else if t >= t3 {
                s3
            } else {
                s1 + (s3 - s1) * (t - t1) / (t3 - t1)
            }
        };
        let expected: f64 = (1..times.len())
            .map(|d| exposure * spread_at(times[d]) * (times[d] - times[d - 1]))
            .sum();

        assert!(
            (fva - expected).abs() < 1e-12 * expected,
            "time-dependent FVA {fva} vs analytic {expected}"
        );
    }
}
