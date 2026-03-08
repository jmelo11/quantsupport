use crate::{
    core::{
        contextmanager::ContextManager, evaluationresults::EvaluationResults, request::Request,
    },
    utils::errors::Result,
};
/// # `Visitor`
pub trait Visitor {}

/// A [`Visitable`] struct.
pub trait Visitable<P: Visitor> {
    /// Accepts a visitor.
    ///
    /// ## Errors
    /// Returns an [`crate::utils::errors::QSError`] if the visit operation fails.
    fn accept(
        &self,
        visitor: &P,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults>;
}
