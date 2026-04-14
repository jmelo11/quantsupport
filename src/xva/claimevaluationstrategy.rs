//! Evaluation strategies for contingent claims.
//!
//! Each variant of [`ClaimEvaluationStrategy`] describes *how* a claim's
//! raw value is computed from simulated market data before notional,
//! discounting, and FX conversion are applied.

use crate::{
    instruments::cashflows::payoffops::PayoffOps,
    time::{date::Date, daycounter::DayCounter},
    xva::contigentclaim::ContingentClaim,
};

/// Aggregation method for reducing multiple observations into a single value.
///
/// Used by [`ClaimEvaluationStrategy::PathDependent`] to combine observations
/// along a simulated path (e.g. arithmetic mean for Asian options).
pub enum PathAggregator {
    /// Arithmetic mean of observations.
    ArithmeticMean,
    /// Geometric mean of observations.
    GeometricMean,
    /// Maximum observation.
    Max,
    /// Minimum observation.
    Min,
    /// Sum of observations.
    Sum,
}

/// Defines how the raw value of a [`ContingentClaim`] is computed from
/// simulated market data.
///
/// The evaluator calls [`ContingentClaim::evaluate`] which dispatches on
/// this enum.  Each variant describes a different payoff structure:
///
/// | Variant | Typical use |
/// |---------|-------------|
/// | [`Deterministic`](Self::Deterministic) | Fixed coupons, redemptions |
/// | [`LinearRate`](Self::LinearRate) | Floating-rate coupons |
/// | [`NonLinearRate`](Self::NonLinearRate) | Caps, floors, digitals |
/// | [`SpotPayoff`](Self::SpotPayoff) | Equity/FX options |
/// | [`PathDependent`](Self::PathDependent) | Asian, lookback |
/// | [`ExerciseContingent`](Self::ExerciseContingent) | Bermudans, callables |
pub enum ClaimEvaluationStrategy {
    /// Known amount (fixed coupons, redemptions, disbursements).
    Deterministic {
        /// Fixed cashflow amount.
        amount: f64,
    },

    /// Linear in a single rate fixing over an accrual period.
    /// value = notional × (fixing + spread) × τ
    /// e.g. floating rate coupon
    LinearRate {
        /// Additive spread over the index rate.
        spread: f64,
        /// Day-count convention for accrual factor τ.
        day_counter: DayCounter,
    },

    /// Non-linear in a single rate fixing over an accrual period.
    /// value = notional × payoff(fixing + spread) × τ
    /// e.g. caplet, floorlet, digital coupon
    NonLinearRate {
        /// Payoff function (call, put, digital, etc.).
        payoff_ops: PayoffOps,
        /// Additive spread over the index rate.
        spread: f64,
        /// Option strike.
        strike: f64,
        /// Day-count convention for accrual factor τ.
        day_counter: DayCounter,
    },

    /// Payoff on a single spot observation (no accrual period).
    /// value = notional × payoff(S)
    /// e.g. equity call, FX option, binary
    SpotPayoff {
        /// Payoff function.
        payoff_ops: PayoffOps,
        /// Option strike.
        strike: f64,
        /// Date the spot is observed.
        observation_date: Date,
    },

    /// Path-dependent: payoff depends on multiple observations.
    /// e.g. Asian option, lookback, cliquet
    PathDependent {
        /// Dates at which the underlying is observed.
        observation_dates: Vec<Date>,
        /// How observations are combined.
        aggregator: PathAggregator,
        /// Payoff function applied to the aggregated value.
        payoff_ops: PayoffOps,
        /// Option strike.
        strike: f64,
    },

    /// Exercise-contingent: conditional on an exercise decision.
    /// e.g. Bermudan swaption, callable bond
    ExerciseContingent {
        /// Date the exercise may occur.
        exercise_date: Date,
        /// Group id for co-terminal exercise decisions.
        exercise_group: usize,
        /// Inner claim realised upon exercise.
        inner: Box<ContingentClaim>,
    },
}
