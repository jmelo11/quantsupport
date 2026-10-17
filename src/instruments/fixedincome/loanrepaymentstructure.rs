use serde::{Deserialize, Serialize};

/// Contractual loan amortization before conditional prepayment and default.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LoanRepaymentStructure {
    /// Interest only, with all surviving principal due at maturity.
    Bullet,
    /// Constant instalment on actual contractual accrual dates between recasts.
    French,
    /// French instalments leaving a fraction of original principal for maturity.
    Balloon {
        /// Residual fraction in [0, 1], reduced proportionally by prepayments.
        residual_fraction: f64,
    },
}
