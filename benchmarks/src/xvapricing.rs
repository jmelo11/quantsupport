//! Calibrated XVA pricing workload for repeated runs and market refreshes.

use std::{collections::HashMap, path::PathBuf};

use quantsupport::prelude::*;

use crate::xva_fixture::{load_credit_curve_specs, load_curve_specs, load_quotes, load_vol_specs};

/// A calibrated USD IRS and USD/CLP cross-currency portfolio in one netting set.
///
/// Every execution returns the complete netting-set exposure cube, CVA/FVA and
/// their published first sensitivities. The engine retains prepared code between
/// calls, while each run generates its paths and results again.
pub struct XvaPricingFixture {
    context: PricingContext,
    engine: XvaEngine,
    netting_sets: HashMap<String, NettingSet>,
}

impl XvaPricingFixture {
    /// Constructs the fixed-seed, monthly, five-year pricing workload.
    ///
    /// Preparation is explicit so a benchmark can exclude it from repeated-run
    /// timings. Use the standalone XVA driver to measure first-call compilation
    /// in a fresh process.
    ///
    /// # Errors
    /// Rejects zero paths and propagates fixture loading, calibration and trade errors.
    pub fn new(paths: usize, variation_margin: bool) -> Result<Self> {
        if paths == 0 {
            return Err(QSError::InvalidValueErr(
                "The XVA pricing benchmark requires a positive path count".into(),
            ));
        }
        let context = pricing_context()?;
        let claims = portfolio_claims(context.evaluation_date())?;
        let mut config: XvaEngineConfig =
            serde_json::from_str(include_str!("../data/xva/xva_config.json"))
                .map_err(|error| QSError::InvalidValueErr(format!("XVA configuration: {error}")))?;
        config.n_paths = paths;
        let mut csa: CsaTerms = serde_json::from_str(include_str!("../data/xva/csa_terms.json"))
            .map_err(|error| QSError::InvalidValueErr(format!("CSA terms: {error}")))?;
        csa.variation_margin = variation_margin;
        let engine = XvaEngine::new(&context, config)?;
        Ok(Self {
            context,
            engine,
            netting_sets: HashMap::from([(
                "portfolio".into(),
                NettingSet::with_csa_terms(claims, csa),
            )]),
        })
    }

    /// Captures and compiles the engine's path, exposure and adjustment calculation.
    ///
    /// # Errors
    /// Propagates model binding, capture and compilation failures.
    pub fn prepare(&mut self) -> Result<()> {
        self.engine.prepare(&mut self.netting_sets)
    }

    /// Generates paths and returns fresh adjustments, the exposure cube and first sensitivities.
    ///
    /// # Errors
    /// Propagates preparation or execution failures from the production engine.
    pub fn run(&mut self) -> Result<ExposureResult> {
        self.engine.run(&mut self.netting_sets)
    }

    /// Refreshes the CLP/USD spot, rebinds the current market and prices the portfolio.
    ///
    /// The multiplier applies to the original spot of one USD per 900 CLP.
    /// This includes market initialization, engine refresh, preparation and a
    /// complete run, so the benchmark measures the cost of pricing changed data.
    ///
    /// # Errors
    /// Rejects nonpositive or nonfinite multipliers and propagates market or pricing errors.
    pub fn reprice_fx(&mut self, multiplier: f64) -> Result<ExposureResult> {
        if !multiplier.is_finite() || multiplier <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "The FX multiplier must be finite and positive".into(),
            ));
        }
        let mut fx = FxStore::new();
        fx.add_fx_rate(
            Currency::CLP,
            Currency::USD,
            Recorded::from(multiplier / 900.0),
        );
        let mut context = self.context.clone().with_fx_store(fx);
        context.initialize()?;
        self.engine.update_market(&context)?;
        self.context = context;
        self.prepare()?;
        self.run()
    }

    /// Reports whether the latest preparation reused its compiled path program.
    #[must_use]
    pub fn preparation_reused_code(&self) -> bool {
        self.engine.preparation_reused_code()
    }
}

/// Loads the rate, credit, volatility and FX markets used by the comparison driver.
fn pricing_context() -> Result<PricingContext> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/xva");
    let quote_store = load_quotes(&data.join("quotes.json"))?;
    let curve_specs = load_curve_specs(&data.join("curve_specs.json"))?
        .into_iter()
        .filter(|spec| *spec.market_index() != MarketIndex::ESTR)
        .collect();
    let credit_specs = load_credit_curve_specs(&data.join("credit_curve_specs.json"))?;
    let (surfaces, cubes) = load_vol_specs(&data.join("vol_specs.json"))?.into_configurations();
    let mut fx = FxStore::new();
    fx.add_fx_rate(Currency::CLP, Currency::USD, Recorded::from(1.0 / 900.0));
    let mut context = PricingContext::new()
        .with_quote_store(quote_store)
        .with_fx_store(fx)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
        .with_curve_configurations(curve_specs)
        .with_credit_curve_configurations(credit_specs)
        .with_volatility_surface_configurations(surfaces)
        .with_volatility_cube_configurations(cubes);
    context.initialize()?;
    Ok(context)
}

/// Builds the same two trades as the standalone cross-branch XVA comparison.
fn portfolio_claims(reference: Date) -> Result<Vec<ContingentClaim>> {
    let maturity = reference.advance(5, TimeUnit::Years)?;
    let notional = 10_000_000.0;
    let swap = MakeSwap::<f64>::default()
        .with_identifier("USD_IRS_5Y".into())
        .with_start_date(reference)
        .with_maturity_date(maturity)
        .with_fixed_rate(0.0378)
        .with_notional(notional)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        ))
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Quarterly)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()?;
    let mut claims =
        SwapTrade::new(swap, reference, notional, Side::LongReceive).into_contingent_claims()?;
    let cross_currency = MakeFloatFloatCrossCurrencySwap::<f64>::default()
        .with_identifier("CLPUSD_XCCY_5Y".into())
        .with_start_date(reference)
        .with_maturity_date(maturity)
        .with_domestic_notional(notional)
        .with_foreign_notional(notional * 900.0)
        .with_domestic_spread(0.0)
        .with_foreign_spread(0.002)
        .with_domestic_currency(Currency::USD)
        .with_foreign_currency(Currency::CLP)
        .with_domestic_market_index(MarketIndex::SOFR)
        .with_foreign_market_index(MarketIndex::ICP)
        .with_side(Side::LongReceive)
        .with_domestic_leg_frequency(Frequency::Semiannual)
        .with_foreign_leg_frequency(Frequency::Semiannual)
        .build()?;
    claims.extend(
        cross_currency
            .legs()
            .to_vec()
            .into_contingent_claims("CLPUSD_XCCY_5Y")?,
    );
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Checks finite values under the price and risk tolerances used by the branch comparison.
    fn assert_close(left: f64, right: f64, relative: f64, absolute: f64) {
        assert!(left.is_finite() && right.is_finite());
        assert!(
            (left - right).abs() <= absolute + relative * left.abs().max(right.abs()),
            "{left} differs from {right}"
        );
    }

    /// Compares every trade/path/date, adjustment label and reported sensitivity.
    fn assert_pricing_matches(left: &ExposureResult, right: &ExposureResult) {
        let cubes = |result: &ExposureResult| {
            result
                .cubes
                .iter()
                .map(|cube| {
                    (
                        cube.trade_id.clone(),
                        (cube.dates.clone(), cube.npvs.clone()),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        let (left_cubes, right_cubes) = (cubes(left), cubes(right));
        assert_eq!(
            left_cubes.keys().collect::<Vec<_>>(),
            right_cubes.keys().collect::<Vec<_>>()
        );
        for (trade, (dates, paths)) in left_cubes {
            let (expected_dates, expected_paths) = &right_cubes[&trade];
            assert_eq!(&dates, expected_dates);
            assert_eq!(paths.len(), expected_paths.len());
            for (path, expected) in paths.iter().zip(expected_paths) {
                assert_eq!(path.len(), expected.len());
                for (&value, &expected_value) in path.iter().zip(expected) {
                    assert_close(value, expected_value, 1e-11, 1e-7);
                }
            }
        }
        let values = |result: &ExposureResult| {
            result
                .xva_values
                .as_ref()
                .unwrap()
                .iter()
                .map(|value| {
                    (
                        (value.netting_set.clone(), value.measure.clone()),
                        value.value,
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        let (left_values, right_values) = (values(left), values(right));
        assert_eq!(
            left_values.keys().collect::<Vec<_>>(),
            right_values.keys().collect::<Vec<_>>()
        );
        for (label, value) in left_values {
            assert_close(value, right_values[&label], 1e-11, 1e-7);
        }
        let risk = |result: &ExposureResult| {
            result
                .sensitivities
                .as_ref()
                .unwrap()
                .iter()
                .cloned()
                .collect::<BTreeMap<_, _>>()
        };
        let (left_risk, right_risk) = (risk(left), risk(right));
        assert_eq!(
            left_risk.keys().collect::<Vec<_>>(),
            right_risk.keys().collect::<Vec<_>>()
        );
        for (label, value) in left_risk {
            assert_close(value, right_risk[&label], 1e-8, 1e-8);
        }
    }

    /// Checks that refresh produces current-market results and reuses code for both CSA modes.
    #[test]
    fn refreshed_pricing_matches_a_fresh_engine_and_reuses_code() -> Result<()> {
        for margin in [false, true] {
            let mut fixture = XvaPricingFixture::new(16, margin)?;
            fixture.prepare()?;
            let initial = fixture.run()?;
            assert_eq!(fixture.netting_sets["portfolio"].claims().len(), 58);
            assert_eq!(initial.cubes.len(), 1);
            assert_eq!(initial.cubes[0].trade_id, "portfolio");
            assert!(initial.cubes.iter().all(|cube| cube.npvs.len() == 16));
            assert_eq!(initial.xva_values.as_ref().unwrap().len(), 2);
            assert!(initial
                .sensitivities
                .as_ref()
                .unwrap()
                .iter()
                .any(|(_, value)| value.abs() > 0.0));
            let updated = fixture.reprice_fx(1.01)?;
            assert!(fixture.preparation_reused_code());
            assert!(initial
                .cubes
                .iter()
                .zip(&updated.cubes)
                .any(|(old, new)| old.npvs != new.npvs));
            let mut fresh = XvaPricingFixture::new(16, margin)?;
            let expected = fresh.reprice_fx(1.01)?;
            assert_pricing_matches(&updated, &expected);
            assert_pricing_matches(&updated, &fixture.run()?);
        }
        Ok(())
    }
}
