//! Claim compression preprocessor.
//!
//! [`ClaimCompressionPreprocessor`] merges claims with identical
//! characteristics into single combined claims, reducing the number
//! of evaluations in the Monte Carlo loop.
//!
//! Currently merges:
//! - **Deterministic** claims sharing `(payment_date, currency,
//!   foreign_currency)`: their effective amounts
//!   `notional × side.sign() × amount` are summed into one claim.

use std::collections::HashMap;

use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    time::date::Date,
    xva::{claimevaluationstrategy::ClaimEvaluationStrategy, contigentclaim::ContingentClaim},
};

/// Compresses a set of claims by merging those with identical economic
/// characteristics.
pub struct ClaimCompressionPreprocessor;

impl ClaimCompressionPreprocessor {
    /// Compresses claims in-place, merging compatible deterministic
    /// cashflows.
    ///
    /// After compression the vector may be shorter. Global indices are
    /// **not** assigned here — that happens in the
    /// [`PreprocessorExecutor`](super::preprocessorexecutor::PreprocessorExecutor)
    /// visit pass that follows.
    pub fn compress(claims: &mut Vec<ContingentClaim>) {
        // Partition into deterministic (compressible) and others.
        let mut deterministic: Vec<ContingentClaim> = Vec::new();
        let mut others: Vec<ContingentClaim> = Vec::new();

        for claim in claims.drain(..) {
            if matches!(
                claim.evaluation_strategy(),
                ClaimEvaluationStrategy::Deterministic { .. }
            ) {
                deterministic.push(claim);
            } else {
                others.push(claim);
            }
        }

        // Group deterministic claims by (payment_date, currency, foreign_currency).
        type Key = (Date, Currency, Option<Currency>);
        let mut groups: HashMap<Key, Vec<ContingentClaim>> = HashMap::new();

        for claim in deterministic {
            let key = (
                claim.payment_date(),
                claim.currency(),
                claim.foreign_currency(),
            );
            groups.entry(key).or_default().push(claim);
        }

        // Merge each group.
        for ((payment_date, currency, foreign_currency), group) in groups {
            if group.len() == 1 {
                // No merging needed — keep the original claim.
                claims.extend(group);
            } else {
                // Sum effective amounts: notional × side × amount.
                let mut effective = 0.0_f64;
                for c in &group {
                    if let ClaimEvaluationStrategy::Deterministic { amount } =
                        c.evaluation_strategy()
                    {
                        effective += c.notional() * c.side().sign() * amount;
                    }
                }

                // Create a single merged claim.
                let merged = ContingentClaim::new(
                    "compressed".to_string(),
                    0,
                    payment_date,
                    None,
                    None,
                    None,
                    currency,
                    foreign_currency,
                    1.0,
                    Side::LongReceive,
                    ClaimEvaluationStrategy::Deterministic { amount: effective },
                    None,
                );
                claims.push(merged);
            }
        }

        // Append the non-deterministic claims unchanged.
        claims.extend(others);
    }
}
