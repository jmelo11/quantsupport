use serde::{Deserialize, Serialize};

use crate::{
    ad::scalar::Scalar,
    utils::errors::{QSError, Result},
};

/// Contractual loan amortization before conditional prepayment and default.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LoanRepaymentStructure {
    /// Interest only, with all surviving principal due at maturity.
    Bullet,
    /// Level scheduled instalments, recalculated at coupon/reset changes.
    /// Stub periods retain the same instalment with actual interest accrual.
    French,
    /// French instalments leaving a fraction of original principal for maturity.
    Balloon {
        /// Residual fraction in [0, 1], reduced proportionally by prepayments.
        residual_fraction: f64,
    },
}

/// Calculates a level instalment with an optional residual principal payment.
/// Uses the supplied period rates, retaining scalar derivatives through balances.
///
/// # Errors
/// Rejects nonfinite period rates, nonpositive compounding factors or annuity factors,
/// and a nonfinite installment.
pub fn level_installment<N: Scalar>(
    principal: N,
    residual: N,
    period_rates: impl Iterator<Item = N>,
) -> Result<N> {
    let mut discount = N::one();
    let mut annuity = N::zero();
    for rate in period_rates {
        let compound = N::one().add_val(rate);
        if !compound.value().is_finite() || compound.value() <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Invalid loan amortization rate".into(),
            ));
        }
        discount = discount.div_val(compound);
        annuity = annuity.add_val(discount);
    }
    let payment = principal
        .sub_val(residual.mul_val(discount))
        .div_val(annuity);
    if !payment.value().is_finite() || annuity.value() <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Invalid loan annuity factor".into(),
        ));
    }
    Ok(payment)
}
