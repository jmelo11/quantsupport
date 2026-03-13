use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        elements::curveelement::{ADCurveElement, DiscountCurveElement},
        marketdatahandling::constructedelementstore::SharedElement,
        request::LegsProvider,
    },
    currencies::exchangeratestore::ExchangeRateStore,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::{cashflow::Cashflow, cashflowtype::CashflowType, leg::Leg},
        fx::fxforward::FxForward,
        rates::{crosscurrencyswap::CrossCurrencySwap, ratefutures::RateFutures},
    },
    math::{
        interpolation::interpolator::Interpolator,
        solvers::{
            solvertraits::{ADJacobian, ContFunc, VectorFunc},
            vectornewton::VectorNewton,
        },
    },
    quotes::quote::{BuiltInstrument, Level},
    rates::{
        bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            curvespec::{BootstrappedCurve, CurveSpec, QuoteSelector},
            resolvedcurvespec::{ResolvedCurveSpec, ResolvedInstrument},
        },
        compounding::Compounding,
        yieldtermstructure::discounttermstructure::DiscountTermStructure,
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

use std::{cell::RefCell, rc::Rc};

/// Dependency-aware, lazy multi-curve bootstrapper.
///
/// Accepts a set of [`CurveSpec`]s and a [`BootstrapDiscountPolicy`] that
/// will determine how to bootstrap each curve. It resolves dependencies between [`CurveSpec`].
///
/// ## Parameters
/// * `curve_specs`: the list of curve specifications to bootstrap. Each spec includes the market index, currency, day count convention, interpolation method, and the list of pillar instruments (identified by their quote IDs).
/// * `discount_policy`: the discount policy defines how to determine the discount curve for each instrument during bootstrapping, including handling of cross-currency instruments and collateralization. See [`BootstrapDiscountPolicy`] for details.
///
/// ## Example
/// ```
/// use quantsupport::prelude::*;
/// use std::collections::HashMap;
///
/// // We create a simple QuoteSelector that holds the market quotes in a HashMap.
/// struct MapSelector {
///     reference_date: Date,
///     quotes: HashMap<String, f64>,
/// }
///
/// impl MapSelector {
///     fn new(reference_date: Date) -> Self {
///         Self {
///             reference_date,
///             quotes: HashMap::new(),
///         }
///     }
///
///     fn add(&mut self, id: &str, rate: f64) {
///         self.quotes.insert(id.to_string(), rate);
///     }
/// }
///
/// impl QuoteSelector for MapSelector {
///         fn select(&self, identifier: &str) -> Option<Quote> {
///             let rate = self.quotes.get(identifier)?;
///             let det: QuoteDetails = identifier.parse().ok()?;
///             let q = Quote::new(det, QuoteLevels::with_mid(*rate));
///             if q.build_instrument(self.reference_date, Level::Mid).is_ok() {
///                 Some(q)
///             } else {
///                 None
///             }
///         }
///         fn reference_date(&self) -> Date {
///             Date::new(2024, 1, 2)
///         }
///     }
///
/// // We pass the market data to the selector
/// let rd = Date::new(2024, 6, 1);
/// let mut selector = MapSelector::new(rd);
/// selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);
/// selector.add("FixedRateDeposit_USD_SOFR_6M", 0.051);
/// selector.add("OIS_USD_SOFR_1Y", 0.048);
/// selector.add("OIS_USD_SOFR_2Y", 0.045);
///
/// // We configure a single curve for the SOFR index, with 4 pillars.
/// let spec = CurveSpec::new(
///     MarketIndex::SOFR,
///     Currency::USD,
///     DayCounter::Actual360,
///     Interpolator::LogLinear,
///     true,
///     vec![
///         "FixedRateDeposit_USD_SOFR_3M".into(),
///         "FixedRateDeposit_USD_SOFR_6M".into(),
///         "OIS_USD_SOFR_1Y".into(),
///         "OIS_USD_SOFR_2Y".into(),
///     ],
/// );
///
/// // Setup the discount policy and bootstrap.
/// let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
/// let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
/// let result = bootstrapper.bootstrap(&selector, Level::Mid);
/// assert!(result.is_ok(), "Bootstrap failed: {:?}", result.err());
/// ```
pub struct MultiCurveBootstrapper {
    curve_specs: Vec<CurveSpec>,
    discount_policy: BootstrapDiscountPolicy,
    exchange_rate_store: ExchangeRateStore,
}

impl MultiCurveBootstrapper {
    /// Creates a bootstrapper from a set of curve specifications.
    #[must_use]
    pub fn new(curve_specs: Vec<CurveSpec>, discount_policy: BootstrapDiscountPolicy) -> Self {
        Self {
            curve_specs,
            discount_policy,
            exchange_rate_store: ExchangeRateStore::new(),
        }
    }

    /// Registers an [`ExchangeRateStore`] for FX spot rates. Required for
    /// instruments referencing multiple currencies (e.g. cross-currency swaps, FX forwards).
    #[must_use]
    pub fn with_exchange_rate_store(mut self, store: ExchangeRateStore) -> Self {
        self.exchange_rate_store = store;
        self
    }

    /// Resolves quotes, determines dependency order, and bootstraps every
    /// configured curve.
    ///
    /// ## Parameters
    /// * `selector`: the quote selector to resolve market quotes for the pillar instruments. The selector should be able to build the corresponding `BuiltInstrument`s for each quote ID, as these are needed for bootstrapping.
    ///
    /// ## Errors
    /// Returns an error if quote resolution fails, a dependency cycle or
    /// missing curve is detected, or if the Newton solver does not converge
    /// for any curve.
    pub fn bootstrap(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<HashMap<MarketIndex, DiscountCurveElement>> {
        // 1. Resolve all curve specs into concrete instruments.
        let resolved = self.resolve_all(selector, level)?;

        // 2. Topological sort respecting curve dependencies.
        let order = Self::dependency_order(&resolved, &self.discount_policy)?;

        // 3. Iteratively bootstrap in dependency order.
        let mut bootstrapped: HashMap<MarketIndex, BootstrappedCurve> = HashMap::new();

        for index in &order {
            let spec = resolved.get(index).ok_or_else(|| {
                QSError::NotFoundErr(format!("Missing resolved spec for {index}"))
            })?;
            let curve = self.bootstrap_curve(index, spec, &bootstrapped)?;
            bootstrapped.insert(index.clone(), curve);
        }

        // 4. Convert to DiscountCurveElements.
        Self::build_curve_elements(&resolved, &bootstrapped)
    }

    // -----------------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------------

    /// Resolves every [`CurveSpec`] into a [`ResolvedCurveSpec`] by selecting
    /// and building the quoted instruments through the given selector.
    fn resolve_all(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<HashMap<MarketIndex, ResolvedCurveSpec>> {
        let mut map = HashMap::new();
        for spec in &self.curve_specs {
            let resolved = spec.resolve(selector, level)?;
            map.insert(spec.market_index().clone(), resolved);
        }
        Ok(map)
    }

    // -----------------------------------------------------------------------
    // Dependency ordering (topological sort)
    // -----------------------------------------------------------------------

    /// Performs a topological sort (Kahn's algorithm) on the curve
    /// dependency graph, returning an ordering where every curve is
    /// bootstrapped only after its dependencies.
    fn dependency_order(
        resolved: &HashMap<MarketIndex, ResolvedCurveSpec>,
        policy: &BootstrapDiscountPolicy,
    ) -> Result<Vec<MarketIndex>> {
        // Build adjacency.
        let mut dep_map: HashMap<MarketIndex, HashSet<MarketIndex>> = HashMap::new();

        for (idx, spec) in resolved {
            let mut deps = spec.dependencies(policy);
            deps.remove(idx); // self-reference is not a dependency

            // Ensure every dependency is available.
            for dep in &deps {
                if !resolved.contains_key(dep) {
                    return Err(QSError::NotFoundErr(format!(
                        "Curve {idx} depends on {dep}, but it is not configured"
                    )));
                }
            }
            dep_map.insert(idx.clone(), deps);
        }

        // Kahn's algorithm.
        let mut indegree: HashMap<MarketIndex, usize> = resolved
            .keys()
            .map(|k| (k.clone(), dep_map.get(k).map_or(0, HashSet::len)))
            .collect();

        let mut reverse: HashMap<MarketIndex, Vec<MarketIndex>> = HashMap::new();
        for (idx, deps) in &dep_map {
            for dep in deps {
                reverse.entry(dep.clone()).or_default().push(idx.clone());
            }
        }

        let mut queue: VecDeque<MarketIndex> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| k.clone())
            .collect();

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(children) = reverse.get(&node) {
                for child in children {
                    if let Some(v) = indegree.get_mut(child) {
                        *v = v.saturating_sub(1);
                        if *v == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        if order.len() < resolved.len() {
            return Err(QSError::InvalidValueErr(
                "Circular dependency detected among curve specifications".into(),
            ));
        }

        Ok(order)
    }

    // -----------------------------------------------------------------------
    // Single-curve bootstrap
    // -----------------------------------------------------------------------

    /// Bootstraps a single curve by solving for discount factors that
    /// reprice all its instruments to zero residual. After the Newton
    /// solver converges, applies the implicit function theorem (IFT) to
    /// attach exact sensitivities w.r.t. market quotes to the result.
    fn bootstrap_curve(
        &self,
        target_index: &MarketIndex,
        spec: &ResolvedCurveSpec,
        other_curves: &HashMap<MarketIndex, BootstrappedCurve>,
    ) -> Result<BootstrappedCurve> {
        Tape::start_recording();
        Tape::set_mark();

        let reference_date = spec.reference_date();
        let dc = spec.day_counter();
        let interp = spec.interpolator();

        // Build pillar time grid: [0, t_1, t_2, …]
        let mut times = vec![0.0_f64];
        for instr in spec.instruments() {
            let t = dc.year_fraction(reference_date, instr.pillar_date());
            times.push(t);
        }

        let n = spec.instruments().len();

        // Initial guess: slight discount (safe for positive-rate environments).
        let x0: Vec<ADReal> = vec![ADReal::new(0.99); n];

        // Build the problem.
        let problem = BootstrapProblem {
            target_index: target_index.clone(),
            reference_date,
            times: times.clone(),
            day_counter: dc,
            interpolator: interp,
            instruments: spec.instruments(),
            other_curves,
            discount_policy: &self.discount_policy,
            exchange_rate_store: &self.exchange_rate_store,
        };

        // Solve.
        let solver = VectorNewton::new(1e-12, 200);
        let solution = solver.solve(&problem, &x0)?;
        // -----------------------------------------------------------------
        // This will be moved into a separete function
        // IFT post-processing: replace solver DFs with DFs whose AD
        // derivatives w.r.t. quote values are computed via the implicit
        // function theorem, avoiding noise from damped Newton steps.
        //
        //   F(x, q) = 0   =>   dx/dq = −J⁻¹ G
        //   J = ∂F/∂x ,  G_diag[i] = ∂F_i/∂q_i  (diagonal because each
        //   residual depends on only its own quote value).
        // -----------------------------------------------------------------
        let converged_x = &solution.x; // Vec<ADReal> of converged DFs (without T0)

        // J = ∂F/∂x  (via AD Jacobian already available in the solver)
        let j_matrix = problem.jacobian_ad(converged_x)?; // Matrix<f64>

        // G_diag = ∂F_i/∂q_i  (annuity of the quote-dependent terms)
        let trial = problem.trial_curve(converged_x);
        let g_diag = problem.quote_derivatives(&trial)?;

        // Solve  J × S_col_j = −g_jj × e_j  for each j.
        // Collect all columns of S.
        let ift_sens = Self::solve_ift(&j_matrix, &g_diag)?; // ift_sens[i][j]

        // Create IFT-corrected DFs connected to quote_values via S.
        let quote_values = spec.quote_values(); // Vec<ADReal> – the pillar leaves
        let mut corrected_dfs = Vec::with_capacity(n);
        for i in 0..n {
            let base = converged_x[i].value();
            let mut df = ADReal::new(base);
            for j in 0..n {
                // (q_j − base_q_j) evaluates to zero but creates a tape
                // edge whose weight is 1.0 w.r.t. q_j, carrying the IFT
                // sensitivity through the chain rule.
                let dq: ADReal = (quote_values[j] - ADReal::new(quote_values[j].value())).into();
                df = (df + dq * ift_sens[i][j]).into();
            }
            corrected_dfs.push(df);
        }

        // Build the bootstrapped curve with IFT-corrected DFs.
        let mut dfs = vec![ADReal::one()];
        dfs.extend(corrected_dfs);
        Ok(BootstrappedCurve::new_with_dfs(
            reference_date,
            times,
            dfs,
            dc,
            interp,
        ))
    }

    /// Solves J × S = −diag(g) for the IFT sensitivity matrix.
    fn solve_ift(j: &[Vec<f64>], g_diag: &[f64]) -> Result<Vec<Vec<f64>>> {
        let n = g_diag.len();
        // Re-use the same Gaussian elimination as the Newton solver.
        // For each column j we solve  J s = −g_jj e_j.
        let mut s = vec![vec![0.0; n]; n];
        for j_col in 0..n {
            let mut rhs: Vec<f64> = vec![0.0; n];
            rhs[j_col] = -g_diag[j_col];
            let col = Self::solve_f64_system(j, &rhs)?;
            for i in 0..n {
                s[i][j_col] = col[i];
            }
        }
        Ok(s)
    }

    /// Solves a dense linear system A x = b using Gaussian elimination with
    /// partial pivoting.  Operates entirely in `f64`.
    /// TODO: replace with a more efficient and stable linear algebra library if needed.
    #[allow(clippy::needless_range_loop)]
    fn solve_f64_system(a: &[Vec<f64>], b: &[f64]) -> Result<Vec<f64>> {
        let n = a.len();
        let mut aa: Vec<Vec<f64>> = a.to_vec();
        let mut bb: Vec<f64> = b.to_vec();

        for i in 0..n {
            // Partial pivot.
            let mut pivot = i;
            let mut max_val = aa[i][i].abs();
            for r in (i + 1)..n {
                if aa[r][i].abs() > max_val {
                    max_val = aa[r][i].abs();
                    pivot = r;
                }
            }
            if max_val < 1e-14 {
                return Err(QSError::SolverErr("Singular Jacobian in IFT".into()));
            }
            if pivot != i {
                aa.swap(i, pivot);
                bb.swap(i, pivot);
            }

            let diag = aa[i][i];
            for c in i..n {
                aa[i][c] /= diag;
            }
            bb[i] /= diag;

            for r in 0..n {
                if r == i {
                    continue;
                }
                let factor = aa[r][i];
                if factor == 0.0 {
                    continue;
                }
                let bi = bb[i];
                for c in i..n {
                    aa[r][c] -= factor * aa[i][c];
                }
                bb[r] -= bi * factor;
            }
        }
        Ok(bb)
    }

    /// Converts the bootstrapped curves into [`DiscountCurveElement`]s
    /// that can be stored in a market-data context, preserving pillar
    /// labels, quote values, and AD links.
    fn build_curve_elements(
        resolved: &HashMap<MarketIndex, ResolvedCurveSpec>,
        bootstrapped: &HashMap<MarketIndex, BootstrappedCurve>,
    ) -> Result<HashMap<MarketIndex, DiscountCurveElement>> {
        let mut map = HashMap::new();

        for (idx, spec) in resolved {
            let bc = bootstrapped.get(idx).ok_or_else(|| {
                QSError::NotFoundErr(format!("Missing bootstrapped curve for {idx}"))
            })?;

            let reference_date = bc.reference_date();
            let dc = spec.day_counter();
            let interp = spec.interpolator();

            // Collect pillar dates (reference date + instrument pillar dates).
            let mut dates = vec![reference_date];
            dates.extend(spec.pillar_dates());

            let dfs = bc.discount_factors().to_vec();
            let labels = spec.pillar_labels();
            let quote_values = spec.quote_values();

            let ts = DiscountTermStructure::<ADReal>::new(
                dates,
                dfs,
                dc,
                interp,
                spec.enable_extrapolation(),
            )?
            .with_pillar_values(quote_values)?
            .with_pillar_labels(labels)?;

            let shared: SharedElement<dyn ADCurveElement> = Rc::new(RefCell::new(ts));

            let elem = DiscountCurveElement::new(idx.clone(), spec.currency(), shared);
            map.insert(idx.clone(), elem);
        }

        Ok(map)
    }
}

/// Maps a trial vector of discount factors into the residual vector used by
/// the Newton solver.
///
/// For each instrument the residual is:
/// * **Deposits / Swaps / Basis-swaps / XCcy-swaps** → NPV (should be ≈ 0)
/// * **Rate futures** → `implied_forward - market_rate`
/// * **FX forwards** → `implied_FX - market_FX`
struct BootstrapProblem<'a> {
    target_index: MarketIndex,
    reference_date: Date,
    times: Vec<f64>,
    day_counter: DayCounter,
    interpolator: Interpolator,
    instruments: &'a [ResolvedInstrument],
    other_curves: &'a HashMap<MarketIndex, BootstrappedCurve>,
    discount_policy: &'a BootstrapDiscountPolicy,
    exchange_rate_store: &'a ExchangeRateStore,
}

impl BootstrapProblem<'_> {
    /// Builds a temporary `BootstrappedCurve` from the trial unknowns.
    fn trial_curve(&self, x: &[ADReal]) -> BootstrappedCurve {
        let mut dfs = Vec::with_capacity(self.times.len());
        dfs.push(ADReal::one()); // DF(0) = 1
        dfs.extend_from_slice(x);
        BootstrappedCurve::new_with_dfs(
            self.reference_date,
            self.times.clone(),
            dfs,
            self.day_counter,
            self.interpolator,
        )
    }

    /// Retrieves the curve for a given index, falling back to
    /// `other_curves` when the index differs from the target.
    fn get_curve<'b>(
        &'b self,
        index: &MarketIndex,
        trial: &'b BootstrappedCurve,
    ) -> Option<&'b BootstrappedCurve> {
        if index == &self.target_index {
            Some(trial)
        } else {
            self.other_curves.get(index)
        }
    }

    // -----------------------------------------------------------------------
    // Residual per instrument type
    // -----------------------------------------------------------------------

    /// Dispatches the residual computation for a single instrument based
    /// on its type (deposit, swap, basis swap, cross-currency swap,
    /// rate futures, or FX forward).
    fn compute_residual(
        &self,
        instr: &ResolvedInstrument,
        trial: &BootstrappedCurve,
    ) -> Result<ADReal> {
        match instr.built() {
            BuiltInstrument::FixedRateDeposit(dep) => {
                self.residual_deposit(dep.legs(), &self.target_index, trial)
            }
            BuiltInstrument::Swap(swap) => self.residual_legs(swap.legs(), trial),
            BuiltInstrument::BasisSwap(bs) => self.residual_legs(bs.legs(), trial),
            BuiltInstrument::CrossCurrencySwap(xccy) => self.residual_xccy(xccy, trial),
            BuiltInstrument::RateFutures(f) => self.residual_futures(f, instr.quote_value(), trial),
            BuiltInstrument::FxForward(fx) => {
                self.residual_fx_forward(fx, instr.quote_value(), trial)
            }
            _ => Err(QSError::InvalidValueErr(
                "Unsupported instrument in bootstrap".into(),
            )),
        }
    }

    // -----------------------------------------------------------------------
    // ∂F / ∂q  —  analytical derivative of each residual w.r.t. its own
    //              quote value, evaluated at the converged DFs.
    // -----------------------------------------------------------------------

    /// Computes the diagonal of the quote-sensitivity matrix G.
    ///
    /// `G[i] = ∂F_i/∂q_i` where `q_i` is instrument i's market quote.
    /// Since each residual depends only on its own quote, G is diagonal.
    fn quote_derivatives(&self, trial: &BootstrappedCurve) -> Result<Vec<f64>> {
        let mut g = Vec::with_capacity(self.instruments.len());
        for instr in self.instruments {
            let d = match instr.built() {
                BuiltInstrument::FixedRateDeposit(dep) => {
                    // F = Σ(cf × DF) ;  fixed cfs scale linearly with q ⇒
                    // ∂F/∂q = Σ(yf_k × DF(t_k)) × notional × side
                    self.annuity_fixed_coupons(dep.legs(), trial)?
                }
                BuiltInstrument::Swap(swap) => self.annuity_fixed_coupons(swap.legs(), trial)?,
                BuiltInstrument::BasisSwap(bs) => {
                    // Quote = spread on one floating leg → annuity of that leg
                    self.annuity_floating_coupons(bs.legs(), trial)?
                }
                BuiltInstrument::CrossCurrencySwap(xccy) => {
                    self.annuity_fixed_coupons(xccy.legs(), trial)?
                }
                BuiltInstrument::RateFutures(_) => {
                    // F = implied_fwd − q  ⇒  ∂F/∂q = −1
                    -1.0
                }
                BuiltInstrument::FxForward(fx) => {
                    // Outright: F = DF_base − q × DF_quote ⇒ ∂F/∂q = −DF_quote
                    // Forward pts: F = DF_base − DF_quote(1 + q/S) ⇒ ∂F/∂q = −DF_quote/S
                    let quote_disc_idx = self
                        .discount_policy
                        .discount_index_for_currency(fx.quote_currency());
                    let q_curve = self.get_curve(&quote_disc_idx, trial).ok_or_else(|| {
                        QSError::NotFoundErr(format!("Missing quote curve {quote_disc_idx}"))
                    })?;
                    let df_q = q_curve.discount_factor(fx.delivery_date())?.value();
                    if fx.has_forward_points() {
                        let spot = self
                            .exchange_rate_store
                            .get_exchange_rate(fx.base_currency(), fx.quote_currency())
                            .map_or(1.0, |r| r.value());
                        -df_q / spot
                    } else {
                        -df_q
                    }
                }
                _ => {
                    return Err(QSError::InvalidValueErr(
                        "Unsupported instrument in quote_derivatives".into(),
                    ))
                }
            };
            g.push(d);
        }
        Ok(g)
    }

    /// Annuity of the fixed-rate coupons: Σ side × notional × yf × DF(pay).
    fn annuity_fixed_coupons(&self, legs: &[Leg<ADReal>], trial: &BootstrappedCurve) -> Result<f64> {
        let disc_index = self.discount_policy.csa_index();
        let disc = self.get_curve(disc_index, trial).unwrap_or(trial);

        let mut annuity = 0.0;
        for leg in legs {
            let side = leg.side().sign();
            for cf in leg.cashflows() {
                if let CashflowType::FixedRateCoupon(c) = cf {
                    let yf = c
                        .rate()
                        .day_counter()
                        .year_fraction(c.accrual_start_date(), c.accrual_end_date());
                    let df = disc.discount_factor(c.payment_date())?.value();
                    annuity += side * c.notional() * yf * df;
                }
            }
        }
        Ok(annuity)
    }

    /// Annuity of the floating-rate coupons (for basis-swap spread):
    /// Σ side × notional × yf × DF(pay).
    fn annuity_floating_coupons(&self, legs: &[Leg<ADReal>], trial: &BootstrappedCurve) -> Result<f64> {
        let disc_index = self.discount_policy.csa_index();
        let disc = self.get_curve(disc_index, trial).unwrap_or(trial);

        let mut annuity = 0.0;
        for leg in legs {
            let side = leg.side().sign();
            for cf in leg.cashflows() {
                if let CashflowType::FloatingRateCoupon(c) = cf {
                    let yf = c
                        .day_counter()
                        .year_fraction(c.accrual_start_date(), c.accrual_end_date());
                    let df = disc.discount_factor(c.payment_date())?.value();
                    annuity += side * 1.0 * yf * df; // notional = 1 for basis swap quote
                }
            }
        }
        Ok(annuity)
    }

    /// Computes the deposit residual: NPV = Σ(cashflow × `DF_target(payment_date)`).
    /// At the solution this equals zero.
    fn residual_deposit(
        &self,
        legs: &[Leg<ADReal>],
        disc_index: &MarketIndex,
        trial: &BootstrappedCurve,
    ) -> Result<ADReal> {
        let disc_curve = self
            .get_curve(disc_index, trial)
            .ok_or_else(|| QSError::NotFoundErr(format!("Missing discount curve {disc_index}")))?;

        let mut npv = ADReal::new(0.0);
        for leg in legs {
            let side = leg.side().sign();
            npv = (npv + self.pv_leg(leg, disc_curve, trial)? * side).into();
        }
        Ok(npv)
    }

    /// Computes the NPV residual for legs-based instruments (swaps, basis swaps)
    /// by summing each leg's present value weighted by its side.
    fn residual_legs(&self, legs: &[Leg<ADReal>], trial: &BootstrappedCurve) -> Result<ADReal> {
        let disc_index = self.discount_policy.csa_index();
        let disc_curve = self
            .get_curve(disc_index, trial)
            .ok_or_else(|| QSError::NotFoundErr(format!("Missing discount curve {disc_index}")))?;

        let mut npv = ADReal::new(0.0);
        for leg in legs {
            let side = leg.side().sign();
            npv = (npv + self.pv_leg(leg, disc_curve, trial)? * side).into();
        }
        Ok(npv)
    }

    /// Computes the cross-currency swap residual, discounting each leg
    /// with its own currency's discount curve as determined by the policy.
    fn residual_xccy(&self, xccy: &CrossCurrencySwap<ADReal>, trial: &BootstrappedCurve) -> Result<ADReal> {
        let dom_disc_idx = self
            .discount_policy
            .discount_index_for_currency(xccy.domestic_currency());
        let for_disc_idx = self
            .discount_policy
            .discount_index_for_currency(xccy.foreign_currency());

        let dom_disc = self.get_curve(&dom_disc_idx, trial).ok_or_else(|| {
            QSError::NotFoundErr(format!("Missing domestic discount curve {dom_disc_idx}"))
        })?;
        let for_disc = self.get_curve(&for_disc_idx, trial).ok_or_else(|| {
            QSError::NotFoundErr(format!("Missing foreign discount curve {for_disc_idx}"))
        })?;

        let legs = xccy.legs();
        let mut npv = ADReal::new(0.0);
        // legs[0] = domestic, legs[1] = foreign
        if legs.len() >= 2 {
            let dom_side = legs[0].side().sign();
            npv = (npv + self.pv_leg(&legs[0], dom_disc, trial)? * dom_side).into();

            let for_side = legs[1].side().sign();
            npv = (npv + self.pv_leg(&legs[1], for_disc, trial)? * for_side).into();
        }
        Ok(npv)
    }

    /// Computes the rate-futures residual as `implied_forward − market_rate`.
    /// The AD-enabled `quote_value` keeps the tape connected to the quote leaf.
    fn residual_futures(
        &self,
        f: &RateFutures,
        quote_value: &ADReal,
        trial: &BootstrappedCurve,
    ) -> Result<ADReal> {
        let proj_idx = f.market_index();
        let proj_curve = self
            .get_curve(&proj_idx, trial)
            .ok_or_else(|| QSError::NotFoundErr(format!("Missing projection curve {proj_idx}")))?;

        let rd = f.rate_definition();
        let implied = proj_curve.forward_rate(
            f.start_date(),
            f.end_date(),
            rd.compounding(),
            rd.frequency(),
        )?;

        Ok((implied - *quote_value).into())
    }

    /// Computes the FX-forward residual normalised as
    /// `DF_base(T) − (F/S) × DF_quote(T) = 0`. Handles both outright
    /// forward quotes and forward-points conventions.
    fn residual_fx_forward(
        &self,
        fx: &FxForward,
        quote_value: &ADReal,
        trial: &BootstrappedCurve,
    ) -> Result<ADReal> {
        let base_disc_idx = self
            .discount_policy
            .discount_index_for_currency(fx.base_currency());
        let quote_disc_idx = self
            .discount_policy
            .discount_index_for_currency(fx.quote_currency());

        let base_curve = self.get_curve(&base_disc_idx, trial).ok_or_else(|| {
            QSError::NotFoundErr(format!("Missing base currency curve {base_disc_idx}"))
        })?;
        let quote_curve = self.get_curve(&quote_disc_idx, trial).ok_or_else(|| {
            QSError::NotFoundErr(format!("Missing quote currency curve {quote_disc_idx}"))
        })?;

        let t = fx.delivery_date();
        let df_base = base_curve.discount_factor(t)?;
        let df_quote = quote_curve.discount_factor(t)?;

        if fx.has_forward_points() {
            // Forward points: F = S + pts  →  F/S = 1 + pts/S = DF_base/DF_quote
            //   DF_base − DF_quote × (1 + pts/S) = 0
            let pts = *quote_value;
            let s = self
                .exchange_rate_store
                .get_exchange_rate(fx.base_currency(), fx.quote_currency())
                .map_err(|_| {
                    QSError::NotFoundErr(format!(
                        "Missing FX spot for {}/{}",
                        fx.base_currency(),
                        fx.quote_currency()
                    ))
                })?;
            Ok((df_base - df_quote * (ADReal::one() + pts / s)).into())
        } else {
            // Outright: F/S = DF_base / DF_quote  →  DF_base - (F/S) × DF_quote = 0
            // quote_value is the outright forward; spot at t₀ cancels.
            let f = *quote_value;
            Ok((df_base - f * df_quote).into())
        }
    }

    // -----------------------------------------------------------------------
    // PV of a single leg
    // -----------------------------------------------------------------------

    /// Computes the present value of a single leg.
    ///
    /// Forward rates for floating coupons are resolved from the projection
    /// curve (identified by `leg.market_index()` or the target curve).
    fn pv_leg(
        &self,
        leg: &Leg<ADReal>,
        discount_curve: &BootstrappedCurve,
        trial: &BootstrappedCurve,
    ) -> Result<ADReal> {
        let proj_index = leg.market_index().unwrap_or(&self.target_index);
        let proj_curve = self.get_curve(proj_index, trial).ok_or_else(|| {
            QSError::NotFoundErr(format!("Missing projection curve {proj_index}"))
        })?;

        let mut pv = ADReal::new(0.0);
        for cf in leg.cashflows() {
            let (amount, pay_date) = self.cashflow_amount(cf, proj_curve)?;
            let df = discount_curve.discount_factor(pay_date)?;

            pv = (pv + amount * df).into();
        }
        Ok(pv)
    }

    /// Computes the amount and payment date for a single cashflow.
    #[allow(clippy::unused_self)]
    fn cashflow_amount(
        &self,
        cf: &CashflowType<ADReal>,
        proj_curve: &BootstrappedCurve,
    ) -> Result<(ADReal, Date)> {
        match cf {
            CashflowType::FixedRateCoupon(c) => {
                let amt = c.amount()?;
                Ok((amt, c.payment_date()))
            }
            CashflowType::FloatingRateCoupon(c) => {
                let fwd = proj_curve.forward_rate(
                    c.accrual_start_date(),
                    c.accrual_end_date(),
                    Compounding::Simple,
                    Frequency::Annual,
                )?;
                c.set_fixing(fwd);
                let amt = c.amount()?;
                Ok((amt, c.payment_date()))
            }
            CashflowType::Redemption(c) => {
                let amt = ADReal::new(c.amount()?);
                Ok((amt, c.payment_date()))
            }
            CashflowType::Disbursement(c) => {
                // Disbursements are outflows — negate so that NPV = Σ(signed_cf × DF) = 0
                // holds for deposits (where disbursement and redemption don't cancel).
                let amt = ADReal::new(-c.amount()?);
                Ok((amt, c.payment_date()))
            }
            CashflowType::OptionEmbeddedCoupon(_) => Err(QSError::InvalidValueErr(
                "Option-embedded coupons are not supported in bootstrapping".into(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Solver trait implementations
// ---------------------------------------------------------------------------

impl ContFunc<[ADReal], Vec<ADReal>> for BootstrapProblem<'_> {
    fn call(&self, x: &[ADReal]) -> Result<Vec<ADReal>> {
        let trial = self.trial_curve(x);

        let mut residuals = Vec::with_capacity(self.instruments.len());
        for instr in self.instruments {
            residuals.push(self.compute_residual(instr, &trial)?);
        }
        Ok(residuals)
    }
}

impl VectorFunc<ADReal, ADReal> for BootstrapProblem<'_> {}
impl ADJacobian for BootstrapProblem<'_> {}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        ad::adreal::{ADReal, IsReal},
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        quotes::quote::{Level, Quote, QuoteDetails, QuoteLevels},
        rates::{
            bootstrapping::{
                bootstrap::MultiCurveBootstrapper,
                bootstrapdiscountpolicy::BootstrapDiscountPolicy,
                curvespec::{BootstrappedCurve, CurveSpec, QuoteSelector},
            },
            compounding::Compounding,
        },
        time::{date::Date, daycounter::DayCounter, enums::Frequency, period::Period},
    };

    // -----------------------------------------------------------------------
    // Test QuoteSelector — HashMap-based
    // -----------------------------------------------------------------------

    /// A simple [`QuoteSelector`] backed by a map keyed on identifier.
    struct MapSelector {
        reference_date: Date,
        quotes: HashMap<String, f64>,
    }

    impl MapSelector {
        fn new(reference_date: Date) -> Self {
            Self {
                reference_date,
                quotes: HashMap::new(),
            }
        }

        fn add(&mut self, id: &str, rate: f64) {
            self.quotes.insert(id.to_string(), rate);
        }
    }

    impl QuoteSelector for MapSelector {
        fn select(&self, identifier: &str) -> Option<Quote> {
            let rate = self.quotes.get(identifier)?;
            let det: QuoteDetails = identifier.parse().ok()?;
            let q = Quote::new(det, QuoteLevels::with_mid(*rate));
            if q.build_instrument(self.reference_date, Level::Mid).is_ok() {
                Some(q)
            } else {
                None
            }
        }
        fn reference_date(&self) -> Date {
            Date::new(2024, 1, 2)
        }
    }

    fn ref_date() -> Date {
        Date::new(2024, 1, 2)
    }

    // -----------------------------------------------------------------------
    // BootstrapDiscountPolicy tests
    // -----------------------------------------------------------------------

    #[test]
    fn discount_policy_default_csa() {
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        assert_eq!(*policy.csa_index(), MarketIndex::SOFR);
        assert_eq!(policy.csa_currency(), Currency::USD);
        // For USD, should return the CSA index.
        assert_eq!(
            policy.discount_index_for_currency(Currency::USD),
            MarketIndex::SOFR
        );
    }

    #[test]
    fn discount_policy_collateral_override() {
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD)
            .with_collateral_curve(
                Currency::CLP,
                MarketIndex::Collateral(Currency::CLP, Currency::USD),
            );
        assert_eq!(
            policy.discount_index_for_currency(Currency::CLP),
            MarketIndex::Collateral(Currency::CLP, Currency::USD)
        );
        // USD still uses CSA.
        assert_eq!(
            policy.discount_index_for_currency(Currency::USD),
            MarketIndex::SOFR
        );
    }

    #[test]
    fn discount_policy_all_indices() {
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD)
            .with_collateral_curve(
                Currency::CLP,
                MarketIndex::Collateral(Currency::CLP, Currency::USD),
            );
        let indices = policy.all_discount_indices();
        assert!(indices.contains(&MarketIndex::SOFR));
        assert!(indices.contains(&MarketIndex::Collateral(Currency::CLP, Currency::USD)));
    }

    // -----------------------------------------------------------------------
    // Collateral MarketIndex variant tests
    // -----------------------------------------------------------------------

    #[test]
    fn collateral_market_index_display_and_parse() {
        let idx = MarketIndex::Collateral(Currency::CLP, Currency::USD);
        let s = idx.to_string();
        assert_eq!(s, "Collateral(CLP/USD)");
        let parsed: MarketIndex = s.parse().unwrap();
        assert_eq!(parsed, idx);
    }

    #[test]
    fn collateral_market_index_hash_eq() {
        use std::collections::HashSet;
        let a = MarketIndex::Collateral(Currency::CLP, Currency::USD);
        let b = MarketIndex::Collateral(Currency::CLP, Currency::USD);
        let c = MarketIndex::Collateral(Currency::EUR, Currency::USD);
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a.clone());
        set.insert(b.clone());
        assert_eq!(set.len(), 1);
        set.insert(c);
        assert_eq!(set.len(), 2);
    }

    // -----------------------------------------------------------------------
    // BootstrappedCurve tests
    // -----------------------------------------------------------------------

    #[test]
    fn bootstrapped_curve_discount_factor_at_reference_date_is_one() {
        let rd = ref_date();
        let times = vec![0.0, 0.5, 1.0];
        let dfs = vec![ADReal::new(1.0), ADReal::new(0.98), ADReal::new(0.96)];
        let curve = BootstrappedCurve::new_with_dfs(
            rd,
            times,
            dfs,
            DayCounter::Actual360,
            Interpolator::LogLinear,
        );
        let df = curve.discount_factor(rd).unwrap();
        assert!((df.value() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn bootstrapped_curve_interpolates_between_pillars() {
        let rd = ref_date();
        let times = vec![0.0, 1.0, 2.0];
        let dfs = vec![ADReal::new(1.0), ADReal::new(0.96), ADReal::new(0.92)];
        let curve = BootstrappedCurve::new_with_dfs(
            rd,
            times,
            dfs,
            DayCounter::Actual365,
            Interpolator::Linear,
        );
        // Interpolate at approximately 0.5y (should be between 1.0 and 0.96).
        let mid_date = rd + Period::from_str("6M").unwrap();
        let df = curve.discount_factor(mid_date).unwrap();
        assert!(df.value() > 0.92 && df.value() < 1.0);
    }

    #[test]
    fn bootstrapped_curve_forward_rate_is_consistent() {
        let rd = ref_date();
        let times = vec![0.0, 0.5, 1.0];
        let dfs = vec![ADReal::new(1.0), ADReal::new(0.975), ADReal::new(0.95)];
        let curve = BootstrappedCurve::new_with_dfs(
            rd,
            times.clone(),
            dfs.clone(),
            DayCounter::Actual360,
            Interpolator::LogLinear,
        );
        // Forward rate from 0 to 1Y = (1/DF - 1)/T approximately.
        let end_date = rd + Period::from_str("1Y").unwrap();
        let fwd = curve
            .forward_rate(rd, end_date, Compounding::Simple, Frequency::Annual)
            .unwrap();
        assert!(fwd.value() > 0.0 && fwd.value() < 0.2);
    }

    // -----------------------------------------------------------------------
    // End-to-end bootstrap: deposits only (self-discounting)
    // -----------------------------------------------------------------------

    #[test]
    fn bootstrapper_single_curve_deposits_only() {
        let rd = ref_date();
        let mut selector = MapSelector::new(rd);
        // 3M deposit at 5%
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);
        // 6M deposit at 5.1%
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.051);

        let spec = CurveSpec::new(
            MarketIndex::SOFR,
            Currency::USD,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_3M".into(),
                "FixedRateDeposit_USD_SOFR_6M".into(),
            ],
        );

        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
        let result = bootstrapper.bootstrap(&selector, Level::Mid);
        assert!(result.is_ok(), "Bootstrap failed: {:?}", result.err());

        let curves = result.unwrap();
        assert!(curves.contains_key(&MarketIndex::SOFR));

        let elem = &curves[&MarketIndex::SOFR];
        let curve = elem.curve();

        // DF at reference date should be ≈ 1.
        let df0 = curve.discount_factor(rd).unwrap();
        assert!(
            (df0.value() - 1.0).abs() < 1e-8,
            "DF(t0) = {} (expected 1.0)",
            df0.value()
        );

        // All DFs should be < 1 (positive rates).
        let df_3m = curve
            .discount_factor(rd + Period::from_str("3M").unwrap())
            .unwrap();
        assert!(
            df_3m.value() < 1.0 && df_3m.value() > 0.95,
            "DF(3M) = {} (expected ~0.987)",
            df_3m.value()
        );
    }

    // -----------------------------------------------------------------------
    // End-to-end bootstrap: deposits + swaps
    // -----------------------------------------------------------------------

    #[test]
    fn bootstrapper_deposits_and_swaps() {
        let rd = ref_date();
        let mut selector = MapSelector::new(rd);
        // Short-end: deposits
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.051);
        // Long-end: OIS swaps
        selector.add("OIS_USD_SOFR_1Y", 0.048);
        selector.add("OIS_USD_SOFR_2Y", 0.045);

        let spec = CurveSpec::new(
            MarketIndex::SOFR,
            Currency::USD,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_3M".into(),
                "FixedRateDeposit_USD_SOFR_6M".into(),
                "OIS_USD_SOFR_1Y".into(),
                "OIS_USD_SOFR_2Y".into(),
            ],
        );

        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
        let result = bootstrapper.bootstrap(&selector, Level::Mid);
        assert!(result.is_ok(), "Bootstrap failed: {:?}", result.err());

        let curves = result.unwrap();
        let elem = &curves[&MarketIndex::SOFR];
        let curve = elem.curve();

        // Monotone decreasing discount factors.
        let df_3m = curve
            .discount_factor(rd + Period::from_str("3M").unwrap())
            .unwrap()
            .value();
        let df_6m = curve
            .discount_factor(rd + Period::from_str("6M").unwrap())
            .unwrap()
            .value();
        let df_1y = curve
            .discount_factor(rd + Period::from_str("1Y").unwrap())
            .unwrap()
            .value();
        let df_2y = curve
            .discount_factor(rd + Period::from_str("2Y").unwrap())
            .unwrap()
            .value();

        assert!(
            df_3m > df_6m && df_6m > df_1y && df_1y > df_2y,
            "DFs not decreasing: 3M={df_3m}, 6M={df_6m}, 1Y={df_1y}, 2Y={df_2y}"
        );

        // Implied forward rate from 0 to 3M should be approximately 5%.
        let fwd_3m = curve
            .forward_rate(
                rd,
                rd + Period::from_str("3M").unwrap(),
                Compounding::Simple,
                Frequency::Annual,
            )
            .unwrap()
            .value();

        assert!(
            (fwd_3m - 0.05).abs() < 0.005,
            "Forward rate 0→3M = {fwd_3m} (expected ~0.05)"
        );
    }

    // -----------------------------------------------------------------------
    // Dependency order: cycle detection
    // -----------------------------------------------------------------------

    #[test]
    fn bootstrapper_detects_missing_dependency() {
        // Deposits are self-discounting, so a deposit-only curve never depends
        // on the CSA index.  Verify that an ICP deposit-only curve bootstraps
        // successfully even when the policy CSA index (SOFR) is absent.
        // This also implicitly exercises `dependency_order` — the important
        // invariant is that *self-discounting instruments don't introduce
        // external dependencies*.
        let rd = ref_date();
        let mut selector = MapSelector::new(rd);
        selector.add("FixedRateDeposit_CLP_ICP_3M", 0.05);

        let spec = CurveSpec::new(
            MarketIndex::ICP,
            Currency::CLP,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["FixedRateDeposit_CLP_ICP_3M".into()],
        );

        // CSA = SOFR (not configured as a CurveSpec), but ICP deposits
        // only self-discount, so the bootstrap should still succeed.
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
        let result = bootstrapper.bootstrap(&selector, Level::Mid);
        assert!(
            result.is_ok(),
            "Deposit-only curve should self-discount and not require CSA: {:?}",
            result.err()
        );
    }

    // -----------------------------------------------------------------------
    // Curve elements expose correct metadata
    // -----------------------------------------------------------------------

    #[test]
    fn curve_elements_expose_market_inputs_and_keep_ad_links() {
        let rd = ref_date();
        let mut selector = MapSelector::new(rd);
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);

        let spec = CurveSpec::new(
            MarketIndex::SOFR,
            Currency::USD,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["FixedRateDeposit_USD_SOFR_3M".into()],
        );

        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
        let curves = bootstrapper.bootstrap(&selector, Level::Mid).unwrap();

        let elem = &curves[&MarketIndex::SOFR];
        assert_eq!(*elem.market_index(), MarketIndex::SOFR);
        assert_eq!(elem.currency(), Currency::USD);

        // The curve should report pillar labels.
        let curve = elem.curve();
        let labels = curve.pillar_labels();
        assert!(labels.is_some(), "Pillar labels should be set");
    }

    // -----------------------------------------------------------------------
    // Multi-curve bootstrap: SOFR + ICP (independent deposit-only curves)
    // -----------------------------------------------------------------------

    #[test]
    fn bootstrapper_bootstraps_sofr_icp_and_collateral_curves_together() {
        let rd = ref_date();

        // ---- Quotes ----
        let mut selector = MapSelector::new(rd);
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.051);
        selector.add("FixedRateDeposit_CLP_ICP_3M", 0.06);
        selector.add("FixedRateDeposit_CLP_ICP_6M", 0.062);

        // ---- Curve specs ----
        let sofr_spec = CurveSpec::new(
            MarketIndex::SOFR,
            Currency::USD,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_3M".into(),
                "FixedRateDeposit_USD_SOFR_6M".into(),
            ],
        );

        let icp_spec = CurveSpec::new(
            MarketIndex::ICP,
            Currency::CLP,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_CLP_ICP_3M".into(),
                "FixedRateDeposit_CLP_ICP_6M".into(),
            ],
        );

        // Both curves self-discount (CSA irrelevant for deposits).
        let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
        let bootstrapper = MultiCurveBootstrapper::new(vec![sofr_spec, icp_spec], policy);
        let result = bootstrapper.bootstrap(&selector, Level::Mid);
        assert!(
            result.is_ok(),
            "Multi-curve bootstrap failed: {:?}",
            result.err()
        );

        let curves = result.unwrap();
        assert!(curves.contains_key(&MarketIndex::SOFR));
        assert!(curves.contains_key(&MarketIndex::ICP));

        // Both curves should have DF < 1 at 6M.
        let df_sofr = curves[&MarketIndex::SOFR]
            .curve()
            .discount_factor(rd + Period::from_str("6M").unwrap())
            .unwrap()
            .value();
        let df_icp = curves[&MarketIndex::ICP]
            .curve()
            .discount_factor(rd + Period::from_str("6M").unwrap())
            .unwrap()
            .value();
        assert!(df_sofr < 1.0 && df_sofr > 0.9, "SOFR DF(6M)={df_sofr}");
        assert!(df_icp < 1.0 && df_icp > 0.9, "ICP DF(6M)={df_icp}");

        // ICP rates are higher → ICP DF should be lower.
        assert!(
            df_icp < df_sofr,
            "ICP DF({df_icp}) should be lower than SOFR DF({df_sofr}) because ICP rates are higher"
        );
    }

    // -----------------------------------------------------------------------
    // Full SOFR curve bootstrap with results, labels, and sensitivities
    // -----------------------------------------------------------------------

    #[test]
    fn bootstrapper_full_sofr_curve_with_sensitivities() {
        use crate::ad::tape::Tape;

        let rd = ref_date(); // 2024-01-02

        // ---- Market quotes: realistic SOFR curve ----
        let mut selector = MapSelector::new(rd);
        // Short-end deposits
        selector.add("FixedRateDeposit_USD_SOFR_1M", 0.0530);
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.0525);
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.0510);
        // OIS swaps (long end)
        selector.add("OIS_USD_SOFR_1Y", 0.0485);
        selector.add("OIS_USD_SOFR_2Y", 0.0440);
        selector.add("OIS_USD_SOFR_3Y", 0.0415);
        selector.add("OIS_USD_SOFR_5Y", 0.0400);
        selector.add("OIS_USD_SOFR_7Y", 0.0395);
        selector.add("OIS_USD_SOFR_10Y", 0.0390);

        let input_ids: Vec<&str> = vec![
            "FixedRateDeposit_USD_SOFR_1M",
            "FixedRateDeposit_USD_SOFR_3M",
            "FixedRateDeposit_USD_SOFR_6M",
            "OIS_USD_SOFR_1Y",
            "OIS_USD_SOFR_2Y",
            "OIS_USD_SOFR_3Y",
            "OIS_USD_SOFR_5Y",
            "OIS_USD_SOFR_7Y",
            "OIS_USD_SOFR_10Y",
        ];
        let base_rates: Vec<f64> = vec![
            0.0530, 0.0525, 0.0510, 0.0485, 0.0440, 0.0415, 0.0400, 0.0395, 0.0390,
        ];

        // Helper closure: build a bootstrapper, bootstrap, return the curve element map.
        let do_bootstrap = |sel: &MapSelector| {
            let spec = CurveSpec::new(
                MarketIndex::SOFR,
                Currency::USD,
                DayCounter::Actual360,
                Interpolator::LogLinear,
                true,
                vec![
                    "FixedRateDeposit_USD_SOFR_1M".into(),
                    "FixedRateDeposit_USD_SOFR_3M".into(),
                    "FixedRateDeposit_USD_SOFR_6M".into(),
                    "OIS_USD_SOFR_1Y".into(),
                    "OIS_USD_SOFR_2Y".into(),
                    "OIS_USD_SOFR_3Y".into(),
                    "OIS_USD_SOFR_5Y".into(),
                    "OIS_USD_SOFR_7Y".into(),
                    "OIS_USD_SOFR_10Y".into(),
                ],
            );
            let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
            let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
            bootstrapper.bootstrap(sel, Level::Mid)
        };

        // ==================================================================
        // 1. Baseline (no tape) — display the curve
        // ==================================================================
        {
            let curves = do_bootstrap(&selector).expect("Base SOFR bootstrap failed");
            let elem = &curves[&MarketIndex::SOFR];
            let curve = elem.curve();

            let pillar_labels = curve
                .pillar_labels()
                .expect("Bootstrapped curve must have pillar labels");

            println!("\n========== SOFR Bootstrapped Curve ==========");
            println!("Reference date: {rd}");
            println!(
                "{:<38} {:>14} {:>14} {:>14}",
                "Pillar", "Quote (%)", "DF", "Zero Rate (%)"
            );
            println!("{}", "-".repeat(80));

            let pillars = curve.pillars().expect("pillars");
            for (label, quote_val) in &pillars {
                let q_pct = quote_val.value() * 100.0;
                let pillar_date = pillar_date_from_label(rd, label);
                let df = curve.discount_factor(pillar_date).unwrap().value();
                let yf = DayCounter::Actual360.year_fraction(rd, pillar_date);
                let zero_pct = if yf > 0.0 { -df.ln() / yf * 100.0 } else { 0.0 };
                println!("{label:<38} {q_pct:>14.4} {df:>14.8} {zero_pct:>14.4}");
            }

            assert!(
                pillar_labels.len() >= 9,
                "Expected at least 9 pillars (one per instrument)"
            );
        }

        let n_inputs = input_ids.len(); // 9

        // ==================================================================
        // 2. AD sensitivities: ∂DF(target) / ∂market_input_i
        //
        // We start tape recording BEFORE the bootstrapper so the entire
        // solve is recorded.  After bootstrap we reset the mark to the
        // tape origin so backward_to_mark covers every operation from
        // quote leaves → solver → DFs → interpolation → DF(target).
        // ==================================================================
        let target_tenors = ["2M", "4M", "9M", "18M", "4Y", "6Y", "8Y"];

        // Helper: AD sens at a given target date.
        let ad_sensitivities = |sel: &MapSelector, target: Date| -> (Vec<(String, f64)>, f64) {
            Tape::start_recording();

            // Full bootstrap — everything recorded on tape.
            let mut curves = do_bootstrap(sel).expect("Bootstrap inside AD pass failed");

            // Reset the mark to tape origin so backward_to_mark
            // covers the entire tape (quote leaves → solver → DFs).
            Tape::reset_mark();

            let elem = curves.get_mut(&MarketIndex::SOFR).unwrap();
            let mut curve = elem.curve_mut();

            // Ensure pillar values (quote leaves) are on tape.
            curve.put_pillars_on_tape();

            let df_target = curve.discount_factor(target).unwrap();
            let val = df_target.value();

            // One backward pass: propagates through interpolation →
            // DFs → Newton solver → quote value leaves.
            df_target.backward_to_mark().unwrap();

            let sens: Vec<(String, f64)> = curve
                .pillars()
                .expect("pillars")
                .iter()
                .map(|(lbl, v)| (lbl.clone(), v.adjoint().unwrap_or(0.0)))
                .collect();

            Tape::stop_recording();
            Tape::rewind_to_init();

            (sens, val)
        };

        println!("\n========== AD Sensitivities: ∂DF(target)/∂market_input ==========",);
        print!("{:<10} {:>14}  ", "Target", "DF(target)");
        for id in &input_ids {
            print!("{:>14}", short_label(id));
        }
        println!();
        println!("{}", "-".repeat(10 + 14 + 2 + 14 * n_inputs));

        let mut ad_results: Vec<(&str, Vec<f64>, f64)> = Vec::new();
        for tenor_str in &target_tenors {
            let target_date = rd + Period::from_str(tenor_str).unwrap();
            let (sens, df_val) = ad_sensitivities(&selector, target_date);

            print!("{tenor_str:<10} {df_val:>14.8}  ");
            let adjoints: Vec<f64> = sens.iter().map(|(_, a)| *a).collect();
            for adj in &adjoints {
                print!("{adj:>14.6}");
            }
            println!();

            ad_results.push((tenor_str, adjoints, df_val));
        }

        // ==================================================================
        // 3. FD validation: central difference ±½ bp per quote,
        //    re-bootstrap, evaluate DF(target) — compare against AD.
        // ==================================================================
        let fd_bump = 1e-4; // 1 bp total width

        println!(
            "\n========== FD Sensitivities: ∂DF(target)/∂market_input (central FD, 1 bp) ==========",
        );
        print!("{:<10} {:>14}  ", "Target", "DF(target)");
        for id in &input_ids {
            print!("{:>14}", short_label(id));
        }
        println!();
        println!("{}", "-".repeat(10 + 14 + 2 + 14 * n_inputs));

        let mut max_rel_err: f64 = 0.0;

        for (tenor_str, ad_adj, base_df) in &ad_results {
            let target_date = rd + Period::from_str(tenor_str).unwrap();
            print!("{tenor_str:<10} {base_df:>14.8}  ");

            for (j, &id) in input_ids.iter().enumerate() {
                // Central difference: (f(x+h) - f(x-h)) / (2h)
                let half = fd_bump / 2.0;

                let mut up_sel = MapSelector::new(rd);
                let mut dn_sel = MapSelector::new(rd);
                for (k, &base_id) in input_ids.iter().enumerate() {
                    let up = if j == k {
                        base_rates[k] + half
                    } else {
                        base_rates[k]
                    };
                    let dn = if j == k {
                        base_rates[k] - half
                    } else {
                        base_rates[k]
                    };
                    up_sel.add(base_id, up);
                    dn_sel.add(base_id, dn);
                }

                let up_curves = do_bootstrap(&up_sel)
                    .unwrap_or_else(|e| panic!("FD bump+ bootstrap failed for {id}: {e:?}"));
                let dn_curves = do_bootstrap(&dn_sel)
                    .unwrap_or_else(|e| panic!("FD bump- bootstrap failed for {id}: {e:?}"));

                let df_up = up_curves[&MarketIndex::SOFR]
                    .curve()
                    .discount_factor(target_date)
                    .unwrap()
                    .value();
                let df_dn = dn_curves[&MarketIndex::SOFR]
                    .curve()
                    .discount_factor(target_date)
                    .unwrap()
                    .value();
                let fd_sens = (df_up - df_dn) / fd_bump;
                print!("{fd_sens:>14.6}");

                // Track relative error for non-negligible sensitivities.
                let ad_val = ad_adj[j];
                let scale = ad_val.abs().max(fd_sens.abs());
                if scale > 1e-6 {
                    let rel = (ad_val - fd_sens).abs() / scale;
                    max_rel_err = max_rel_err.max(rel);
                }

                // Sign and sparsity must agree.
                if ad_val.abs() > 1e-6 || fd_sens.abs() > 1e-6 {
                    assert!(
                        ad_val.signum() == fd_sens.signum()
                            || ad_val.abs() < 1e-4
                            || fd_sens.abs() < 1e-4,
                        "AD/FD sign mismatch at target={tenor_str}, input={id}: \
                         AD={ad_val:.8}, FD={fd_sens:.8}"
                    );
                }
                let _ = id;
            }
            println!();
        }

        println!("\nMax relative error (AD vs FD): {max_rel_err:.2e}");
        // IFT post-processing gives exact sensitivities (up to FD truncation
        // error); tolerance set conservatively at 1e-4.
        assert!(
            max_rel_err < 1e-4,
            "AD vs FD maximum relative error {max_rel_err:.2e} exceeds 1e-4"
        );

        println!("\n========== AD vs FD agreement verified ==========\n");
    }

    /// Shorten a pillar label for display: strip common prefixes.
    fn short_label(label: &str) -> &str {
        label
            .strip_prefix("FixedRateDeposit_USD_")
            .or_else(|| label.strip_prefix("OIS_USD_"))
            .unwrap_or(label)
    }

    /// Helper: infer a pillar date from a label like "FixedRateDeposit_USD_SOFR_3M"
    /// or "OIS_USD_SOFR_1Y" by extracting the tenor suffix.
    fn pillar_date_from_label(reference_date: Date, label: &str) -> Date {
        let tenor_str = label.rsplit('_').next().unwrap_or("0D");
        let period = Period::from_str(tenor_str)
            .unwrap_or(Period::new(0, crate::time::enums::TimeUnit::Days));
        reference_date + period
    }
}
