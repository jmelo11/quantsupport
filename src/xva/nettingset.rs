//! Netting set — a group of claims under a single netting agreement.
//!
//! A [`NettingSet`] holds a collection of [`ContingentClaim`]s that are
//! subject to the same ISDA master agreement (or equivalent bilateral
//! netting arrangement) and carries its own [`DiscountPolicy`] reflecting
//! the CSA collateral terms.

use crate::{core::collateral::DiscountPolicy, xva::contigentclaim::ContingentClaim};

/// A group of contingent claims under a single netting agreement.
///
/// Each netting set carries its own [`DiscountPolicy`] which determines
/// the discount curve used for each claim based on the collateral terms
/// of the CSA.
pub struct NettingSet {
    claims: Vec<ContingentClaim>,
    discount_policy: Box<dyn DiscountPolicy>,
}

impl NettingSet {
    /// Creates a new netting set from a vector of claims and a discount policy.
    #[must_use]
    pub fn new(claims: Vec<ContingentClaim>, discount_policy: Box<dyn DiscountPolicy>) -> Self {
        Self {
            claims,
            discount_policy,
        }
    }

    /// Returns the claims in this netting set.
    #[must_use]
    pub fn claims(&self) -> &[ContingentClaim] {
        &self.claims
    }

    /// Returns a mutable slice of the claims.
    pub fn claims_mut(&mut self) -> &mut [ContingentClaim] {
        &mut self.claims
    }

    /// Returns a mutable reference to the underlying claim vector
    /// (needed for compression which may change the length).
    pub(crate) const fn claims_vec_mut(&mut self) -> &mut Vec<ContingentClaim> {
        &mut self.claims
    }

    /// Returns a reference to the discount policy.
    #[must_use]
    pub fn discount_policy(&self) -> &dyn DiscountPolicy {
        &*self.discount_policy
    }

    /// Returns the discount policy and a mutable slice of claims
    /// simultaneously, avoiding double-borrow issues.
    pub fn discount_policy_and_claims_mut(
        &mut self,
    ) -> (&dyn DiscountPolicy, &mut [ContingentClaim]) {
        (&*self.discount_policy, &mut self.claims)
    }

    /// Consumes the netting set and returns its claims.
    #[must_use]
    pub fn into_claims(self) -> Vec<ContingentClaim> {
        self.claims
    }

    /// Number of claims in this netting set.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.claims.len()
    }

    /// Whether the netting set is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}
