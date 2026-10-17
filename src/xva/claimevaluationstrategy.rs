//! Evaluation strategies for contingent claims.
//!
//! Each variant of [`ClaimEvaluationStrategy`] describes *how* a claim's
//! raw value is computed from simulated market data before notional,
//! discounting, and FX conversion are applied.

use crate::{
    instruments::cashflows::payoffops::PayoffOps,
    scripting::product::ScriptedPayoff,
    time::{date::Date, daycounter::DayCounter},
    xva::contigentclaim::ContingentClaim,
};

/// Aggregation method for reducing multiple observations into a single value.
///
/// Describes the requested reduction for [`ClaimEvaluationStrategy::PathDependent`]
/// (e.g. arithmetic mean for Asian options).
/// This is currently stored metadata: `ContingentClaim::evaluate` consumes
/// an already resolved observation and does not execute the aggregator.
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
/// | [`Scripted`](Self::Scripted) | Conditional or path-dependent scripted payments |
pub enum ClaimEvaluationStrategy {
    /// Known amount (fixed coupons, redemptions, disbursements).
    Deterministic {
            /// Raw amount per claim notional; claim evaluation multiplies by notional again.
        amount: f64,
    },

    /// Linear in a single rate fixing over an accrual period.
    /// raw = (fixing + spread) × τ
    /// e.g. floating rate coupon
    LinearRate {
        /// Additive spread over the index rate.
        spread: f64,
        /// Day-count convention for accrual factor τ.
        day_counter: DayCounter,
    },

    /// Non-linear in a single rate fixing over an accrual period.
    /// raw = payoff(fixing + spread) × τ
    /// e.g. caplet, floorlet, digital coupon
    NonLinearRate {
        /// Payoff function (call, put, digital, etc.).
        payoff_ops: PayoffOps,
        /// Additive spread over the index rate.
        spread: f64,
        /// Stored strike metadata; evaluation uses the strike encoded in `payoff_ops`.
        strike: f64,
        /// Day-count convention for accrual factor τ.
        day_counter: DayCounter,
    },

    /// Payoff on a single spot observation (no accrual period).
    /// raw = payoff(S)
    /// e.g. equity call, FX option, binary
    SpotPayoff {
        /// Payoff function.
        payoff_ops: PayoffOps,
        /// Stored strike metadata; evaluation uses the strike encoded in `payoff_ops`.
        strike: f64,
        /// Date the spot is observed.
        observation_date: Date,
    },

    /// Path-dependent: payoff depends on multiple observations.
    /// e.g. Asian option, lookback, cliquet
    PathDependent {
        /// Dates at which the underlying is observed.
        observation_dates: Vec<Date>,
        /// Requested aggregation metadata, not executed by the basic claim evaluator.
        aggregator: PathAggregator,
        /// Payoff function applied to the aggregated value.
        payoff_ops: PayoffOps,
        /// Stored strike metadata; evaluation uses the strike encoded in `payoff_ops`.
        strike: f64,
    },

    /// Exercise metadata wrapping an inner claim.
    ///
    /// The basic evaluator always evaluates the inner claim; it does not solve
    /// an exercise policy or gate payment on an exercise decision.
    ExerciseContingent {
        /// Date the exercise may occur.
        exercise_date: Date,
        /// Group id for co-terminal exercise decisions.
        exercise_group: usize,
        /// Inner claim realised upon exercise.
        inner: Box<ContingentClaim>,
    },

    /// Raw payoff produced by one indexed `pays` expression in a script.
    Scripted {
        /// Compiled script and the payment to capture when it is evaluated.
        payoff: ScriptedPayoff,
    },
}
