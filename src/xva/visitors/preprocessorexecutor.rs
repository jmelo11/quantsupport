//! Claim inspection and market-data request collection.
//!
//! The [`PreprocessorExecutor`](crate::xva::visitors::preprocessorexecutor::PreprocessorExecutor) walks a set of [`NettingSet`](crate::xva::nettingset::NettingSet)s, running a
//! pipeline of [`ClaimPreprocessor`](crate::xva::visitors::claimpreprocessor::ClaimPreprocessor)s (fixing resolution, etc.),
//! optionally compressing claims, and then collecting each claim's
//! [`SimulationRequest`](crate::xva::visitors::preprocessorexecutor::SimulationRequest) into shared contiguous response blocks, assigning indices so
//! the evaluator can locate each claim's response within a scenario.
//!
//! Each [`NettingSet`](crate::xva::nettingset::NettingSet) carries its own [`DiscountPolicy`](crate::core::collateral::DiscountPolicy) which is used
//! to resolve the discount curve for claims within that set.

use std::collections::HashMap;

use crate::{
    core::marketdatahandling::{
        discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
        fxrequest::FxRequest, pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
    },
    time::date::Date,
    xva::nettingset::NettingSet,
};

use super::{
    claimcompressionpreprocessor::ClaimCompressionPreprocessor,
    claimpreprocessor::ClaimPreprocessor,
};

/// Declares the market data a [`ContingentClaim`](crate::xva::contigentclaim::ContingentClaim) needs for simulation.
///
/// Each field is optional — `None` means the claim does not require that
/// data category. The [`PreprocessorExecutor`] collects each claim's complete
/// response block and passes the flat list to the
/// [`MarketModel`](super::marketmodel::MarketModel).
#[derive(Default, Clone, PartialEq, Eq, Hash)]
pub struct SimulationRequest {
    /// Exclusive cutoff: inactive on and after this date.
    pub expiration_date: Option<Date>,
    /// Discount factor request.
    pub discount_request: Option<DiscountRequest>,
    /// Forward rate request.
    pub forward_rate_request: Option<ForwardRateRequest>,
    /// FX rate request.
    pub fx_request: Option<FxRequest>,
    /// Spot observation request.
    pub spot_request: Option<SpotRequest>,
    /// Path-dependent observation request.
    pub path_dependent_request: Option<PathDependentRequest>,
}

impl SimulationRequest {
    /// Returns whether the request is needed at `evaluation_date`.
    #[must_use]
    pub fn is_active_on(&self, evaluation_date: Date) -> bool {
        self.expiration_date
            .is_none_or(|expiration| evaluation_date < expiration)
    }
}

/// Collects [`SimulationRequest`]s from a set of [`NettingSet`]s.
///
/// The [`PreprocessorExecutor`] is the first step of the XVA pipeline.  For
/// each netting set it:
///
/// 1. Runs the per-claim [`ClaimPreprocessor`] pipeline.
/// 2. Optionally compresses compatible claims via
///    [`ClaimCompressionPreprocessor`].
/// 3. Resolves the discount curve using each netting set's own
///    [`DiscountPolicy`](crate::core::collateral::DiscountPolicy).
/// 4. Shares identical complete request blocks and assigns their flat-vector
///    indices, locating the corresponding [`SimulationResponse`](super::marketmodel::SimulationResponse).
pub struct PreprocessorExecutor {
    requests: Vec<SimulationRequest>,
    preprocessors: Vec<Box<dyn ClaimPreprocessor>>,
    compress: bool,
}

impl Default for PreprocessorExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PreprocessorExecutor {
    /// Creates a `PreprocessorExecutor` without any preprocessors.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: Vec::new(),
            preprocessors: Vec::new(),
            compress: false,
        }
    }

    /// Appends a preprocessor to the pipeline. Preprocessors run in
    /// the order they are added, before simulation requests are built.
    #[must_use]
    pub fn with_preprocessor(mut self, preprocessor: Box<dyn ClaimPreprocessor>) -> Self {
        self.preprocessors.push(preprocessor);
        self
    }

    /// Enables claim compression. Compatible deterministic cashflows
    /// within each netting set will be merged after per-claim
    /// preprocessing, reducing the number of evaluations in the
    /// Monte Carlo loop.
    #[must_use]
    pub const fn with_compression(mut self) -> Self {
        self.compress = true;
        self
    }

    /// Visits all netting sets, preprocessing claims and collecting
    /// simulation requests, reusing identical complete response blocks.
    ///
    /// For each netting set the pipeline is:
    /// 1. Per-claim preprocessing (fixing resolution, …).
    /// 2. Claim compression (if enabled).
    /// 3. Discount resolution via the netting set's own
    ///    [`DiscountPolicy`](crate::core::collateral::DiscountPolicy).
    /// 4. Index assignment into shared global response blocks.
    ///
    /// Sharing compares every request field, including expiration and observation
    /// dates. Claims retain their trade identity, amounts and evaluation strategy;
    /// only identical market observations share storage. Scripted response blocks
    /// remain contiguous and are shared only when their complete layouts match.
    ///
    /// Discount-policy errors are not propagated: the request then has no
    /// discount factor. Claim evaluation defaults a missing discount to one.
    pub fn visit<'a>(&mut self, netting_sets: impl IntoIterator<Item = &'a mut NettingSet>) {
        self.requests.clear();
        let mut blocks: HashMap<Vec<SimulationRequest>, usize> = HashMap::new();

        for ns in netting_sets {
            // Phase 1: per-claim preprocessing.
            for claim in ns.claims_mut() {
                for pp in &self.preprocessors {
                    pp.process(claim);
                }
            }

            // Phase 2: compression.
            if self.compress {
                ClaimCompressionPreprocessor::compress(ns.claims_vec_mut());
            }

            // Phase 3 + 4: discount resolution & index assignment.
            let (policy, claims) = ns.discount_policy_and_claims_mut();
            for claim in claims {
                let mut requests = claim.simulation_requests();
                for request in &mut requests {
                    request.expiration_date = Some(claim.payment_date());
                }
                if let Ok(discount_index) = policy.accept(claim) {
                    requests[0].discount_request =
                        Some(DiscountRequest::new(discount_index, claim.payment_date()));
                }
                let index = *blocks.entry(requests).or_insert_with_key(|block| {
                    let index = self.requests.len();
                    self.requests.extend_from_slice(block);
                    index
                });
                claim.set_idx(index);
            }
        }
    }

    /// Returns distinct response blocks in first-encounter order.
    #[must_use]
    pub fn requests(&self) -> &[SimulationRequest] {
        &self.requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{collateral::SingleCurveCSADiscountPolicy, trade::Side},
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        models::lgm::{lgmcomponents::LgmRateModel, lgmmarketmodel::LgmMarketModel},
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        scripting::{
            nodes::event::{CodedEvent, EventStream},
            product::ScriptedProduct,
        },
        time::daycounter::DayCounter,
        utils::errors::Result,
        xva::{
            claimevaluationstrategy::ClaimEvaluationStrategy,
            contigentclaim::ContingentClaim,
            visitors::{
                exposureevaluator::{ExposureEvaluator, ExposureResult},
                marketmodel::MarketModel,
            },
        },
    };
    use std::{iter::once, slice::from_ref};

    /// Builds distinct trades with repeated and different rate observations and expiries.
    fn claims() -> Vec<ContingentClaim> {
        let reference = Date::new(2025, 1, 1);
        let middle = Date::new(2025, 7, 1);
        let maturity = Date::new(2026, 1, 1);
        [
            ("fixed.a", maturity, None, 1_000_000.0, Side::LongReceive),
            ("fixed.b", maturity, None, 2_000_000.0, Side::PayShort),
            (
                "float.a",
                maturity,
                Some(reference),
                1_000_000.0,
                Side::LongReceive,
            ),
            (
                "float.b",
                maturity,
                Some(middle),
                3_000_000.0,
                Side::LongReceive,
            ),
            (
                "float.c",
                maturity,
                Some(reference),
                2_000_000.0,
                Side::PayShort,
            ),
            ("early", middle, None, 500_000.0, Side::LongReceive),
        ]
        .into_iter()
        .map(|(id, payment, start, notional, side)| {
            let strategy = if start.is_some() {
                ClaimEvaluationStrategy::LinearRate {
                    spread: 0.001,
                    day_counter: DayCounter::Actual360,
                }
            } else {
                ClaimEvaluationStrategy::Deterministic { amount: 0.02 }
            };
            ContingentClaim::new(
                id.into(),
                0,
                payment,
                start,
                start,
                start.map(|_| payment),
                Currency::USD,
                None,
                notional,
                side,
                strategy,
                start.map(|_| MarketIndex::SOFR),
            )
        })
        .collect()
    }

    /// Evaluates every trade/path/date through the ordinary cube-only pricing method.
    fn evaluate(
        requests: Vec<SimulationRequest>,
        claims: &[ContingentClaim],
    ) -> Result<ExposureResult> {
        let reference = Date::new(2025, 1, 1);
        let curve = FlatForwardTermStructure::new(reference, 0.04, RateDefinition::default());
        let mut model = LgmMarketModel::new(
            Currency::USD,
            MarketIndex::SOFR,
            reference,
            DayCounter::Actual365,
        )
        .with_n_paths(8)
        .with_seed(42);
        model.add_curve_model(MarketIndex::SOFR, LgmRateModel::new(0.05, 0.005, &curve));
        let dates = vec![reference, Date::new(2025, 7, 1), Date::new(2026, 1, 1)];
        model.set_evaluation_dates(dates.clone());
        model.set_requests(requests);
        let trades = claims
            .iter()
            .map(|claim| (claim.trade_id().to_string(), from_ref(claim)))
            .collect();
        ExposureEvaluator::<f64>::new(dates, &model).evaluate(&trades)
    }

    /// Checks every cube entry against independent, unshared request construction.
    #[test]
    fn shared_requests_preserve_trade_cubes_and_expiration() -> Result<()> {
        let mut shared = NettingSet::new(
            claims(),
            Box::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::SOFR,
                Currency::USD,
            )),
        );
        let mut processor = PreprocessorExecutor::new();
        processor.visit(once(&mut shared));
        assert_eq!(processor.requests().len(), 4);
        assert_eq!(shared.claims()[0].idx(), shared.claims()[1].idx());
        assert_eq!(shared.claims()[2].idx(), shared.claims()[4].idx());
        assert_ne!(shared.claims()[2].idx(), shared.claims()[3].idx());
        assert_ne!(shared.claims()[0].idx(), shared.claims()[5].idx());

        let mut unshared = claims();
        let mut requests = Vec::new();
        for claim in &mut unshared {
            claim.set_idx(requests.len());
            let mut block = claim.simulation_requests();
            for request in &mut block {
                request.expiration_date = Some(claim.payment_date());
            }
            block[0].discount_request = Some(DiscountRequest::new(
                MarketIndex::SOFR,
                claim.payment_date(),
            ));
            requests.extend(block);
        }
        assert_eq!(requests.len(), 6);
        let expected = evaluate(requests, &unshared)?;
        let actual = evaluate(processor.requests().to_vec(), shared.claims())?;
        assert_eq!(actual.cubes.len(), expected.cubes.len());
        for (expected, actual) in expected.cubes.iter().zip(&actual.cubes) {
            assert_eq!(actual.trade_id, expected.trade_id);
            assert_eq!(actual.dates, expected.dates);
            assert_eq!(actual.npvs, expected.npvs);
            assert!(actual.npvs.iter().all(|path| path.last() == Some(&0.0)));
        }
        Ok(())
    }

    /// Checks that different CSA discount curves cannot alias the same response block.
    #[test]
    fn request_sharing_respects_each_netting_sets_discount_policy() {
        let mut first = NettingSet::new(
            claims(),
            Box::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::SOFR,
                Currency::USD,
            )),
        );
        let mut second = NettingSet::new(
            claims(),
            Box::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::TermSOFR3m,
                Currency::USD,
            )),
        );
        let mut processor = PreprocessorExecutor::new();
        processor.visit([&mut first, &mut second]);
        assert_eq!(processor.requests().len(), 8);
        for (left, right) in first.claims().iter().zip(second.claims()) {
            assert_ne!(left.idx(), right.idx());
        }
        processor.visit(once(&mut second));
        assert_eq!(processor.requests().len(), 4);
        assert_eq!(second.claims()[0].idx(), Some(0));
    }

    /// Checks that sharing whole scripted blocks preserves their contiguous tail layouts.
    #[test]
    fn scripted_blocks_share_only_identical_complete_request_layouts() -> Result<()> {
        let reference = Date::new(2025, 1, 1);
        let maturity = Date::new(2026, 1, 1);
        let mut claims = Vec::new();
        for (id, expression) in [
            ("sum", r#"Spot("A") + Spot("B")"#),
            ("difference", r#"Spot("A") - Spot("B")"#),
            ("other", r#"Spot("A") + Spot("C")"#),
        ] {
            let events = EventStream::try_from(vec![CodedEvent::new(
                maturity,
                format!("p = 0; p pays {expression};"),
            )])?;
            claims.extend(
                ScriptedProduct::new(id, events, reference, Currency::USD, MarketIndex::SOFR)?
                    .contingent_claims()?,
            );
        }
        let width = claims[0].simulation_requests().len();
        assert!(width > 1);
        let mut set = NettingSet::new(
            claims,
            Box::new(SingleCurveCSADiscountPolicy::new(
                MarketIndex::SOFR,
                Currency::USD,
            )),
        );
        let mut processor = PreprocessorExecutor::new();
        processor.visit(once(&mut set));
        assert_eq!(set.claims()[0].idx(), Some(0));
        assert_eq!(set.claims()[1].idx(), Some(0));
        assert_eq!(set.claims()[2].idx(), Some(width));
        assert_eq!(processor.requests().len(), 2 * width);
        Ok(())
    }
}
