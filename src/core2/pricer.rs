use crate::{
    core2::{
        evaluationresults::EvaluationResults, pricingcontext::PricingContext,
        pricingrequest::PricingRequest,
    },
    prelude::AtlasError,
};

/// # `Pricer`
/// The `Pricer` trait should be implemented by any instrument pricing methodology. Implementers
/// must also implement [`Send`] and [`Sync`].
pub trait Pricer: Send + Sync {
    /// The associated instrument to be priced.
    type Item;
    ///
    /// Evaluates the instrument over a [`PricingRequest`] given a [`PricingContext`].
    ///
    /// # Arguments
    /// * `trade`: the associated instrument that this pricer is capable of handeling.
    /// * `requests`: a slice containing the different [`PricingRequest`] that being required to resolve.
    /// * `ctx`: a [`PricingContext`].
    ///
    /// # Returns
    /// Returns [`EvaluationResults`] if the evaluation succeded.
    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[PricingRequest],
        ctx: &PricingContext,
    ) -> Result<EvaluationResults, AtlasError>;
}
