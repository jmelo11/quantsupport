use crate::{
    core::{
        contextmanager::ContextManager, evaluationresults::EvaluationResults, request::Request,
    },
    utils::errors::QSError,
};
/// # `Visitor`
pub trait Visitor {}

/// A [`Visitable`] struct.
pub trait Visitable<P: Visitor> {
    /// Accepts a visitor.
    ///
    /// ## Errors
    /// Returns an [`QSError`] if the visit operation fails.
    fn accept(
        &self,
        visitor: &P,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults, QSError>;
}

// pub trait VisitExampleTrade: Visitor {
//     fn visit_example(
//         &self,
//         trade: &ExampleInstrumentTrade,
//         requests: &[Request],
//         ctx: &ContextManager,
//     ) -> Result<RiskMetrics, PricingError>;
//     // fn visit_bond
//     // fn visit_option
//     // fn visit_callable_bond
// }
