// use std::{
//     cell::RefCell,
//     collections::{HashMap, HashSet, VecDeque},
//     rc::Rc,
// };

// use crate::{
//     ad::adreal::{ADReal, IsReal},
//     core::{
//         contextmanager::ContextManager,
//         elements::curveelement::DiscountCurveElement,
//         marketdatahandling::{
//             constructedelementrequest::ConstructedElementRequest,
//             constructedelementstore::ConstructedElementStore,
//             marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
//         },
//         pricer::Pricer,
//         request::{LegsProvider, Request},
//         trade::Side,
//     },
//     currencies::currency::Currency,
//     currencies::exchangeratestore::ExchangeRateStore,
//     indices::marketindex::MarketIndex,
//     instruments::{
//         cashflows::{
//             cashflow::Cashflow, cashflowtype::CashflowType, coupons::NonLinearCoupon, leg::Leg,
//         },
//         fixedincome::fixedratedeposit::{FixedRateDeposit, FixedRateDepositTrade},
//         fx::fxforward::FxForwardTrade,
//         rates::{
//             basisswap::{BasisSwap, BasisSwapTrade},
//             crosscurrencyswap::{CrossCurrencySwap, CrossCurrencySwapTrade},
//             ratefutures::RateFuturesTrade,
//             swap::{Swap, SwapTrade},
//         },
//     },
//     math::{
//         interpolation::interpolator::Interpolator,
//         solvers::{
//             solvertraits::{ADJacobian, ContFunc, VectorFunc},
//             vectornewton::VectorNewton,
//         },
//     },
//     pricers::cashflows::discountingcashflowpricer::CashflowDiscountPricer,
//     pricers::{fx::fxforwardpricer::FxForwardPricer, rates::ratefuturespricer::RateFuturesPricer},
//     quotes::quote::{BuiltInstrument, Level, Quote},
//     rates::yieldtermstructure::{
//         discounttermstructure::DiscountTermStructure,
//         interestratestermstructure::InterestRatesTermStructure,
//     },
//     time::{date::Date, daycounter::DayCounter, period::Period},
//     utils::errors::{AtlasError, Result},
// };

// /// Selects market quotes for a given curve index and tenor.
// pub trait QuoteSelector {
//     /// Returns the quote matching `market_index` and `tenor`.
//     fn select(&self, market_index: &MarketIndex, tenor: &Period) -> Option<Quote>;
// }

// /// Curve bootstrap specification with instrument buckets and settings.
// pub struct CurveSpect {
//     /// Target curve market index.
//     pub market_index: MarketIndex,
//     /// Target curve currency.
//     pub currency: Currency,
//     /// Explicit dependency list (in addition to inferred ones).
//     pub dependencies: Vec<MarketIndex>,
//     /// Day-counter of calibrated curve nodes.
//     pub day_counter: DayCounter,
//     /// Interpolator used by the calibrated curve.
//     pub interpolator: Interpolator,
//     /// Extrapolation flag for the calibrated curve.
//     pub enable_extrapolation: bool,
//     /// Deposit tenors used as calibration instruments.
//     pub deposits: Vec<Period>,
//     /// Futures tenors used as calibration instruments.
//     pub futures: Vec<Period>,
//     /// Swap tenors used as calibration instruments.
//     pub swaps: Vec<Period>,
//     /// Basis-swap tenors used as calibration instruments.
//     pub basis_swaps: Vec<Period>,
//     /// Cross-currency swap tenors used as calibration instruments.
//     pub xccy_swaps: Vec<Period>,
//     /// FX outright forward tenors used as calibration instruments.
//     pub fx_forwards: Vec<Period>,
//     /// FX forward-point tenors used as calibration instruments.
//     pub fx_forward_points: Vec<Period>,
//     /// Spot FX map keyed by concatenated pair name (e.g. `EURUSD`).
//     pub fx_spot: HashMap<String, f64>,
// }

// impl CurveSpect {
//     /// Resolves configured tenors into concrete calibration instruments.
//     ///
//     /// # Errors
//     /// Returns an error if quote levels are missing or a pillar date cannot be inferred.
//     pub fn resolve(
//         &self,
//         selector: &impl QuoteSelector,
//         level: Level,
//     ) -> Result<ResolvedCurveSpec> {
//         let mut instruments = Vec::new();

//         self.collect_bucket(
//             selector,
//             level,
//             &self.deposits,
//             CalibrationKind::PriceZero,
//             &mut instruments,
//         )?;
//         self.collect_bucket(
//             selector,
//             level,
//             &self.futures,
//             CalibrationKind::FuturePrice,
//             &mut instruments,
//         )?;
//         self.collect_bucket(
//             selector,
//             level,
//             &self.swaps,
//             CalibrationKind::PriceZero,
//             &mut instruments,
//         )?;
//         self.collect_bucket(
//             selector,
//             level,
//             &self.basis_swaps,
//             CalibrationKind::PriceZero,
//             &mut instruments,
//         )?;
//         self.collect_bucket(
//             selector,
//             level,
//             &self.xccy_swaps,
//             CalibrationKind::PriceZero,
//             &mut instruments,
//         )?;
//         self.collect_bucket(
//             selector,
//             level,
//             &self.fx_forwards,
//             CalibrationKind::FxForwardOutright,
//             &mut instruments,
//         )?;
//         self.collect_bucket(
//             selector,
//             level,
//             &self.fx_forward_points,
//             CalibrationKind::FxForwardPoints,
//             &mut instruments,
//         )?;

//         instruments.sort_by_key(|x| x.pillar_date);

//         Ok(ResolvedCurveSpec {
//             market_index: self.market_index.clone(),
//             currency: self.currency,
//             dependencies: self.dependencies.clone(),
//             day_counter: self.day_counter,
//             interpolator: self.interpolator,
//             enable_extrapolation: self.enable_extrapolation,
//             fx_spot: self.fx_spot.clone(),
//             instruments,
//         })
//     }

//     /// Collects quotes from one tenor bucket and transforms them into
//     /// resolved calibration instruments.
//     ///
//     /// # Errors
//     /// Returns an error when quote extraction/building fails.
//     fn collect_bucket(
//         &self,
//         selector: &impl QuoteSelector,
//         level: Level,
//         tenors: &[Period],
//         kind: CalibrationKind,
//         out: &mut Vec<ResolvedInstrument>,
//     ) -> Result<()> {
//         for tenor in tenors {
//             let Some(quote) = selector.select(&self.market_index, tenor) else {
//                 continue;
//             };

//             let quote_value = quote.levels().value(level)?;
//             let built = if matches!(kind, CalibrationKind::FxForwardPoints) {
//                 None
//             } else {
//                 Some(quote.build_instrument(level)?)
//             };
//             let pillar_date = self.resolve_pillar_date(&quote, built.as_ref())?;

//             out.push(ResolvedInstrument {
//                 quote,
//                 level,
//                 built,
//                 kind,
//                 quote_value,
//                 pillar_date,
//             });
//         }
//         Ok(())
//     }

//     /// Determines the calibration pillar date for a quote/instrument.
//     ///
//     /// # Errors
//     /// Returns an error if maturity information cannot be inferred.
//     fn resolve_pillar_date(&self, quote: &Quote, built: Option<&BuiltInstrument>) -> Result<Date> {
//         if let Some(inst) = built {
//             return match inst {
//                 BuiltInstrument::FixedRateDeposit(x) => Self::max_leg_payment(x.legs()),
//                 BuiltInstrument::Swap(x) => Self::max_leg_payment(x.legs()),
//                 BuiltInstrument::BasisSwap(x) => Self::max_leg_payment(x.legs()),
//                 BuiltInstrument::CrossCurrencySwap(x) => Self::max_leg_payment(x.legs()),
//                 BuiltInstrument::RateFutures(x) => Ok(x.end_date()),
//                 BuiltInstrument::FxForward(x) => Ok(x.delivery_date()),
//                 _ => quote
//                     .details()
//                     .maturity()
//                     .or_else(|| quote.details().tenor().map(|t| quote.reference_date() + t))
//                     .ok_or_else(|| {
//                         AtlasError::ValueNotSetErr("Unable to infer pillar date".into())
//                     }),
//             };
//         }

//         quote
//             .details()
//             .maturity()
//             .or_else(|| quote.details().tenor().map(|t| quote.reference_date() + t))
//             .ok_or_else(|| AtlasError::ValueNotSetErr("Unable to infer pillar date".into()))
//     }

//     /// Returns the latest payment date across all legs.
//     ///
//     /// # Errors
//     /// Returns an error if no cashflows are present.
//     fn max_leg_payment(legs: &[Leg]) -> Result<Date> {
//         legs.iter()
//             .flat_map(|l| l.cashflows())
//             .map(|cf| match cf {
//                 CashflowType::FixedRateCoupon(c) => c.payment_date(),
//                 CashflowType::FloatingRateCoupon(c) => c.payment_date(),
//                 CashflowType::OptionEmbeddedCoupon(c) => c.payment_date(),
//                 CashflowType::Redemption(c) => c.payment_date(),
//                 CashflowType::Disbursement(c) => c.payment_date(),
//             })
//             .max()
//             .ok_or_else(|| AtlasError::ValueNotSetErr("Instrument has no cashflows".into()))
//     }
// }

// /// Resolved calibration payload for one curve.
// pub struct ResolvedCurveSpec {
//     /// Target curve market index.
//     pub market_index: MarketIndex,
//     /// Target curve currency.
//     pub currency: Currency,
//     /// Explicit dependency list.
//     pub dependencies: Vec<MarketIndex>,
//     /// Day-counter of calibrated curve nodes.
//     pub day_counter: DayCounter,
//     /// Interpolator used by the calibrated curve.
//     pub interpolator: Interpolator,
//     /// Extrapolation flag for the calibrated curve.
//     pub enable_extrapolation: bool,
//     /// Spot FX map keyed by pair string.
//     pub fx_spot: HashMap<String, f64>,
//     /// Ordered instrument list used for calibration.
//     pub instruments: Vec<ResolvedInstrument>,
// }

// #[derive(Clone, Copy)]
// enum CalibrationKind {
//     PriceZero,
//     FuturePrice,
//     FxForwardOutright,
//     FxForwardPoints,
// }

// /// A resolved calibration instrument.
// pub struct ResolvedInstrument {
//     quote: Quote,
//     level: Level,
//     built: Option<BuiltInstrument>,
//     kind: CalibrationKind,
//     quote_value: f64,
//     pillar_date: Date,
// }

// /// Iterative multi-curve bootstrap engine.
// pub struct CurveBootstrapper {
//     tol: f64,
//     max_iter: usize,
//     resolved_specs: HashMap<MarketIndex, ResolvedCurveSpec>,
// }

// impl CurveBootstrapper {
//     /// Creates a bootstrapper with convergence tolerance and outer iteration cap.
//     #[must_use]
//     pub fn new(
//         tol: f64,
//         max_iter: usize,
//         resolved_specs: HashMap<MarketIndex, ResolvedCurveSpec>,
//     ) -> Self {
//         Self {
//             tol,
//             max_iter,
//             resolved_specs,
//         }
//     }

//     /// Resolves dependency order for all configured curves.
//     ///
//     /// # Errors
//     /// Returns an error when a required dependency is missing.
//     pub fn resolve_dependencies(&self) -> Result<Vec<MarketIndex>> {
//         let currency_to_index = self.currency_to_curve_index()?;
//         let mut dep_map: HashMap<MarketIndex, HashSet<MarketIndex>> = HashMap::new();

//         for (idx, spec) in &self.resolved_specs {
//             let mut deps: HashSet<MarketIndex> = spec.dependencies.iter().cloned().collect();
//             for instrument in &spec.instruments {
//                 for dep in self.infer_dependencies(instrument, idx, &currency_to_index)? {
//                     deps.insert(dep);
//                 }
//             }

//             deps.remove(idx);
//             for dep in &deps {
//                 if !self.resolved_specs.contains_key(dep) {
//                     return Err(AtlasError::NotFoundErr(format!(
//                         "Curve {} depends on {}, but dependency is missing",
//                         idx, dep
//                     )));
//                 }
//             }
//             dep_map.insert(idx.clone(), deps);
//         }

//         let mut indegree: HashMap<MarketIndex, usize> = self
//             .resolved_specs
//             .keys()
//             .cloned()
//             .map(|k| (k, 0))
//             .collect();
//         let mut reverse: HashMap<MarketIndex, Vec<MarketIndex>> = HashMap::new();

//         for (idx, deps) in &dep_map {
//             if let Some(v) = indegree.get_mut(idx) {
//                 *v = deps.len();
//             }
//             for dep in deps {
//                 reverse.entry(dep.clone()).or_default().push(idx.clone());
//             }
//         }

//         let mut queue: VecDeque<MarketIndex> = indegree
//             .iter()
//             .filter(|(_, d)| **d == 0)
//             .map(|(k, _)| k.clone())
//             .collect();

//         let mut order = Vec::new();
//         while let Some(node) = queue.pop_front() {
//             order.push(node.clone());
//             if let Some(children) = reverse.get(&node) {
//                 for child in children {
//                     if let Some(v) = indegree.get_mut(child) {
//                         *v = v.saturating_sub(1);
//                         if *v == 0 {
//                             queue.push_back(child.clone());
//                         }
//                     }
//                 }
//             }
//         }

//         if order.len() < self.resolved_specs.len() {
//             for idx in self.resolved_specs.keys() {
//                 if !order.contains(idx) {
//                     order.push(idx.clone());
//                 }
//             }
//         }

//         Ok(order)
//     }

//     /// Runs iterative dual/multi-curve bootstrapping.
//     ///
//     /// # Errors
//     /// Returns an error when calibration fails or does not converge.
//     pub fn bootstrap(
//         &self,
//         _ctx: &ContextManager,
//     ) -> Result<HashMap<MarketIndex, DiscountCurveElement>> {
//         let order = self.resolve_dependencies()?;
//         let mut curves: HashMap<MarketIndex, CurveState> = HashMap::new();
//         let mut converged = false;

//         for _ in 0..self.max_iter {
//             let previous = curves.clone();
//             curves.clear();

//             for idx in &order {
//                 let spec = self.resolved_specs.get(idx).ok_or_else(|| {
//                     AtlasError::NotFoundErr(format!("Missing resolved spec for {idx}"))
//                 })?;
//                 let state = self.calibrate_curve_vector(idx, spec, &curves, &previous)?;
//                 curves.insert(idx.clone(), state);
//             }

//             if previous.is_empty() {
//                 continue;
//             }

//             let mut max_move: f64 = 0.0;
//             for (idx, curve) in &curves {
//                 if let Some(prev) = previous.get(idx) {
//                     max_move = max_move.max(curve.max_abs_diff(prev)?);
//                 } else {
//                     max_move = f64::INFINITY;
//                 }
//             }

//             if max_move.is_finite() && max_move <= self.tol {
//                 converged = true;
//                 break;
//             }
//         }

//         if !converged && !curves.is_empty() {
//             return Err(AtlasError::SolverErr(
//                 "Curve bootstrap did not converge within max iterations".into(),
//             ));
//         }

//         self.to_curve_elements(&curves)
//     }

//     /// Calibrates one curve by solving its full node vector in one system.
//     ///
//     /// # Errors
//     /// Returns an error when the curve has no instruments or solver evaluation fails.
//     fn calibrate_curve_vector(
//         &self,
//         target_index: &MarketIndex,
//         spec: &ResolvedCurveSpec,
//         current_iter_curves: &HashMap<MarketIndex, CurveState>,
//         previous_iter_curves: &HashMap<MarketIndex, CurveState>,
//     ) -> Result<CurveState> {
//         if spec.instruments.is_empty() {
//             return Err(AtlasError::ValueNotSetErr(format!(
//                 "No instruments configured for curve {target_index}"
//             )));
//         }

//         self.validate_unique_pillars(spec)?;

//         let reference_date = spec.instruments[0].quote.reference_date();
//         let prior =
//             self.initial_curve_guess(target_index, spec, previous_iter_curves, reference_date)?;

//         let x0 = spec
//             .instruments
//             .iter()
//             .map(|inst| prior.discount_factor(inst.pillar_date))
//             .collect::<Result<Vec<_>>>()?;

//         let currency_to_index = self.currency_to_curve_index()?;
//         let merged_base = self.merge_curves(current_iter_curves, previous_iter_curves);

//         let x0_ad = x0.iter().copied().map(ADReal::new).collect::<Vec<_>>();
//         let problem = BootstrapVectorProblem {
//             bootstrapper: self,
//             target_index,
//             spec,
//             merged_base: &merged_base,
//             currency_to_index: &currency_to_index,
//             evaluation_date: reference_date,
//         };
//         let solver = VectorNewton::<BootstrapVectorProblem<'_>>::new(self.tol.max(1e-12), 64);
//         let solution = solver.solve(&problem, &x0_ad)?;

//         let mut labels = Vec::with_capacity(spec.instruments.len() + 1);
//         labels.push(format!("{}_spot", target_index));
//         labels.extend(
//             spec.instruments
//                 .iter()
//                 .map(|x| x.quote.details().identifier()),
//         );

//         let mut dates = Vec::with_capacity(spec.instruments.len() + 1);
//         dates.push(reference_date);
//         dates.extend(spec.instruments.iter().map(|x| x.pillar_date));

//         let mut dfs = Vec::with_capacity(spec.instruments.len() + 1);
//         dfs.push(1.0);
//         dfs.extend(solution.x.into_iter().map(|v| v.value()));

//         CurveState::from_nodes(
//             dates,
//             dfs,
//             labels,
//             spec.day_counter,
//             spec.interpolator,
//             spec.enable_extrapolation,
//         )
//     }

//     /// Checks that no two calibration instruments share the same pillar date.
//     ///
//     /// # Errors
//     /// Returns an error if duplicate pillars are detected.
//     fn validate_unique_pillars(&self, spec: &ResolvedCurveSpec) -> Result<()> {
//         let mut seen = HashSet::new();
//         for inst in &spec.instruments {
//             if !seen.insert(inst.pillar_date) {
//                 return Err(AtlasError::InvalidValueErr(
//                     "Each calibration instrument must map to a unique pillar date".into(),
//                 ));
//             }
//         }
//         Ok(())
//     }

//     /// Builds an initial curve guess for solver warm start.
//     ///
//     /// # Errors
//     /// Returns an error if initial curve state creation fails.
//     fn initial_curve_guess(
//         &self,
//         target_index: &MarketIndex,
//         spec: &ResolvedCurveSpec,
//         previous_iter_curves: &HashMap<MarketIndex, CurveState>,
//         reference_date: Date,
//     ) -> Result<CurveState> {
//         if let Some(curve) = previous_iter_curves.get(target_index) {
//             return Ok(curve.clone());
//         }

//         let mut dates = vec![reference_date];
//         let mut dfs = vec![1.0];
//         let mut labels = vec![format!("{}_spot", target_index)];

//         for inst in &spec.instruments {
//             let t = spec
//                 .day_counter
//                 .year_fraction(reference_date, inst.pillar_date);
//             dates.push(inst.pillar_date);
//             dfs.push((-0.02 * t.max(0.0)).exp());
//             labels.push(inst.quote.details().identifier());
//         }

//         CurveState::from_nodes(
//             dates,
//             dfs,
//             labels,
//             spec.day_counter,
//             spec.interpolator,
//             spec.enable_extrapolation,
//         )
//     }

//     /// Evaluates residual vector for a candidate node vector.
//     ///
//     /// # Errors
//     /// Returns an error if dimensions mismatch or instrument valuation fails.
//     fn residual_vector(
//         &self,
//         target_index: &MarketIndex,
//         spec: &ResolvedCurveSpec,
//         merged_base: &HashMap<MarketIndex, CurveState>,
//         currency_to_index: &HashMap<Currency, MarketIndex>,
//         evaluation_date: Date,
//         x: &[ADReal],
//     ) -> Result<Vec<ADReal>> {
//         if x.len() != spec.instruments.len() {
//             return Err(AtlasError::InvalidValueErr(
//                 "Residual vector input size does not match instrument count".into(),
//             ));
//         }

//         let trial_target = CurveState::from_nodes(
//             {
//                 let mut d = vec![evaluation_date];
//                 d.extend(spec.instruments.iter().map(|inst| inst.pillar_date));
//                 d
//             },
//             {
//                 let mut v = vec![1.0];
//                 v.extend(x.iter().map(|v| v.value()));
//                 v
//             },
//             {
//                 let mut l = vec![format!("{}_spot", target_index)];
//                 l.extend(
//                     spec.instruments
//                         .iter()
//                         .map(|inst| inst.quote.details().identifier()),
//                 );
//                 l
//             },
//             spec.day_counter,
//             spec.interpolator,
//             spec.enable_extrapolation,
//         )?;

//         let mut all_curves = merged_base.clone();
//         all_curves.insert(target_index.clone(), trial_target);

//         spec.instruments
//             .iter()
//             .map(|inst| {
//                 self.instrument_residual(
//                     inst,
//                     &all_curves,
//                     currency_to_index,
//                     evaluation_date,
//                     spec,
//                 )
//                 .map(ADReal::new)
//             })
//             .collect()
//     }

//     /// Evaluates a scalar residual for one calibration instrument.
//     ///
//     /// # Errors
//     /// Returns an error if pricing or required market fields are unavailable.
//     fn instrument_residual(
//         &self,
//         instrument: &ResolvedInstrument,
//         all_curves: &HashMap<MarketIndex, CurveState>,
//         currency_to_index: &HashMap<Currency, MarketIndex>,
//         evaluation_date: Date,
//         spec: &ResolvedCurveSpec,
//     ) -> Result<f64> {
//         match instrument.kind {
//             CalibrationKind::PriceZero => {
//                 self.price_zero_residual(instrument, all_curves, evaluation_date)
//             }
//             CalibrationKind::FuturePrice => {
//                 let built = instrument.quote.build_instrument(instrument.level)?;
//                 let BuiltInstrument::RateFutures(fut) = built else {
//                     return Err(AtlasError::InvalidValueErr(
//                         "Future calibration expects BuiltInstrument::RateFutures".into(),
//                     ));
//                 };
//                 let trade = RateFuturesTrade::new(fut, evaluation_date, 1.0, Side::PayShort);
//                 let pricer = RateFuturesPricer::new();
//                 let provider = BootstrapMarketDataProvider::new(
//                     evaluation_date,
//                     all_curves,
//                     &self.resolved_specs,
//                 )?;
//                 let eval = pricer.evaluate(&trade, &[Request::Value], &provider)?;
//                 let model = eval
//                     .price()
//                     .ok_or_else(|| AtlasError::ValueNotSetErr("Missing futures quote".into()))?;
//                 Ok(model - instrument.quote_value)
//             }
//             CalibrationKind::FxForwardOutright => {
//                 let built = instrument.quote.build_instrument(instrument.level)?;
//                 let BuiltInstrument::FxForward(fwd) = built else {
//                     return Err(AtlasError::InvalidValueErr(
//                         "FX forward calibration expects BuiltInstrument::FxForward".into(),
//                     ));
//                 };
//                 let trade = FxForwardTrade::new(fwd, evaluation_date, 1.0, Side::PayShort);
//                 let pricer = FxForwardPricer::new();
//                 let provider = BootstrapMarketDataProvider::new(
//                     evaluation_date,
//                     all_curves,
//                     &self.resolved_specs,
//                 )?;
//                 let eval = pricer.evaluate(&trade, &[Request::Value], &provider)?;
//                 let model = eval
//                     .price()
//                     .ok_or_else(|| AtlasError::ValueNotSetErr("Missing FX forward quote".into()))?;
//                 Ok(model - instrument.quote_value)
//             }
//             CalibrationKind::FxForwardPoints => {
//                 let d = instrument.quote.details();
//                 let base = d.pay_currency().ok_or_else(|| {
//                     AtlasError::ValueNotSetErr("FX forward points missing base currency".into())
//                 })?;
//                 let quote_ccy = d.receive_currency().ok_or_else(|| {
//                     AtlasError::ValueNotSetErr("FX forward points missing quote currency".into())
//                 })?;
//                 let tenor = d.tenor().ok_or_else(|| {
//                     AtlasError::ValueNotSetErr("FX forward points missing tenor".into())
//                 })?;
//                 let delivery = instrument.quote.reference_date() + tenor;

//                 let spot = self.fx_spot(spec, base, quote_ccy)?;
//                 let model = self.fx_forward_from_curves(
//                     all_curves,
//                     currency_to_index,
//                     base,
//                     quote_ccy,
//                     delivery,
//                     spot,
//                 )?;
//                 Ok((model - spot) - instrument.quote_value)
//             }
//         }
//     }

//     /// Computes model PV residual for instruments calibrated to zero NPV.
//     ///
//     /// # Errors
//     /// Returns an error if instrument pricing fails.
//     fn price_zero_residual(
//         &self,
//         instrument: &ResolvedInstrument,
//         all_curves: &HashMap<MarketIndex, CurveState>,
//         evaluation_date: Date,
//     ) -> Result<f64> {
//         let provider =
//             BootstrapMarketDataProvider::new(evaluation_date, all_curves, &self.resolved_specs)?;
//         let built = instrument.quote.build_instrument(instrument.level)?;

//         match built {
//             BuiltInstrument::FixedRateDeposit(inst) => {
//                 let trade = FixedRateDepositTrade::new(inst, evaluation_date, 1.0, Side::PayShort);
//                 let pricer =
//                     CashflowDiscountPricer::<FixedRateDeposit, FixedRateDepositTrade>::new();
//                 let eval = pricer.evaluate(&trade, &[Request::Value], &provider)?;
//                 eval.price()
//                     .ok_or_else(|| AtlasError::ValueNotSetErr("Missing price".into()))
//             }
//             BuiltInstrument::Swap(inst) => {
//                 let trade = SwapTrade::new(inst, evaluation_date, 1.0, Side::PayShort);
//                 let pricer = CashflowDiscountPricer::<Swap, SwapTrade>::new();
//                 let eval = pricer.evaluate(&trade, &[Request::Value], &provider)?;
//                 eval.price()
//                     .ok_or_else(|| AtlasError::ValueNotSetErr("Missing price".into()))
//             }
//             BuiltInstrument::BasisSwap(inst) => {
//                 let trade = BasisSwapTrade::new(inst, evaluation_date, 1.0, Side::PayShort);
//                 let pricer = CashflowDiscountPricer::<BasisSwap, BasisSwapTrade>::new();
//                 let eval = pricer.evaluate(&trade, &[Request::Value], &provider)?;
//                 eval.price()
//                     .ok_or_else(|| AtlasError::ValueNotSetErr("Missing price".into()))
//             }
//             BuiltInstrument::CrossCurrencySwap(inst) => {
//                 let trade =
//                     CrossCurrencySwapTrade::new(inst, evaluation_date, 1.0, 1.0, Side::PayShort);
//                 let pricer =
//                     CashflowDiscountPricer::<CrossCurrencySwap, CrossCurrencySwapTrade>::new();
//                 let eval = pricer.evaluate(&trade, &[Request::Value], &provider)?;
//                 eval.price()
//                     .ok_or_else(|| AtlasError::ValueNotSetErr("Missing price".into()))
//             }
//             _ => Err(AtlasError::NotImplementedErr(
//                 "Price-zero residual not implemented for this instrument type".into(),
//             )),
//         }
//     }

//     /// Infers curve dependencies required to value one instrument.
//     ///
//     /// # Errors
//     /// Returns an error if required dependency metadata cannot be resolved.
//     fn infer_dependencies(
//         &self,
//         instrument: &ResolvedInstrument,
//         target_curve: &MarketIndex,
//         currency_to_index: &HashMap<Currency, MarketIndex>,
//     ) -> Result<Vec<MarketIndex>> {
//         let mut deps = HashSet::new();

//         if let Some(built) = &instrument.built {
//             match built {
//                 BuiltInstrument::FixedRateDeposit(inst) => {
//                     let idx = inst.market_index();
//                     if &idx != target_curve {
//                         deps.insert(idx);
//                     }
//                 }
//                 BuiltInstrument::Swap(inst) => {
//                     let idx = inst.market_index();
//                     if &idx != target_curve {
//                         deps.insert(idx);
//                     }
//                 }
//                 BuiltInstrument::BasisSwap(inst) => {
//                     let pay = inst.pay_market_index();
//                     let recv = inst.receive_market_index();
//                     if &pay != target_curve {
//                         deps.insert(pay);
//                     }
//                     if &recv != target_curve {
//                         deps.insert(recv);
//                     }
//                 }
//                 BuiltInstrument::CrossCurrencySwap(inst) => {
//                     let dom = inst.domestic_market_index();
//                     let for_idx = inst.foreign_market_index();
//                     if &dom != target_curve {
//                         deps.insert(dom);
//                     }
//                     if &for_idx != target_curve {
//                         deps.insert(for_idx);
//                     }
//                 }
//                 BuiltInstrument::RateFutures(inst) => {
//                     let idx = inst.market_index();
//                     if &idx != target_curve {
//                         deps.insert(idx);
//                     }
//                 }
//                 BuiltInstrument::FxForward(inst) => {
//                     if let Some(idx) = currency_to_index.get(&inst.base_currency()) {
//                         if idx != target_curve {
//                             deps.insert(idx.clone());
//                         }
//                     }
//                     if let Some(idx) = currency_to_index.get(&inst.quote_currency()) {
//                         if idx != target_curve {
//                             deps.insert(idx.clone());
//                         }
//                     }
//                 }
//                 _ => {}
//             }
//         }

//         if matches!(instrument.kind, CalibrationKind::FxForwardPoints) {
//             let details = instrument.quote.details();
//             if let Some(base) = details.pay_currency() {
//                 if let Some(idx) = currency_to_index.get(&base) {
//                     if idx != target_curve {
//                         deps.insert(idx.clone());
//                     }
//                 }
//             }
//             if let Some(quote) = details.receive_currency() {
//                 if let Some(idx) = currency_to_index.get(&quote) {
//                     if idx != target_curve {
//                         deps.insert(idx.clone());
//                     }
//                 }
//             }
//         }

//         Ok(deps.into_iter().collect())
//     }

//     /// Builds a unique mapping from currency to configured curve index.
//     ///
//     /// # Errors
//     /// Returns an error if multiple curves map to the same currency.
//     fn currency_to_curve_index(&self) -> Result<HashMap<Currency, MarketIndex>> {
//         let mut map = HashMap::new();
//         for (idx, spec) in &self.resolved_specs {
//             if let Some(existing) = map.insert(spec.currency, idx.clone()) {
//                 return Err(AtlasError::InvalidValueErr(format!(
//                     "Currency {} mapped to multiple curves: {} and {}",
//                     spec.currency, existing, idx
//                 )));
//             }
//         }
//         Ok(map)
//     }

//     /// Merges current-iteration and previous-iteration curve states.
//     fn merge_curves(
//         &self,
//         current: &HashMap<MarketIndex, CurveState>,
//         previous: &HashMap<MarketIndex, CurveState>,
//     ) -> HashMap<MarketIndex, CurveState> {
//         let mut out = previous.clone();
//         for (k, v) in current {
//             out.insert(k.clone(), v.clone());
//         }
//         out
//     }

//     /// Resolves an FX spot for a pair from local or global specs.
//     ///
//     /// # Errors
//     /// Returns an error if neither direct nor inverse spot is available.
//     fn fx_spot(&self, spec: &ResolvedCurveSpec, base: Currency, quote: Currency) -> Result<f64> {
//         let pair = format!("{base}{quote}");
//         let inv_pair = format!("{quote}{base}");

//         if let Some(v) = spec.fx_spot.get(&pair) {
//             return Ok(*v);
//         }
//         if let Some(v) = spec.fx_spot.get(&inv_pair) {
//             return Ok(1.0 / v);
//         }

//         for other in self.resolved_specs.values() {
//             if let Some(v) = other.fx_spot.get(&pair) {
//                 return Ok(*v);
//             }
//             if let Some(v) = other.fx_spot.get(&inv_pair) {
//                 return Ok(1.0 / v);
//             }
//         }

//         Err(AtlasError::NotFoundErr(format!(
//             "FX spot for pair {} (or inverse) not found",
//             pair
//         )))
//     }

//     /// Computes FX forward outright from spot and discount factors.
//     ///
//     /// # Errors
//     /// Returns an error if required curves or discount factors are missing.
//     fn fx_forward_from_curves(
//         &self,
//         curves: &HashMap<MarketIndex, CurveState>,
//         currency_to_index: &HashMap<Currency, MarketIndex>,
//         base: Currency,
//         quote: Currency,
//         delivery: Date,
//         spot: f64,
//     ) -> Result<f64> {
//         let base_idx = currency_to_index.get(&base).ok_or_else(|| {
//             AtlasError::NotFoundErr(format!("No curve mapped to base currency {base}"))
//         })?;
//         let quote_idx = currency_to_index.get(&quote).ok_or_else(|| {
//             AtlasError::NotFoundErr(format!("No curve mapped to quote currency {quote}"))
//         })?;

//         let df_base = curves
//             .get(base_idx)
//             .ok_or_else(|| AtlasError::NotFoundErr(format!("Missing base curve {}", base_idx)))?
//             .discount_factor(delivery)?;
//         let df_quote = curves
//             .get(quote_idx)
//             .ok_or_else(|| AtlasError::NotFoundErr(format!("Missing quote curve {}", quote_idx)))?
//             .discount_factor(delivery)?;

//         Ok(spot * df_quote / df_base)
//     }

//     /// Converts internal curve states into `DiscountCurveElement` objects.
//     ///
//     /// # Errors
//     /// Returns an error if term-structure construction fails.
//     fn to_curve_elements(
//         &self,
//         curves: &HashMap<MarketIndex, CurveState>,
//     ) -> Result<HashMap<MarketIndex, DiscountCurveElement>> {
//         let mut out = HashMap::new();
//         for (idx, curve) in curves {
//             let spec = self
//                 .resolved_specs
//                 .get(idx)
//                 .ok_or_else(|| AtlasError::NotFoundErr(format!("Missing spec for {idx}")))?;

//             let ad_dfs = curve
//                 .discount_factors
//                 .iter()
//                 .copied()
//                 .map(ADReal::new)
//                 .collect::<Vec<_>>();

//             let ts = DiscountTermStructure::<ADReal>::new(
//                 curve.dates.clone(),
//                 ad_dfs,
//                 curve.day_counter,
//                 curve.interpolator,
//                 curve.enable_extrapolation,
//             )?
//             .with_pillar_labels(curve.labels.clone())?;

//             out.insert(
//                 idx.clone(),
//                 DiscountCurveElement::new(idx.clone(), spec.currency, Rc::new(RefCell::new(ts))),
//             );
//         }

//         Ok(out)
//     }
// }

// /// Adapter that exposes bootstrap residual vectors through the shared solver trait.
// struct BootstrapVectorProblem<'a> {
//     bootstrapper: &'a CurveBootstrapper,
//     target_index: &'a MarketIndex,
//     spec: &'a ResolvedCurveSpec,
//     merged_base: &'a HashMap<MarketIndex, CurveState>,
//     currency_to_index: &'a HashMap<Currency, MarketIndex>,
//     evaluation_date: Date,
// }

// impl ContFunc<[ADReal], Vec<ADReal>> for BootstrapVectorProblem<'_> {
//     /// Evaluates bootstrap residual vector for solver integration.
//     ///
//     /// # Errors
//     /// Returns an error if residual evaluation fails.
//     fn call(&self, x: &[ADReal]) -> Result<Vec<ADReal>> {
//         self.bootstrapper.residual_vector(
//             self.target_index,
//             self.spec,
//             self.merged_base,
//             self.currency_to_index,
//             self.evaluation_date,
//             x,
//         )
//     }
// }

// impl VectorFunc<ADReal, ADReal> for BootstrapVectorProblem<'_> {}

// impl ADJacobian for BootstrapVectorProblem<'_> {}

// #[derive(Clone)]
// struct CurveState {
//     dates: Vec<Date>,
//     discount_factors: Vec<f64>,
//     labels: Vec<String>,
//     day_counter: DayCounter,
//     interpolator: Interpolator,
//     enable_extrapolation: bool,
// }

// impl CurveState {
//     /// Builds a normalized curve state from node vectors.
//     ///
//     /// # Errors
//     /// Returns an error on size mismatch or invalid node values.
//     fn from_nodes(
//         dates: Vec<Date>,
//         discount_factors: Vec<f64>,
//         labels: Vec<String>,
//         day_counter: DayCounter,
//         interpolator: Interpolator,
//         enable_extrapolation: bool,
//     ) -> Result<Self> {
//         if dates.len() != discount_factors.len() || dates.len() != labels.len() {
//             return Err(AtlasError::InvalidValueErr(
//                 "Curve node dates, discount factors and labels size mismatch".into(),
//             ));
//         }

//         for df in &discount_factors {
//             if *df <= 0.0 {
//                 return Err(AtlasError::InvalidValueErr(
//                     "Discount factors must be strictly positive".into(),
//                 ));
//             }
//         }

//         let mut zipped = dates
//             .into_iter()
//             .zip(discount_factors)
//             .zip(labels)
//             .collect::<Vec<_>>();
//         zipped.sort_by_key(|((d, _), _)| *d);

//         Ok(Self {
//             dates: zipped.iter().map(|((d, _), _)| *d).collect(),
//             discount_factors: zipped.iter().map(|((_, v), _)| *v).collect(),
//             labels: zipped.into_iter().map(|(_, l)| l).collect(),
//             day_counter,
//             interpolator,
//             enable_extrapolation,
//         })
//     }

//     /// Returns discount factor at a date via exact lookup or interpolation.
//     ///
//     /// # Errors
//     /// Returns an error if interpolation fails.
//     fn discount_factor(&self, date: Date) -> Result<f64> {
//         if let Some((idx, _)) = self.dates.iter().enumerate().find(|(_, d)| **d == date) {
//             return Ok(self.discount_factors[idx]);
//         }

//         let ts = DiscountTermStructure::<f64>::new(
//             self.dates.clone(),
//             self.discount_factors.clone(),
//             self.day_counter,
//             self.interpolator,
//             self.enable_extrapolation,
//         )?;
//         ts.discount_factor(date)
//     }

//     /// Computes the maximum absolute node move vs another curve state.
//     ///
//     /// # Errors
//     /// Returns an error if either side interpolation fails.
//     fn max_abs_diff(&self, other: &Self) -> Result<f64> {
//         let mut all_dates: HashSet<Date> = self.dates.iter().copied().collect();
//         for d in &other.dates {
//             all_dates.insert(*d);
//         }

//         let mut max_diff: f64 = 0.0;
//         for d in all_dates {
//             max_diff = max_diff.max((self.discount_factor(d)? - other.discount_factor(d)?).abs());
//         }
//         Ok(max_diff)
//     }
// }

// struct BootstrapMarketDataProvider {
//     eval_date: Date,
//     curves: HashMap<MarketIndex, CurveState>,
//     currencies: HashMap<MarketIndex, Currency>,
//     fx_store: ExchangeRateStore,
// }

// impl BootstrapMarketDataProvider {
//     /// Creates an in-memory market-data adapter from bootstrapped curve states.
//     ///
//     /// # Errors
//     /// Returns an error if any curve lacks corresponding specification metadata.
//     fn new(
//         eval_date: Date,
//         curves: &HashMap<MarketIndex, CurveState>,
//         specs: &HashMap<MarketIndex, ResolvedCurveSpec>,
//     ) -> Result<Self> {
//         let mut currencies = HashMap::new();
//         for idx in curves.keys() {
//             let spec = specs
//                 .get(idx)
//                 .ok_or_else(|| AtlasError::NotFoundErr(format!("Missing spec for {}", idx)))?;
//             currencies.insert(idx.clone(), spec.currency);
//         }

//         let mut fx_store = ExchangeRateStore::new();
//         for spec in specs.values() {
//             for (pair, value) in &spec.fx_spot {
//                 if pair.len() != 6 {
//                     continue;
//                 }
//                 let base = pair[0..3].parse::<Currency>();
//                 let quote = pair[3..6].parse::<Currency>();
//                 if let (Ok(base), Ok(quote)) = (base, quote) {
//                     fx_store.add_exchange_rate(base, quote, ADReal::new(*value));
//                 }
//             }
//         }

//         Ok(Self {
//             eval_date,
//             curves: curves.clone(),
//             currencies,
//             fx_store,
//         })
//     }
// }

// impl MarketDataProvider for BootstrapMarketDataProvider {
//     /// Builds market data for the requested constructed elements.
//     ///
//     /// # Errors
//     /// Returns an error if a requested curve/currency cannot be resolved.
//     fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData> {
//         let mut store = ConstructedElementStore::default();

//         let requested_indices: Vec<MarketIndex> =
//             if let Some(constructed) = request.constructed_elements_request() {
//                 constructed
//                     .iter()
//                     .filter_map(|req| match req {
//                         ConstructedElementRequest::DiscountCurve { market_index } => {
//                             Some(market_index.clone())
//                         }
//                         _ => None,
//                     })
//                     .collect()
//             } else {
//                 self.curves.keys().cloned().collect()
//             };

//         for market_index in requested_indices {
//             let curve = self.curves.get(&market_index).ok_or_else(|| {
//                 AtlasError::NotFoundErr(format!("Missing curve {}", market_index))
//             })?;
//             let currency = self.currencies.get(&market_index).copied().ok_or_else(|| {
//                 AtlasError::NotFoundErr(format!("Missing currency for curve {}", market_index))
//             })?;

//             let ad_dfs = curve
//                 .discount_factors
//                 .iter()
//                 .copied()
//                 .map(ADReal::new)
//                 .collect::<Vec<_>>();

//             let ts = DiscountTermStructure::<ADReal>::new(
//                 curve.dates.clone(),
//                 ad_dfs,
//                 curve.day_counter,
//                 curve.interpolator,
//                 curve.enable_extrapolation,
//             )?
//             .with_pillar_labels(curve.labels.clone())?;

//             store.discount_curves_mut().insert(
//                 market_index.clone(),
//                 DiscountCurveElement::new(market_index, currency, Rc::new(RefCell::new(ts))),
//             );
//         }

//         let mut md = MarketData::new(HashMap::new(), store, &[]);
//         if request.needs_exchange_rates() {
//             md = md.with_exchange_rate_store(self.fx_store.clone());
//         }
//         Ok(md)
//     }

//     /// Returns adapter evaluation date.
//     fn evaluation_date(&self) -> Date {
//         self.eval_date
//     }
// }

// #[cfg(test)]
// mod tests {
//     use std::collections::HashMap;

//     use crate::{
//         core::contextmanager::ContextManager,
//         currencies::currency::Currency,
//         indices::marketindex::MarketIndex,
//         math::interpolation::interpolator::Interpolator,
//         quotes::{
//             fixingstore::FixingStore,
//             quote::{Level, Quote, QuoteDetails, QuoteLevels},
//             quotestore::QuoteStore,
//         },
//         rates::bootstrapping::bootstrap::{
//             CalibrationKind, CurveBootstrapper, ResolvedCurveSpec, ResolvedInstrument,
//         },
//         time::{date::Date, daycounter::DayCounter},
//     };

//     #[test]
//     fn bootstrapper_resolves_explicit_dependency_order() {
//         let sofr = "SOFR".parse::<MarketIndex>().expect("valid SOFR index");
//         let term_sofr = "TermSOFR3m"
//             .parse::<MarketIndex>()
//             .expect("valid TermSOFR index");

//         let base_spec = ResolvedCurveSpec {
//             market_index: sofr.clone(),
//             currency: Currency::USD,
//             dependencies: Vec::new(),
//             day_counter: DayCounter::Actual360,
//             interpolator: Interpolator::Linear,
//             enable_extrapolation: false,
//             fx_spot: HashMap::new(),
//             instruments: Vec::new(),
//         };

//         let dependent_spec = ResolvedCurveSpec {
//             market_index: term_sofr.clone(),
//             currency: Currency::CLP,
//             dependencies: vec![sofr.clone()],
//             day_counter: DayCounter::Actual360,
//             interpolator: Interpolator::Linear,
//             enable_extrapolation: false,
//             fx_spot: HashMap::new(),
//             instruments: Vec::new(),
//         };

//         let mut specs = HashMap::new();
//         specs.insert(sofr.clone(), base_spec);
//         specs.insert(term_sofr.clone(), dependent_spec);

//         let bootstrapper = CurveBootstrapper::new(1e-10, 10, specs);
//         let order = bootstrapper
//             .resolve_dependencies()
//             .expect("dependency resolution should succeed");

//         let pos_sofr = order
//             .iter()
//             .position(|x| x == &sofr)
//             .expect("SOFR in order");
//         let pos_term = order
//             .iter()
//             .position(|x| x == &term_sofr)
//             .expect("TermSOFR in order");
//         assert!(pos_sofr < pos_term);
//     }

//     #[test]
//     fn bootstrapper_returns_error_for_curve_without_instruments() {
//         let reference_date = Date::new(2024, 1, 1);
//         let sofr = "SOFR".parse::<MarketIndex>().expect("valid SOFR index");

//         let spec = ResolvedCurveSpec {
//             market_index: sofr.clone(),
//             currency: Currency::USD,
//             dependencies: Vec::new(),
//             day_counter: DayCounter::Actual360,
//             interpolator: Interpolator::Linear,
//             enable_extrapolation: false,
//             fx_spot: HashMap::new(),
//             instruments: Vec::new(),
//         };

//         let mut specs = HashMap::new();
//         specs.insert(sofr, spec);

//         let bootstrapper = CurveBootstrapper::new(1e-10, 5, specs);
//         let ctx = ContextManager::new(QuoteStore::new(reference_date), FixingStore::default());

//         assert!(bootstrapper.bootstrap(&ctx).is_err());
//     }

//     #[test]
//     fn bootstrapper_resolves_dual_curve_dependency_from_fx_forward_points() {
//         let reference_date = Date::new(2024, 1, 1);
//         let usd_curve_index = "SOFR".parse::<MarketIndex>().expect("valid SOFR index");
//         let clp_curve_index = "ICP".parse::<MarketIndex>().expect("valid ICP index");

//         let details: QuoteDetails = "FxForwardPoints_CLPUSD_1M"
//             .parse()
//             .expect("valid fx forward points quote details");
//         let tenor = details
//             .tenor()
//             .expect("tenor present in fx forward points quote");

//         let fx_points_quote = Quote::new(reference_date, details, QuoteLevels::with_mid(0.0));
//         let fx_points_inst = ResolvedInstrument {
//             quote: fx_points_quote,
//             level: Level::Mid,
//             built: None,
//             kind: CalibrationKind::FxForwardPoints,
//             quote_value: 0.0,
//             pillar_date: reference_date + tenor,
//         };

//         let usd_spec = ResolvedCurveSpec {
//             market_index: usd_curve_index.clone(),
//             currency: Currency::USD,
//             dependencies: Vec::new(),
//             day_counter: DayCounter::Actual360,
//             interpolator: Interpolator::Linear,
//             enable_extrapolation: false,
//             fx_spot: HashMap::new(),
//             instruments: Vec::new(),
//         };

//         let clp_spec = ResolvedCurveSpec {
//             market_index: clp_curve_index.clone(),
//             currency: Currency::CLP,
//             dependencies: Vec::new(),
//             day_counter: DayCounter::Actual360,
//             interpolator: Interpolator::Linear,
//             enable_extrapolation: false,
//             fx_spot: HashMap::new(),
//             instruments: vec![fx_points_inst],
//         };

//         let mut specs = HashMap::new();
//         specs.insert(usd_curve_index.clone(), usd_spec);
//         specs.insert(clp_curve_index.clone(), clp_spec);

//         let bootstrapper = CurveBootstrapper::new(1e-10, 10, specs);
//         let order = bootstrapper
//             .resolve_dependencies()
//             .expect("dependency resolution should succeed");

//         let pos_usd = order
//             .iter()
//             .position(|x| x == &usd_curve_index)
//             .expect("USD curve in order");
//         let pos_clp = order
//             .iter()
//             .position(|x| x == &clp_curve_index)
//             .expect("CLP curve in order");
//         assert!(pos_usd < pos_clp);
//     }
// }
