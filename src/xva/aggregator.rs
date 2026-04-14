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
    fn name(&self) -> &str;

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
    fn name(&self) -> &str;

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
            inv_n: 1.0 / n_paths as f64,
        }
    }
}

impl<T: Scalar> PfeAggregator<T> for CvaAggregator<T> {
    fn name(&self) -> &str {
        "CVA"
    }

    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T {
        let mut c_p = T::zero();
        for d in 1..dates.len().min(npvs.len()) {
            let exposure = npvs[d].max_val(T::zero());
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
    inv_n: f64,
}

impl<T: Scalar> DvaAggregator<T> {
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
            inv_n: 1.0 / n_paths as f64,
        }
    }
}

impl<T: Scalar> PfeAggregator<T> for DvaAggregator<T> {
    fn name(&self) -> &str {
        "DVA"
    }

    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T {
        let mut d_p = T::zero();
        for d in 1..dates.len().min(npvs.len()) {
            let exposure = npvs[d].neg_val().max_val(T::zero());
            let delta_pd = self.survival_probs[d - 1].sub_val(self.survival_probs[d]);
            d_p = d_p.add_val(exposure.mul_val(delta_pd));
        }
        d_p.mul_val(self.lgd).mul_val(T::scalar(self.inv_n))
    }
}

/// Funding valuation adjustment aggregator.
pub struct FvaAggregator<T: Scalar> {
    funding_spread: T,
    inv_n: f64,
}

impl<T: Scalar> FvaAggregator<T> {
    pub fn new(funding_spread: T, n_paths: usize) -> Self {
        Self {
            funding_spread,
            inv_n: 1.0 / n_paths as f64,
        }
    }
}

impl<T: Scalar> PfeAggregator<T> for FvaAggregator<T> {
    fn name(&self) -> &str {
        "FVA"
    }

    fn aggregate_path(&self, npvs: &[T], dates: &[Date]) -> T {
        let dc = DayCounter::Actual365;
        let mut f_p = T::zero();
        for d in 1..dates.len().min(npvs.len()) {
            let dt = dc.year_fraction(dates[d - 1], dates[d]);
            f_p = f_p.add_val(npvs[d].mul_val(self.funding_spread).mul_val(T::scalar(dt)));
        }
        f_p.mul_val(T::scalar(self.inv_n))
    }
}

/// Factory for [`CvaAggregator`].
pub struct CvaFactory {
    pub credit_spread: f64,
    pub recovery: f64,
    pub n_paths: usize,
}

impl PfeAggregatorFactory for CvaFactory {
    fn name(&self) -> &str {
        "CVA"
    }

    fn create_aggregator(&self, ref_date: Date, dates: &[Date]) -> AggregatorBundle {
        let cs = DualFwd::new(self.credit_spread);
        let rec = DualFwd::new(self.recovery);
        let agg = CvaAggregator::new(cs, rec, self.n_paths, ref_date, dates);
        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves: vec![
                ("CVA.credit_spread".to_string(), cs),
                ("CVA.recovery".to_string(), rec),
            ],
        }
    }
}

/// Factory for [`DvaAggregator`].
pub struct DvaFactory {
    pub own_spread: f64,
    pub own_recovery: f64,
    pub n_paths: usize,
}

impl PfeAggregatorFactory for DvaFactory {
    fn name(&self) -> &str {
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
    pub funding_spread: f64,
    pub n_paths: usize,
}

impl PfeAggregatorFactory for FvaFactory {
    fn name(&self) -> &str {
        "FVA"
    }

    fn create_aggregator(&self, _ref_date: Date, _dates: &[Date]) -> AggregatorBundle {
        let fs = DualFwd::new(self.funding_spread);
        let agg = FvaAggregator::new(fs, self.n_paths);
        AggregatorBundle {
            aggregator: Box::new(agg),
            leaves: vec![("FVA.funding_spread".to_string(), fs)],
        }
    }
}
