use serde::{Deserialize, Serialize};

use crate::{indices::marketindex::MarketIndex, time::date::Date};

/// Contractual loan coupon definition, independent of payment and reset frequencies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LoanRateDefinition {
    /// Annual fixed coupon.
    Fixed(f64),
    /// Index fixing plus annual spread.
    Floating {
        /// Floating-rate index.
        index: MarketIndex,
        /// Annual additive spread.
        spread: f64,
    },
    /// Fixed until the switch date, then index plus spread.
    Mixed {
        /// Annual coupon before the switch.
        fixed_rate: f64,
        /// First floating accrual date, including a mid-payment transition.
        switch_date: Date,
        /// Floating-rate index after the switch.
        index: MarketIndex,
        /// Annual additive spread after the switch.
        spread: f64,
    },
}
