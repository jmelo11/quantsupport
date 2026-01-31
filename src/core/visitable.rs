use crate::{
    core::{
        evaluationresults::EvaluationResults, pricingcontext::PricingContext,
        pricingrequest::PricingRequest,
    },
    utils::errors::AtlasError,
};
/// # `Visitor`
pub trait Visitor {}

/// # `Visitable`
pub trait Visitable<P: Visitor> {
    /// Accepts a visitor.
    fn accept(
        &self,
        visitor: &P,
        requests: &[PricingRequest],
        ctx: &PricingContext,
    ) -> Result<EvaluationResults, AtlasError>;
}

// pub trait VisitExampleTrade: Visitor {
//     fn visit_example(
//         &self,
//         trade: &ExampleInstrumentTrade,
//         requests: &[PricingRequest],
//         ctx: &PricingContext,
//     ) -> Result<RiskMetrics, PricingError>;
//     // fn visit_bond
//     // fn visit_option
//     // fn visit_callable_bond
// }
