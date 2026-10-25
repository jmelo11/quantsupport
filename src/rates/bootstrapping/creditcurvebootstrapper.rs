//! Credit curve bootstrapper.
//!
//! Strips piecewise-constant hazard rates from CDS par-spread quotes and
//! exposes the result as a survival curve ([`DiscountTermStructure`] where the
//! "discount factor" at a date is the survival probability). The bootstrapped
//! curve carries the CDS spreads as pillar values together with a
//! finite-difference IFT Jacobian so that downstream pricers obtain
//! sensitivities to the original CDS quotes through the standard
//! `put_pillars_on_tape` mechanism.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ad::dual::DualFwd,
    ad::scalar::Scalar,
    core::elements::curveelement::{CreditCurveElement, DiscountCurveElement},
    indices::marketindex::MarketIndex,
    math::solvers::{bisection::Bisection, solvertraits::ContFunc},
    quotes::{quote::Level, quoteselector::QuoteSelector},
    rates::{
        bootstrapping::creditcurveconfiguration::CreditCurveConfiguration,
        yieldtermstructure::{
            discounttermstructure::DiscountTermStructure,
            interestratestermstructure::InterestRatesTermStructure,
        },
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency, schedule::MakeSchedule},
    utils::errors::{QSError, Result},
};

const HAZARD_LOWER: f64 = 1e-12;
const HAZARD_UPPER: f64 = 20.0;
const MAX_ITERATIONS: i64 = 200;
const SPREAD_BUMP: f64 = 1e-6;

/// Bootstraps survival curves from CDS par-spread quotes.
pub struct CreditCurveBootstrapper {
    specs: Vec<CreditCurveConfiguration>,
}

impl CreditCurveBootstrapper {
    /// Creates a new bootstrapper for the given curve configurations.
    #[must_use]
    pub const fn new(specs: Vec<CreditCurveConfiguration>) -> Self {
        Self { specs }
    }

    /// Bootstraps all configured credit curves.
    ///
    /// `discount_curves` must contain the discount curve referenced by each
    /// configuration's `discount_index` (typically the output of the
    /// multi-curve bootstrapper).
    ///
    /// # Errors
    /// Returns an error if quotes or discount curves are missing, or if the
    /// hazard-rate strip fails for any pillar.
    pub fn bootstrap(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
        discount_curves: &HashMap<MarketIndex, DiscountCurveElement>,
    ) -> Result<HashMap<MarketIndex, CreditCurveElement>> {
        let mut curves = HashMap::new();
        for spec in &self.specs {
            let element = Self::bootstrap_single(spec, selector, level, discount_curves)?;
            curves.insert(spec.market_index().clone(), element);
        }
        Ok(curves)
    }

    /// Calibrates one credit curve against its quotes using the resolved discount curve.
    fn bootstrap_single(
        spec: &CreditCurveConfiguration,
        selector: &impl QuoteSelector,
        level: Level,
        discount_curves: &HashMap<MarketIndex, DiscountCurveElement>,
    ) -> Result<CreditCurveElement> {
        let ref_date = selector.reference_date();
        let discount_element = discount_curves.get(spec.discount_index()).ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Discount curve {} required by credit curve {}",
                spec.discount_index(),
                spec.market_index()
            ))
        })?;
        let discount = discount_element.to_f64_term_structure(spec.day_counter())?;

        // Keep each quote's premium day count separate from survival-curve time.
        let mut pillars: Vec<(Date, f64, String, DayCounter)> =
            Vec::with_capacity(spec.quotes().len());
        for id in spec.quotes() {
            let quote = selector
                .select(id)
                .ok_or_else(|| QSError::NotFoundErr(format!("CDS quote {id}")))?;
            let tenor = quote
                .details()
                .tenor()
                .ok_or_else(|| QSError::InvalidValueErr(format!("CDS quote {id} has no tenor")))?;
            let spread = quote.levels().value(level)?;
            let premium_dc = quote
                .details()
                .day_counter()
                .unwrap_or_else(|| spec.day_counter());
            pillars.push((ref_date + tenor, spread, id.clone(), premium_dc));
        }
        pillars.sort_by_key(|p| p.0);
        if pillars.is_empty() {
            return Err(QSError::InvalidValueErr(format!(
                "Credit curve {} has no quotes",
                spec.market_index()
            )));
        }
        if pillars.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(QSError::InvalidValueErr(format!(
                "Credit curve {} has duplicate pillar maturities",
                spec.market_index()
            )));
        }

        let pillar_dates: Vec<Date> = pillars.iter().map(|p| p.0).collect();
        let spreads: Vec<f64> = pillars.iter().map(|p| p.1).collect();
        let labels: Vec<String> = pillars.iter().map(|p| p.2.clone()).collect();
        let premium_day_counters: Vec<_> = pillars.iter().map(|p| p.3).collect();

        let strip_ctx = StripContext {
            discount: &discount,
            ref_date,
            day_counter: spec.day_counter(),
            frequency: spec.premium_frequency(),
            recovery: spec.recovery(),
            pillar_dates: &pillar_dates,
            premium_day_counters: &premium_day_counters,
        };

        // Base strip and survival probabilities at pillar dates.
        let base_hazards = strip_ctx.strip(&spreads)?;
        let base_survivals = strip_ctx.survivals_at_pillars(&base_hazards);

        // Finite-difference IFT Jacobian: rows = pillar survivals (nodes[1..]),
        // columns = CDS spread quotes.
        let n = pillars.len();
        let mut jacobian = vec![vec![0.0; n]; n];
        for j in 0..n {
            let mut bumped_spreads = spreads.clone();
            bumped_spreads[j] += SPREAD_BUMP;
            let bumped_hazards = strip_ctx.strip(&bumped_spreads)?;
            let bumped_survivals = strip_ctx.survivals_at_pillars(&bumped_hazards);
            for i in 0..n {
                jacobian[i][j] = (bumped_survivals[i] - base_survivals[i]) / SPREAD_BUMP;
            }
        }

        // Survival curve: node 0 is the reference date with S = 1.
        let mut dates = Vec::with_capacity(n + 1);
        let mut survivals = Vec::with_capacity(n + 1);
        dates.push(ref_date);
        survivals.push(DualFwd::scalar(1.0));
        for (d, s) in pillar_dates.iter().zip(&base_survivals) {
            dates.push(*d);
            survivals.push(DualFwd::scalar(*s));
        }

        let curve = DiscountTermStructure::<DualFwd>::new(
            dates,
            survivals,
            spec.day_counter(),
            spec.interpolator(),
            spec.enable_extrapolation(),
        )?
        .with_pillar_values(spreads.iter().map(|s| DualFwd::scalar(*s)).collect())?
        .with_pillar_labels(labels)?
        .with_ift_sensitivities(jacobian);

        Ok(CreditCurveElement::new(
            spec.market_index().clone(),
            spec.recovery(),
            Rc::new(RefCell::new(curve)),
        ))
    }
}

/// Shared data for stripping the hazards of one credit curve.
struct StripContext<'a> {
    discount: &'a DiscountTermStructure<f64>,
    ref_date: Date,
    day_counter: DayCounter,
    frequency: Frequency,
    recovery: f64,
    pillar_dates: &'a [Date],
    premium_day_counters: &'a [DayCounter],
}

impl StripContext<'_> {
    /// Sequentially strips one piecewise-constant hazard per pillar.
    fn strip(&self, spreads: &[f64]) -> Result<Vec<f64>> {
        let mut hazards: Vec<f64> = Vec::with_capacity(spreads.len());
        for (k, spread) in spreads.iter().enumerate() {
            // Boundary case: a non-positive spread implies a (near) riskless
            // entity for this bucket; the bisection bracket has no sign change
            // there, so assign the minimal hazard directly.
            if *spread <= 0.0 {
                hazards.push(HAZARD_LOWER);
                continue;
            }
            let schedule = MakeSchedule::new(self.ref_date, self.pillar_dates[k])
                .with_frequency(self.frequency)
                .build()?;
            let objective = HazardObjective {
                ctx: self,
                known_hazards: &hazards,
                schedule_dates: schedule.dates(),
                spread: *spread,
                premium_day_counter: self.premium_day_counters[k],
            };
            let solution =
                Bisection::<HazardObjective<'_>>::new(HAZARD_LOWER, HAZARD_UPPER, MAX_ITERATIONS)
                    .solve(&objective)
                    .map_err(|e| {
                        QSError::SolverErr(format!(
                            "Credit strip failed at pillar {} ({}): {e}",
                            k, self.pillar_dates[k]
                        ))
                    })?;
            hazards.push(solution.x);
        }
        Ok(hazards)
    }

    /// Year fraction from the reference date.
    fn time(&self, date: Date) -> f64 {
        self.day_counter.year_fraction(self.ref_date, date)
    }

    /// Survival probability at time `t` for the given hazards, using the
    /// candidate hazard `last` beyond the last known pillar.
    fn survival(&self, t: f64, hazards: &[f64], last: f64) -> f64 {
        let mut integral = 0.0;
        let mut prev = 0.0;
        for (j, hazard) in hazards.iter().enumerate() {
            let boundary = self.time(self.pillar_dates[j]);
            if t <= boundary {
                integral += hazard * (t - prev).max(0.0);
                return (-integral).exp();
            }
            integral += hazard * (boundary - prev);
            prev = boundary;
        }
        integral = last.mul_add((t - prev).max(0.0), integral);
        (-integral).exp()
    }

    /// Survival probabilities at each pillar date using fully-stripped hazards.
    fn survivals_at_pillars(&self, hazards: &[f64]) -> Vec<f64> {
        self.pillar_dates
            .iter()
            .map(|d| self.survival(self.time(*d), hazards, *hazards.last().unwrap_or(&0.0)))
            .collect()
    }
}

/// Root function: premium leg minus protection leg of the pillar CDS as a
/// function of the last-bucket hazard rate.
struct HazardObjective<'a> {
    ctx: &'a StripContext<'a>,
    known_hazards: &'a [f64],
    schedule_dates: &'a [Date],
    spread: f64,
    premium_day_counter: DayCounter,
}

impl ContFunc<f64> for HazardObjective<'_> {
    fn call(&self, x: &f64) -> Result<f64> {
        let ctx = self.ctx;
        let mut premium = 0.0;
        let mut protection = 0.0;
        let mut s_prev = 1.0;
        for w in self.schedule_dates.windows(2) {
            let (d0, d1) = (w[0], w[1]);
            if d1 <= ctx.ref_date {
                continue;
            }
            let delta = self.premium_day_counter.year_fraction(d0, d1);
            let df = ctx.discount.discount_factor(d1)?;
            let s1 = ctx.survival(ctx.time(d1), self.known_hazards, *x);
            let default_prob = s_prev - s1;
            // Premium on survival plus accrual-on-default (half-period approx).
            premium = (self.spread * delta * df).mul_add(0.5f64.mul_add(default_prob, s1), premium);
            protection = ((1.0 - ctx.recovery) * df).mul_add(default_prob, protection);
            s_prev = s1;
        }
        Ok(premium - protection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
            },
            pricer::Pricer,
            request::Request,
            trade::Side,
        },
        currencies::currency::Currency,
        instruments::credit::creditdefaultswap::{CdsTrade, CreditDefaultSwap},
        math::interpolation::interpolator::Interpolator,
        pricers::credit::cdspricer::CdsPricer,
        quotes::quote::{Quote, QuoteDetails, QuoteLevels},
        time::enums::TimeUnit,
        utils::errors::QSError,
    };
    use std::collections::HashMap;

    struct TestSelector {
        ref_date: Date,
        quotes: HashMap<String, Quote>,
    }

    impl QuoteSelector for TestSelector {
        fn select(&self, identifier: &str) -> Option<Quote> {
            self.quotes.get(identifier).cloned()
        }

        fn reference_date(&self) -> Date {
            self.ref_date
        }
    }

    struct SimpleMarketDataProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for SimpleMarketDataProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(MarketData::new(
                self.market_data.fixings().clone(),
                self.market_data.constructed_elements().clone(),
            ))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    fn flat_discount_element(ref_date: Date, rate: f64) -> Result<DiscountCurveElement> {
        let dc = DayCounter::Actual360;
        let dates: Vec<Date> = (0..=10)
            .map(|i| ref_date.advance(i, TimeUnit::Years))
            .collect();
        let dfs: Vec<DualFwd> = dates
            .iter()
            .map(|d| DualFwd::scalar((-rate * dc.year_fraction(ref_date, *d)).exp()))
            .collect();
        let curve =
            DiscountTermStructure::<DualFwd>::new(dates, dfs, dc, Interpolator::LogLinear, true)?;
        Ok(DiscountCurveElement::new(
            MarketIndex::SOFR,
            std::rc::Rc::new(std::cell::RefCell::new(curve)),
        ))
    }

    fn cds_quote(id: &str, spread: f64) -> Result<(String, Quote)> {
        let details = QuoteDetails::parse(id, '_')?;
        Ok((
            id.to_string(),
            Quote::new(details, QuoteLevels::with_mid(spread)),
        ))
    }

    /// Bootstraps a CORPORATE-A credit curve from the given `(quote id, spread)`
    /// pairs over a flat 3% discount curve.
    fn bootstrap_with(
        ref_date: Date,
        spreads: &[(&str, f64)],
        recovery: f64,
    ) -> Result<(
        HashMap<MarketIndex, CreditCurveElement>,
        DiscountCurveElement,
    )> {
        let quote_ids: Vec<String> = spreads.iter().map(|(id, _)| (*id).to_string()).collect();
        let mut quotes = HashMap::new();
        for (id, spread) in spreads {
            let (key, quote) = cds_quote(id, *spread)?;
            quotes.insert(key, quote);
        }
        let selector = TestSelector { ref_date, quotes };

        let discount_element = flat_discount_element(ref_date, 0.03)?;
        let mut discount_curves = HashMap::new();
        discount_curves.insert(MarketIndex::SOFR, discount_element.clone());

        let spec = CreditCurveConfiguration::new(
            MarketIndex::Credit("CORPORATE-A".to_string()),
            Currency::USD,
            MarketIndex::SOFR,
            recovery,
            quote_ids,
        );
        let curves = CreditCurveBootstrapper::new(vec![spec]).bootstrap(
            &selector,
            Level::Mid,
            &discount_curves,
        )?;
        Ok((curves, discount_element))
    }

    fn bootstrap_test_curve(
        ref_date: Date,
    ) -> Result<(
        HashMap<MarketIndex, CreditCurveElement>,
        DiscountCurveElement,
    )> {
        bootstrap_with(
            ref_date,
            &[
                ("Cds_CORPORATE-A_USD_1Y", 0.010),
                ("Cds_CORPORATE-A_USD_3Y", 0.015),
                ("Cds_CORPORATE-A_USD_5Y", 0.020),
            ],
            0.4,
        )
    }

    fn get_corporate_curve(
        curves: &HashMap<MarketIndex, CreditCurveElement>,
    ) -> Result<&CreditCurveElement> {
        curves
            .get(&MarketIndex::Credit("CORPORATE-A".to_string()))
            .ok_or_else(|| QSError::NotFoundErr("credit curve".into()))
    }

    /// Survival probabilities at the curve nodes (including the ref node).
    fn node_survivals(element: &CreditCurveElement) -> Result<Vec<(Date, f64)>> {
        let nodes = element
            .curve()
            .nodes()
            .ok_or_else(|| QSError::NotFoundErr("nodes".into()))?;
        Ok(nodes.iter().map(|(d, v)| (*d, v.value())).collect())
    }

    /// Prices a spot-starting quarterly CORPORATE-A CDS against the given curves.
    #[allow(clippy::too_many_arguments)]
    fn price_cds(
        curves: &HashMap<MarketIndex, CreditCurveElement>,
        discount_element: &DiscountCurveElement,
        ref_date: Date,
        years: i32,
        contract_spread: f64,
        recovery: f64,
        side: Side,
        requests: &[Request],
    ) -> Result<crate::core::evaluationresults::EvaluationResults> {
        let credit_index = MarketIndex::Credit("CORPORATE-A".to_string());
        let mut store = ConstructedElementStore::default();
        store
            .discount_curves_mut()
            .insert(MarketIndex::SOFR, discount_element.clone());
        store
            .credit_curves_mut()
            .insert(credit_index.clone(), get_corporate_curve(curves)?.clone());
        let provider = SimpleMarketDataProvider {
            evaluation_date: ref_date,
            market_data: MarketData::new(HashMap::new(), store),
        };

        let cds = CreditDefaultSwap::new(
            format!("CDS_CORPORATE-A_{years}Y"),
            credit_index,
            MarketIndex::SOFR,
            Currency::USD,
            ref_date,
            ref_date.advance(years, TimeUnit::Years),
            contract_spread,
            recovery,
            Frequency::Quarterly,
            DayCounter::Actual360,
        )?;
        let trade = CdsTrade::new(cds, ref_date, 1_000_000.0, side);
        CdsPricer::new().evaluate(&trade, requests, &provider)
    }

    #[test]
    fn bootstrap_produces_decreasing_survivals() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (curves, _) = bootstrap_test_curve(ref_date)?;
        let credit_index = MarketIndex::Credit("CORPORATE-A".to_string());
        let element = curves
            .get(&credit_index)
            .ok_or_else(|| QSError::NotFoundErr("credit curve".into()))?;
        let nodes = element
            .curve()
            .nodes()
            .ok_or_else(|| QSError::NotFoundErr("nodes".into()))?;
        assert_eq!(nodes.len(), 4); // ref date + 3 pillars
        assert!((nodes[0].1.value() - 1.0).abs() < 1e-12);
        for w in nodes.windows(2) {
            let (s_prev, s_next) = (w[0].1.value(), w[1].1.value());
            assert!(s_next < s_prev, "survivals must be strictly decreasing");
            assert!(s_next > 0.0 && s_next < 1.0);
        }
        Ok(())
    }

    #[test]
    fn pillar_cds_reprices_at_par() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (curves, discount_element) = bootstrap_test_curve(ref_date)?;
        let credit_index = MarketIndex::Credit("CORPORATE-A".to_string());

        let mut store = ConstructedElementStore::default();
        store
            .discount_curves_mut()
            .insert(MarketIndex::SOFR, discount_element);
        store.credit_curves_mut().insert(
            credit_index.clone(),
            curves
                .get(&credit_index)
                .ok_or_else(|| QSError::NotFoundErr("credit curve".into()))?
                .clone(),
        );
        let provider = SimpleMarketDataProvider {
            evaluation_date: ref_date,
            market_data: MarketData::new(HashMap::new(), store),
        };

        let notional = 1_000_000.0;
        let quoted_spread = 0.020;
        let cds = CreditDefaultSwap::new(
            "CDS_CORPORATE-A_5Y".to_string(),
            credit_index,
            MarketIndex::SOFR,
            Currency::USD,
            ref_date,
            ref_date.advance(5, TimeUnit::Years),
            quoted_spread,
            0.4,
            Frequency::Quarterly,
            DayCounter::Actual360,
        )?;
        let trade = CdsTrade::new(cds, ref_date, notional, Side::LongReceive);

        let pricer = CdsPricer::new();
        let results = pricer.evaluate(
            &trade,
            &[Request::Value, Request::FairRate(0), Request::Sensitivities],
            &provider,
        )?;

        let price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        assert!(
            price.abs() < 1e-3 * notional.sqrt(),
            "pillar CDS should reprice at par, got {price}"
        );

        let fair = results
            .fair_rate()
            .ok_or_else(|| QSError::UnexpectedErr("missing fair rate".into()))?;
        assert!(
            (fair - quoted_spread).abs() < 1e-6,
            "fair spread {fair} should match the quoted spread {quoted_spread}"
        );

        let sensitivities = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("missing sensitivities".into()))?;
        assert!(
            sensitivities
                .instrument_keys()
                .iter()
                .any(|k| k.contains("Cds_CORPORATE-A_USD_5Y")),
            "sensitivities should include the CDS quote pillars"
        );
        Ok(())
    }

    #[test]
    fn individual_cds_quote_day_counts_reprice_at_par() -> Result<()> {
        let ref_date = Date::new(2026, 10, 2);
        let (curves, discount) = bootstrap_with(
            ref_date,
            &[
                ("Cds_CORPORATE-A_USD_1Y_Actual365", 0.010),
                ("Cds_CORPORATE-A_USD_3Y_Thirty360", 0.015),
                ("Cds_CORPORATE-A_USD_5Y", 0.020),
            ],
            0.4,
        )?;
        let credit_index = MarketIndex::Credit("CORPORATE-A".into());
        let mut store = ConstructedElementStore::default();
        store
            .discount_curves_mut()
            .insert(MarketIndex::SOFR, discount);
        store
            .credit_curves_mut()
            .insert(credit_index.clone(), get_corporate_curve(&curves)?.clone());
        let provider = SimpleMarketDataProvider {
            evaluation_date: ref_date,
            market_data: MarketData::new(HashMap::new(), store),
        };
        for (years, spread, dc) in [
            (1, 0.010, DayCounter::Actual365),
            (3, 0.015, DayCounter::Thirty360),
            (5, 0.020, DayCounter::Actual360),
        ] {
            let cds = CreditDefaultSwap::new(
                format!("CDS-{years}Y"),
                credit_index.clone(),
                MarketIndex::SOFR,
                Currency::USD,
                ref_date,
                ref_date.advance(years, TimeUnit::Years),
                spread,
                0.4,
                Frequency::Quarterly,
                dc,
            )?;
            let trade = CdsTrade::new(cds, ref_date, 1_000_000.0, Side::LongReceive);
            let result = CdsPricer::new().evaluate(&trade, &[Request::Value], &provider)?;
            assert!(result.price().unwrap().abs() < 0.01, "{years}Y {dc:?}");
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Stress tests: boundary conditions and limiting cases
    // -------------------------------------------------------------------

    /// Credit triangle: for a flat CDS curve, the implied flat hazard must
    /// satisfy `λ ≈ s / (1 − R)` (exact in continuous time; quarterly
    /// discretization introduces only a small error).
    #[test]
    fn credit_triangle_flat_spread() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (spread, recovery) = (0.02, 0.4);
        let (curves, _) = bootstrap_with(
            ref_date,
            &[
                ("Cds_CORPORATE-A_USD_1Y", spread),
                ("Cds_CORPORATE-A_USD_3Y", spread),
                ("Cds_CORPORATE-A_USD_5Y", spread),
            ],
            recovery,
        )?;
        let nodes = node_survivals(get_corporate_curve(&curves)?)?;
        let dc = DayCounter::Actual360;
        let expected_hazard = spread / (1.0 - recovery);
        for (date, survival) in nodes.iter().skip(1) {
            let t = dc.year_fraction(ref_date, *date);
            let implied_hazard = -survival.ln() / t;
            let rel_err = (implied_hazard - expected_hazard).abs() / expected_hazard;
            assert!(
                rel_err < 5e-3,
                "credit triangle violated at {date}: implied {implied_hazard}, expected {expected_hazard}"
            );
        }
        Ok(())
    }

    /// Zero spread is a riskless entity: survival stays at ~1.
    #[test]
    fn zero_spread_gives_full_survival() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (curves, _) = bootstrap_with(
            ref_date,
            &[
                ("Cds_CORPORATE-A_USD_1Y", 0.0),
                ("Cds_CORPORATE-A_USD_5Y", 0.0),
            ],
            0.4,
        )?;
        for (_, survival) in node_survivals(get_corporate_curve(&curves)?)? {
            assert!(
                (survival - 1.0).abs() < 1e-9,
                "zero spread must imply ~full survival, got {survival}"
            );
        }
        Ok(())
    }

    /// Tiny (1bp) and huge (2000bp) spreads must both strip cleanly and
    /// respect the credit triangle within discretization error.
    #[test]
    fn extreme_spreads_bootstrap() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let dc = DayCounter::Actual360;
        for spread in [1e-4, 0.20] {
            let (curves, _) = bootstrap_with(
                ref_date,
                &[
                    ("Cds_CORPORATE-A_USD_1Y", spread),
                    ("Cds_CORPORATE-A_USD_5Y", spread),
                ],
                0.4,
            )?;
            let nodes = node_survivals(get_corporate_curve(&curves)?)?;
            let expected_hazard = spread / 0.6;
            for (date, survival) in nodes.iter().skip(1) {
                assert!(*survival > 0.0 && *survival < 1.0);
                let t = dc.year_fraction(ref_date, *date);
                let implied = -survival.ln() / t;
                let rel_err = (implied - expected_hazard).abs() / expected_hazard;
                assert!(
                    rel_err < 2e-2,
                    "spread {spread}: implied hazard {implied} vs expected {expected_hazard}"
                );
            }
        }
        Ok(())
    }

    /// A spread requiring a hazard beyond the solver cap must fail with a
    /// clear error instead of returning a bogus curve.
    #[test]
    fn impossible_spread_errors() {
        let ref_date = Date::new(2025, 1, 2);
        // Required hazard ≈ 15 / 0.6 = 25 > HAZARD_UPPER (20).
        let result = bootstrap_with(ref_date, &[("Cds_CORPORATE-A_USD_5Y", 15.0)], 0.4);
        assert!(result.is_err(), "absurd spread should fail the strip");
    }

    /// A mildly inverted spread curve must still strip into a valid,
    /// monotonically decreasing survival curve.
    #[test]
    fn inverted_spread_curve_bootstraps() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (curves, _) = bootstrap_with(
            ref_date,
            &[
                ("Cds_CORPORATE-A_USD_1Y", 0.020),
                ("Cds_CORPORATE-A_USD_3Y", 0.019),
                ("Cds_CORPORATE-A_USD_5Y", 0.018),
            ],
            0.4,
        )?;
        let nodes = node_survivals(get_corporate_curve(&curves)?)?;
        for w in nodes.windows(2) {
            assert!(w[1].1 < w[0].1, "survivals must remain decreasing");
            assert!(w[1].1 > 0.0);
        }
        Ok(())
    }

    /// Protection buyer and seller values must be exactly antisymmetric.
    #[test]
    fn buyer_seller_antisymmetry() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (curves, discount) = bootstrap_test_curve(ref_date)?;
        let buyer = price_cds(
            &curves,
            &discount,
            ref_date,
            5,
            0.012,
            0.4,
            Side::LongReceive,
            &[Request::Value],
        )?
        .price()
        .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        let seller = price_cds(
            &curves,
            &discount,
            ref_date,
            5,
            0.012,
            0.4,
            Side::PayShort,
            &[Request::Value],
        )?
        .price()
        .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        assert!(
            buyer.abs() > 1.0,
            "off-market CDS should have nonzero value"
        );
        assert!(
            (buyer + seller).abs() < 1e-9 * buyer.abs(),
            "buyer {buyer} and seller {seller} must be antisymmetric"
        );
        Ok(())
    }

    /// The protection leg is linear in loss-given-default: with the same
    /// survival curve, fair spreads for two instrument recoveries must be in
    /// the exact ratio of their LGDs.
    #[test]
    fn fair_spread_scales_with_lgd() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let (curves, discount) = bootstrap_test_curve(ref_date)?;
        let fair_zero_recovery = price_cds(
            &curves,
            &discount,
            ref_date,
            5,
            0.02,
            0.0,
            Side::LongReceive,
            &[Request::FairRate(0)],
        )?
        .fair_rate()
        .ok_or_else(|| QSError::UnexpectedErr("missing fair rate".into()))?;
        let fair_base = price_cds(
            &curves,
            &discount,
            ref_date,
            5,
            0.02,
            0.4,
            Side::LongReceive,
            &[Request::FairRate(0)],
        )?
        .fair_rate()
        .ok_or_else(|| QSError::UnexpectedErr("missing fair rate".into()))?;
        let ratio = fair_zero_recovery / fair_base;
        assert!(
            (ratio - 1.0 / 0.6).abs() < 1e-10,
            "fair spread must scale with LGD: ratio {ratio}, expected {}",
            1.0 / 0.6
        );
        Ok(())
    }

    /// AD sensitivities (through the IFT Jacobian) must match a brute-force
    /// finite-difference bump-and-rebootstrap of each CDS quote.
    #[test]
    fn ad_sensitivities_match_finite_difference() -> Result<()> {
        let ref_date = Date::new(2025, 1, 2);
        let base_spreads = [
            ("Cds_CORPORATE-A_USD_1Y", 0.010),
            ("Cds_CORPORATE-A_USD_3Y", 0.015),
            ("Cds_CORPORATE-A_USD_5Y", 0.020),
        ];
        let contract_spread = 0.012; // off-market so quote sensitivities are nonzero
        let bump = 1e-4; // 1bp

        let (curves, discount) = bootstrap_with(ref_date, &base_spreads, 0.4)?;
        let results = price_cds(
            &curves,
            &discount,
            ref_date,
            5,
            contract_spread,
            0.4,
            Side::LongReceive,
            &[Request::Value, Request::Sensitivities],
        )?;
        let base_price = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
        let sens = results
            .sensitivities()
            .ok_or_else(|| QSError::UnexpectedErr("missing sensitivities".into()))?;

        for (j, (bumped_id, _)) in base_spreads.iter().enumerate() {
            let mut bumped = base_spreads;
            bumped[j].1 += bump;
            let (curves_up, discount_up) = bootstrap_with(ref_date, &bumped, 0.4)?;
            let bumped_price = price_cds(
                &curves_up,
                &discount_up,
                ref_date,
                5,
                contract_spread,
                0.4,
                Side::LongReceive,
                &[Request::Value],
            )?
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("missing price".into()))?;
            let fd = (bumped_price - base_price) / bump;

            let ad = sens
                .instrument_keys()
                .iter()
                .zip(sens.exposure())
                .find(|(k, _)| k.as_str() == *bumped_id)
                .map(|(_, v)| *v)
                .ok_or_else(|| {
                    QSError::UnexpectedErr(format!("missing AD sensitivity for {bumped_id}"))
                })?;

            let denom = fd.abs().max(1.0);
            assert!(
                (ad - fd).abs() / denom < 2e-2,
                "{bumped_id}: AD {ad} vs FD {fd}"
            );
        }
        Ok(())
    }
}
