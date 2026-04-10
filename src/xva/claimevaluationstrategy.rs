use crate::{
    instruments::cashflows::payoffops::PayoffOps,
    time::{date::Date, daycounter::DayCounter},
    xva::contigentclaim::ContingentClaim,
};

/// How to reduce multiple observations into a single fixing.
pub enum PathAggregator {
    ArithmeticMean,
    GeometricMean,
    Max,
    Min,
    Sum,
}

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
