use crate::{
    core::{
        pricingcontext::PricingContext, evaluationresults::EvaluationResults, request::Request,
    },
    utils::errors::Result,
};

/// A marker trait for visitor types.
pub trait Visitor {}

/// A [`Visitable`] struct. This is reserved for later use.
pub trait Visitable<P: Visitor> {
    /// Accepts a visitor.
    ///
    /// ## Errors
    /// Returns a [`QSError`](crate::utils::errors::QSError) if the visit operation fails.
    fn accept(
        &self,
        visitor: &P,
        requests: &[Request],
        ctx: &PricingContext,
    ) -> Result<EvaluationResults>;
}
