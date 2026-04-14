use crate::xva::contigentclaim::ContingentClaim;

/// A preprocessing step applied to each [`ContingentClaim`] before
/// simulation requests are collected.
pub trait ClaimPreprocessor {
    /// Mutate a single claim in place (e.g. resolve discount curve,
    /// set realized fixings, compress, …).
    fn process(&self, claim: &mut ContingentClaim);
}
