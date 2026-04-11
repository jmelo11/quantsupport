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
    ArithmeticMean,
    GeometricMean,
    Max,
    Min,
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
    Deterministic { amount: f64 },

    /// Linear in a single rate fixing over an accrual period.
    /// value = notional × (fixing + spread) × τ
    /// e.g. floating rate coupon
    LinearRate {
        spread: f64,
        day_counter: DayCounter,
    },

    /// Non-linear in a single rate fixing over an accrual period.
    /// value = notional × payoff(fixing + spread) × τ
    /// e.g. caplet, floorlet, digital coupon
    NonLinearRate {
        payoff_ops: PayoffOps,
        spread: f64,
        strike: f64,
        day_counter: DayCounter,
    },

    /// Payoff on a single spot observation (no accrual period).
    /// value = notional × payoff(S)
    /// e.g. equity call, FX option, binary
    SpotPayoff {
        payoff_ops: PayoffOps,
        strike: f64,
        observation_date: Date,
    },

    /// Path-dependent: payoff depends on multiple observations.
    /// e.g. Asian option, lookback, cliquet
    PathDependent {
        observation_dates: Vec<Date>,
        aggregator: PathAggregator,
        payoff_ops: PayoffOps,
        strike: f64,
    },

    /// Exercise-contingent: conditional on an exercise decision.
    /// e.g. Bermudan swaption, callable bond
    ExerciseContingent {
        exercise_date: Date,
        exercise_group: usize,
        inner: Box<ContingentClaim>,
    },
}
