//! Netting set — a group of claims under a single netting agreement.
//!
//! A [`NettingSet`] holds a collection of [`ContingentClaim`]s that are
//! subject to the same ISDA master agreement (or equivalent bilateral
//! netting arrangement) and carries its own [`DiscountPolicy`] reflecting
//! the CSA collateral terms.

use crate::{
    core::collateral::DiscountPolicy,
    xva::{contigentclaim::ContingentClaim, csa::CsaTerms},
};

/// A group of contingent claims under a single netting agreement.
///
/// Each netting set carries its own [`DiscountPolicy`] which determines
/// the discount curve used for each claim based on the collateral terms
/// of the CSA. When built from [`CsaTerms`], the netting set also carries
/// the client's credit/funding parameters used by the XVA engine.
pub struct NettingSet {
    claims: Vec<ContingentClaim>,
    discount_policy: Box<dyn DiscountPolicy>,
    csa_terms: Option<CsaTerms>,
}

impl NettingSet {
    /// Creates a new netting set from a vector of claims and a discount policy.
    #[must_use]
    pub fn new(claims: Vec<ContingentClaim>, discount_policy: Box<dyn DiscountPolicy>) -> Self {
        Self {
            claims,
            discount_policy,
            csa_terms: None,
        }
    }

    /// Creates a netting set from claims and the client's [`CsaTerms`].
    ///
    /// The discount policy is derived from the collateral terms, and the
    /// credit/funding parameters are stored for use by the XVA engine.
    #[must_use]
    pub fn with_csa_terms(claims: Vec<ContingentClaim>, csa_terms: CsaTerms) -> Self {
        let policy = csa_terms.discount_policy();
        Self {
            claims,
            discount_policy: Box::new(policy),
            csa_terms: Some(csa_terms),
        }
    }

    /// Returns the client's CSA terms, if the set was built from them.
    #[must_use]
    pub const fn csa_terms(&self) -> Option<&CsaTerms> {
        self.csa_terms.as_ref()
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use super::*;
    use crate::{
        ad::{dual::DualFwd, scalar::Scalar},
        core::{
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::constructedelementstore::ConstructedElementStore,
            pricingcontext::PricingContext, trade::Side,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        models::modelconfiguration::{GaussianRateModelParameters, ParameterSource},
        quotes::quotestore::QuoteStore,
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{date::Date, daycounter::DayCounter, enums::Frequency},
        utils::errors::{QSError, Result},
        xva::{
            aggregator::{CvaAggregator, PfeAggregator},
            claimevaluationstrategy::ClaimEvaluationStrategy,
            engine::{LgmModelConfig, XvaEngine, XvaEngineConfig},
            visitors::exposureevaluator::ExposureResult,
        },
    };

    fn terms() -> CsaTerms {
        CsaTerms {
            collateral_index: MarketIndex::SOFR,
            collateral_currency: Currency::USD,
            variation_margin: false,
            counterparty_threshold: 0.0,
            own_threshold: 0.0,
            minimum_transfer_amount: 0.0,
            credit_spread: 0.01,
            recovery: 0.4,
            funding_spread: 0.005,
            funding_spread_curve: None,
            funding_index: None,
            credit_index: None,
        }
    }

    fn run(terms: CsaTerms) -> Result<ExposureResult> {
        let reference_date = Date::new(2025, 1, 1);
        let maturity = Date::new(2026, 1, 1);
        let curve = DiscountTermStructure::<DualFwd>::new(
            vec![reference_date, maturity],
            vec![DualFwd::one(), DualFwd::scalar(0.95)],
            DayCounter::Actual365,
            Interpolator::LogLinear,
            true,
        )?
        .with_pillar_values(vec![DualFwd::scalar(0.95)])?
        .with_pillar_labels(vec!["SOFR.1Y".into()])?;
        let mut elements = ConstructedElementStore::default();
        elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
        );
        let context = PricingContext::new()
            .with_quote_store(QuoteStore::new(reference_date))
            .with_constructed_elements(elements)
            .with_base_currency(Currency::USD)
            .with_base_index(MarketIndex::SOFR);
        let claim = ContingentClaim::new(
            "cashflow".into(),
            0,
            maturity,
            None,
            None,
            None,
            Currency::USD,
            None,
            1.0,
            Side::LongReceive,
            ClaimEvaluationStrategy::Deterministic { amount: 100.0 },
            None,
        );
        let mut netting_sets = HashMap::from([(
            "client".into(),
            NettingSet::with_csa_terms(vec![claim], terms),
        )]);
        XvaEngine::new(
            &context,
            XvaEngineConfig {
                model_configs: vec![LgmModelConfig {
                    market_index: MarketIndex::SOFR,
                    lambda: Some(0.05),
                    parameter_source: Some(ParameterSource::Fixed(
                        GaussianRateModelParameters::new(0.0),
                    )),
                    driver: None,
                }],
                fx_configs: Vec::new(),
                n_paths: 8,
                seed: 42,
                frequency: Frequency::Quarterly,
            },
        )?
        .run(&mut netting_sets)
    }

    fn cva(result: &ExposureResult) -> Result<f64> {
        result
            .xva_values
            .as_ref()
            .and_then(|values| values.iter().find(|value| value.measure == "CVA"))
            .map(|value| value.value)
            .ok_or_else(|| QSError::NotFoundErr("CVA test result".into()))
    }

    #[test]
    fn variation_margin_is_opt_in_and_preserves_raw_cubes() -> Result<()> {
        let raw = run(terms())?;
        assert!(cva(&raw)? > 0.0);
        assert!(raw.xva_values.as_ref().is_some_and(|values| values
            .iter()
            .any(|value| value.measure == "FVA" && value.value > 0.0)));

        let mut fully_margined = terms();
        fully_margined.variation_margin = true;
        let margined = run(fully_margined)?;
        assert!(margined
            .xva_values
            .as_ref()
            .is_some_and(|values| values.iter().all(|value| value.value.abs() < 1e-12)));
        assert_eq!(raw.cubes[0].npvs, margined.cubes[0].npvs);

        let mut disabled = terms();
        disabled.counterparty_threshold = 10.0;
        disabled.own_threshold = 20.0;
        disabled.minimum_transfer_amount = 5.0;
        assert_eq!(cva(&run(disabled)?)?, cva(&raw)?);
        Ok(())
    }

    #[test]
    fn variation_margin_threshold_and_mta_change_engine_xva() -> Result<()> {
        let raw = run(terms())?;
        let mut csa = terms();
        csa.variation_margin = true;
        csa.counterparty_threshold = 10.0;
        let capped = run(csa.clone())?;
        assert!(cva(&capped)? > 0.0 && cva(&capped)? < cva(&raw)?);

        csa.minimum_transfer_amount = 5.0;
        let retained = run(csa.clone())?;
        assert!(cva(&retained)? > cva(&capped)?);
        let cube = &retained.cubes[0];
        let dc = DayCounter::Actual365;
        let discounts: Vec<f64> = cube
            .dates
            .iter()
            .map(|&date| 0.95_f64.powf(dc.year_fraction(cube.dates[0], date)))
            .collect();
        let aggregator = CvaAggregator::new(0.01, 0.4, 8, cube.dates[0], &cube.dates)
            .with_system_discounts(discounts);
        let expected: f64 = cube
            .npvs
            .iter()
            .map(|path| {
                let residual: Vec<f64> = path.iter().map(|&npv| 10.0 * npv / path[0]).collect();
                aggregator.aggregate_path(&residual, &cube.dates)
            })
            .sum();
        assert!((cva(&retained)? - expected).abs() < 1e-12);

        csa.minimum_transfer_amount = 1_000.0;
        assert_eq!(cva(&run(csa)?)?, cva(&raw)?);
        Ok(())
    }

    #[test]
    fn engine_rejects_invalid_margin_amounts_even_when_disabled() {
        for invalid in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut csa = terms();
            csa.counterparty_threshold = invalid;
            assert!(run(csa).is_err());
            let mut csa = terms();
            csa.own_threshold = invalid;
            assert!(run(csa).is_err());
            let mut csa = terms();
            csa.minimum_transfer_amount = invalid;
            assert!(run(csa).is_err());
        }
    }
}
