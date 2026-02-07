use crate::{
    core::{
        evaluationresults::EvaluationResults, pricingdata::PricingDataContext,
        pricingrequest::PricingRequest,
    },
    utils::errors::AtlasError,
};
/// # `Visitor`
pub trait Visitor {}

/// # `Visitable`
pub trait Visitable<P: Visitor> {
    /// Accepts a visitor.
    ///
    /// ## Errors
    /// Returns an [`AtlasError`] if the visit operation fails.
    fn accept(
        &self,
        visitor: &P,
        requests: &[PricingRequest],
        ctx: &PricingDataContext,
    ) -> Result<EvaluationResults, AtlasError>;
}

// pub trait VisitExampleTrade: Visitor {
//     fn visit_example(
//         &self,
//         trade: &ExampleInstrumentTrade,
//         requests: &[PricingRequest],
//         ctx: &PricingDataContext,
//     ) -> Result<RiskMetrics, PricingError>;
//     // fn visit_bond
//     // fn visit_option
//     // fn visit_callable_bond
// }
