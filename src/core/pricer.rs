use crate::{
    core::{
        evaluationresults::EvaluationResults, pricingdata::PricingDataContext,
        pricingrequest::PricingRequest,
    },
    utils::errors::AtlasError,
};

/// # `Pricer`
/// The `Pricer` trait should be implemented by any instrument pricing methodology. Implementers
/// must also implement [`Send`] and [`Sync`].
pub trait Pricer: Send + Sync {
    /// The associated instrument to be priced.
    type Item;
    ///
    /// Evaluates the instrument over a [`PricingRequest`] given a [`PricingDataContext`].
    ///
    /// ## Arguments
    /// * `trade`: the associated instrument that this pricer is capable of handeling.
    /// * `requests`: a slice containing the different [`PricingRequest`] that being required to resolve.
    /// * `ctx`: a [`PricingDataContext`].
    ///
    /// ## Returns
    /// Returns [`EvaluationResults`] if the evaluation succeded.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the evaluation fails.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[PricingRequest],
        ctx: &PricingDataContext,
    ) -> Result<EvaluationResults, AtlasError>;
}
