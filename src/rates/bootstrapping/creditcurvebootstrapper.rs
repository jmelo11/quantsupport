//! Credit curve bootstrapper.
//!
//! Strips piecewise-constant hazard rates from CDS par-spread quotes and
//! exposes the result as a survival curve ([`DiscountTermStructure`](crate::rates::yieldtermstructure::discounttermstructure::DiscountTermStructure) where the
//! "discount factor" at a date is the survival probability). The bootstrapped
//! curve carries the CDS spreads as pillar values together with a
//! AD-generated implicit-function Jacobian so that downstream pricers obtain
//! sensitivities to the original CDS quotes through the standard
//! `register_pillars` mechanism.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ad::{scalar::Scalar, CompiledFunction, DerivativeOrder, ImplicitDerivatives, Recorded},
    core::elements::curveelement::{CreditCurveElement, DiscountCurveElement},
    indices::marketindex::MarketIndex,
    math::solvers::{bisection::Bisection, solvertraits::ContFunc},
    quotes::{quote::Level, quoteselector::QuoteSelector},
    rates::{
        bootstrapping::creditcurveconfiguration::CreditCurveConfiguration,
        yieldtermstructure::{
            discounttermstructure::DiscountTermStructure,
            interestratestermstructure::InterestRatesTermStructure, primalcurve::PrimalCurve,
        },
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency, schedule::MakeSchedule},
    utils::errors::{QSError, Result},
};

const HAZARD_LOWER: f64 = 1e-12;
const HAZARD_UPPER: f64 = 20.0;
const MAX_ITERATIONS: i64 = 200;

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
        let discount_curve = discount_element.curve();
        let discount = PrimalCurve::new(&*discount_curve);

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

        // Residual and survival derivatives come from the same scalar-generic
        // equations used by the numeric strip. Only the implicit linear solve
        // is separate; there is no quote bump or repeated recalibration.
        let n = pillars.len();
        let jacobian = strip_ctx.quote_jacobian(&base_hazards, &spreads)?;

        // Survival curve: node 0 is the reference date with S = 1.
        let mut dates = Vec::with_capacity(n + 1);
        let mut survivals = Vec::with_capacity(n + 1);
        dates.push(ref_date);
        survivals.push(Recorded::scalar(1.0));
        for (d, s) in pillar_dates.iter().zip(&base_survivals) {
            dates.push(*d);
            survivals.push(Recorded::scalar(*s));
        }

        let curve = DiscountTermStructure::<Recorded>::new(
            dates,
            survivals,
            spec.day_counter(),
            spec.interpolator(),
            spec.enable_extrapolation(),
        )?
        .with_pillar_values(spreads.iter().map(|s| Recorded::scalar(*s)).collect())?
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
    discount: &'a dyn InterestRatesTermStructure<f64>,
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
    fn survival<S: Scalar>(&self, t: f64, hazards: &[S], last: S) -> S {
        let mut integral = S::zero();
        let mut prev = 0.0;
        for (j, hazard) in hazards.iter().enumerate() {
            let boundary = self.time(self.pillar_dates[j]);
            if t <= boundary {
                integral = integral.add_val(hazard.mul_val(S::scalar((t - prev).max(0.0))));
                return integral.neg_val().exp();
            }
            integral = integral.add_val(hazard.mul_val(S::scalar(boundary - prev)));
            prev = boundary;
        }
        integral = integral.add_val(last.mul_val(S::scalar((t - prev).max(0.0))));
        integral.neg_val().exp()
    }

    /// Survival probabilities at each pillar date using fully-stripped hazards.
    fn survivals_at_pillars<S: Scalar>(&self, hazards: &[S]) -> Vec<S> {
        self.pillar_dates
            .iter()
            .map(|d| {
                self.survival(
                    self.time(*d),
                    hazards,
                    hazards.last().copied().unwrap_or_else(S::zero),
                )
            })
            .collect()
    }

    /// Shared CDS residual for numerical root solving and compiled AD.
    fn residual<S: Scalar>(
        &self,
        hazards: &[S],
        last: S,
        spread: S,
        dates: &[Date],
        day_counter: DayCounter,
    ) -> Result<S> {
        let mut premium = S::zero();
        let mut protection = S::zero();
        let mut previous = S::one();
        for window in dates.windows(2) {
            let (start, end) = (window[0], window[1]);
            if end <= self.ref_date {
                continue;
            }
            let delta = day_counter.year_fraction(start, end);
            let discount = self.discount.discount_factor(end)?;
            let survival = self.survival(self.time(end), hazards, last);
            let default_probability = previous.sub_val(survival);
            let premium_weight = spread
                .mul_val(S::scalar(delta))
                .mul_val(S::scalar(discount));
            premium = premium.add_val(
                premium_weight
                    .mul_val(survival.add_val(default_probability.mul_val(S::scalar(0.5)))),
            );
            protection = protection
                .add_val(S::scalar((1.0 - self.recovery) * discount).mul_val(default_probability));
            previous = survival;
        }
        Ok(premium.sub_val(protection))
    }

    /// Computes d(survival)/d(quote) with AD partials and implicit differentiation.
    fn quote_jacobian(&self, hazards: &[f64], spreads: &[f64]) -> Result<Vec<Vec<f64>>> {
        let derivatives = self.quote_derivatives(hazards, spreads, DerivativeOrder::First)?;
        Ok(derivatives
            .jacobian()
            .chunks_exact(spreads.len())
            .map(<[f64]>::to_vec)
            .collect())
    }

    /// Captures the actual residual and survival equations at the requested order.
    fn quote_derivatives(
        &self,
        hazards: &[f64],
        spreads: &[f64],
        order: DerivativeOrder,
    ) -> Result<ImplicitDerivatives> {
        let n = hazards.len();
        let mut inputs = hazards.to_vec();
        inputs.extend_from_slice(spreads);
        let schedules = self
            .pillar_dates
            .iter()
            .map(|date| {
                MakeSchedule::new(self.ref_date, *date)
                    .with_frequency(self.frequency)
                    .build()
            })
            .collect::<Result<Vec<_>>>()?;
        let function =
            CompiledFunction::try_trace_with_order(&inputs, order, |inputs| -> Result<_> {
                let (hazards, spreads) = inputs.split_at(n);
                let mut values = Vec::with_capacity(2 * n);
                for (index, schedule) in schedules.iter().enumerate() {
                    let residual = self.residual(
                        &hazards[..index],
                        hazards[index],
                        spreads[index],
                        schedule.dates(),
                        self.premium_day_counters[index],
                    )?;
                    // Nonpositive quotes use the strip's active lower-bound constraint.
                    values.push(spreads[index].select_greater(
                        0.0.into(),
                        residual,
                        hazards[index] - HAZARD_LOWER,
                    ));
                }
                values.extend(self.survivals_at_pillars(hazards));
                Ok(values)
            })
            .map_err(|error| QSError::SolverErr(format!("credit residual AD: {error}")))?;
        ImplicitDerivatives::evaluate(&function, &inputs, n)
            .map_err(|error| QSError::SolverErr(format!("credit implicit derivatives: {error}")))
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
        self.ctx.residual(
            self.known_hazards,
            *x,
            self.spread,
            self.schedule_dates,
            self.premium_day_counter,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            evaluationresults::EvaluationResults,
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
            .map(|i| {
                ref_date
                    .advance(i, TimeUnit::Years)
                    .expect("valid date advancement")
            })
            .collect();
        let dfs: Vec<Recorded> = dates
            .iter()
            .map(|d| Recorded::scalar((-rate * dc.year_fraction(ref_date, *d)).exp()))
            .collect();
        let curve =
            DiscountTermStructure::<Recorded>::new(dates, dfs, dc, Interpolator::LogLinear, true)?;
        Ok(DiscountCurveElement::new(
            MarketIndex::SOFR,
            Rc::new(RefCell::new(curve)),
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
    ) -> Result<EvaluationResults> {
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
            ref_date
                .advance(years, TimeUnit::Years)
                .expect("valid date advancement"),
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
            ref_date
                .advance(5, TimeUnit::Years)
                .expect("valid date advancement"),
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
                ref_date
                    .advance(years, TimeUnit::Years)
                    .expect("valid date advancement"),
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

    /// Checks ad survival quote jacobian matches central rebootstrap bumps.
    #[test]
    fn ad_survival_quote_jacobian_matches_central_rebootstrap_bumps() -> Result<()> {
        let reference = Date::new(2025, 1, 2);
        let spreads = [
            ("Cds_CORPORATE-A_USD_1Y", 0.010),
            ("Cds_CORPORATE-A_USD_3Y", 0.015),
            ("Cds_CORPORATE-A_USD_5Y", 0.020),
        ];
        let index = MarketIndex::Credit("CORPORATE-A".into());
        let (curves, _) = bootstrap_with(reference, &spreads, 0.4)?;
        let curve = curves[&index].curve();
        let jacobian = curve
            .ift_sensitivities()
            .ok_or_else(|| QSError::NotFoundErr("credit quote Jacobian".into()))?;
        let bump = 1e-6;
        for column in 0..spreads.len() {
            let mut up = spreads;
            let mut down = spreads;
            up[column].1 += bump;
            down[column].1 -= bump;
            let (up_curves, _) = bootstrap_with(reference, &up, 0.4)?;
            let (down_curves, _) = bootstrap_with(reference, &down, 0.4)?;
            let up_nodes = up_curves[&index]
                .curve()
                .nodes()
                .ok_or_else(|| QSError::NotFoundErr("credit nodes".into()))?;
            let down_nodes = down_curves[&index]
                .curve()
                .nodes()
                .ok_or_else(|| QSError::NotFoundErr("credit nodes".into()))?;
            for (row, (up, down)) in up_nodes.iter().zip(&down_nodes).skip(1).enumerate() {
                let expected = (up.1.value() - down.1.value()) / (2.0 * bump);
                assert!(
                    (jacobian[row][column] - expected).abs() <= 3e-6 * expected.abs().max(1.0),
                    "row {row}, quote {column}: AD {} vs central bump {expected}",
                    jacobian[row][column]
                );
            }
        }
        Ok(())
    }

    /// Checks ad credit hessians include implicit hazard curvature.
    #[test]
    fn ad_credit_hessians_include_implicit_hazard_curvature() -> Result<()> {
        let reference = Date::new(2025, 1, 2);
        let discount_element = flat_discount_element(reference, 0.03)?;
        let discount_curve = discount_element.curve();
        let discount = PrimalCurve::new(&*discount_curve);
        let dates = [Date::new(2026, 1, 2), Date::new(2028, 1, 2)];
        let day_counters = [DayCounter::Actual360; 2];
        let context = StripContext {
            discount: &discount,
            ref_date: reference,
            day_counter: DayCounter::Actual365,
            frequency: Frequency::Quarterly,
            recovery: 0.4,
            pillar_dates: &dates,
            premium_day_counters: &day_counters,
        };
        let spreads = [0.010, 0.015];
        let hazards = context.strip(&spreads)?;
        let derivatives = context.quote_derivatives(&hazards, &spreads, DerivativeOrder::Second)?;
        let first = context.quote_derivatives(&hazards, &spreads, DerivativeOrder::First)?;
        assert!(first.hessians().is_none());
        for (actual, expected) in derivatives.jacobian().iter().zip(first.jacobian()) {
            assert!((actual - expected).abs() < 1e-12);
        }
        let hessians = derivatives.hessians().unwrap();
        let bump = 2e-6;
        for column in 0..2 {
            let mut up = spreads;
            let mut down = spreads;
            up[column] += bump;
            down[column] -= bump;
            let up_jacobian = context.quote_jacobian(&context.strip(&up)?, &up)?;
            let down_jacobian = context.quote_jacobian(&context.strip(&down)?, &down)?;
            for observation in 0..2 {
                for row in 0..2 {
                    let expected = (up_jacobian[observation][row]
                        - down_jacobian[observation][row])
                        / (2.0 * bump);
                    let actual = hessians[observation * 4 + row * 2 + column];
                    assert!(
                        (actual - expected).abs() < 2e-5 * expected.abs().max(1.0),
                        "survival {observation}, quotes ({row}, {column}): AD {actual}, central {expected}"
                    );
                    assert!((actual - hessians[observation * 4 + column * 2 + row]).abs() < 1e-10);
                }
            }
        }
        Ok(())
    }

    /// Checks zero spread uses the active hazard constraint derivative.
    #[test]
    fn zero_spread_uses_the_active_hazard_constraint_derivative() -> Result<()> {
        let reference = Date::new(2025, 1, 2);
        let spreads = [("Cds_CORPORATE-A_USD_1Y", 0.0)];
        let (curves, _) = bootstrap_with(reference, &spreads, 0.4)?;
        let curve = curves[&MarketIndex::Credit("CORPORATE-A".into())].curve();
        let jacobian = curve
            .ift_sensitivities()
            .ok_or_else(|| QSError::NotFoundErr("credit quote Jacobian".into()))?;
        assert_eq!(jacobian, &[vec![0.0]]);
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
