use std::{cell::RefCell, collections::HashMap, rc::Rc};

use nalgebra::{DMatrix, DVector};

use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        elements::curveelement::{ADCurveElement, DiscountCurveElement},
        marketdatahandling::constructedelementstore::SharedElement,
    },
    currencies::exchangeratestore::ExchangeRateStore,
    indices::marketindex::MarketIndex,
    math::{
        interpolation::interpolator::Interpolator,
        solvers::{
            solvertraits::{ContFunc, JacobianFunc, VectorFunc},
            vectornewton::VectorNewton,
        },
    },
    quotes::quote::Level,
    rates::{
        bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            bootstraputils::{dependency_order, BootstrapCurveSet, CrossCurveDep, SolvedCurve},
            calibrationinstrument::CalibrationInstrument,
            curveconfiguration::{CurveConfiguration, QuoteSelector},
        },
        yieldtermstructure::discounttermstructure::DiscountTermStructure,
    },
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

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
/// ```ignore
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
///             if q.build_instrument(self.reference_date, Level::Mid, None).is_ok() {
///                 Some(q)
///             } else {
///                 None
///             }
///         }
///         fn reference_date(&self) -> Date {
///             self.reference_date
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
/// let spec = CurveConfiguration::new(
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
    curve_specs: Vec<CurveConfiguration>,
    discount_policy: BootstrapDiscountPolicy,
    exchange_rate_store: ExchangeRateStore,
}

impl MultiCurveBootstrapper {
    /// Creates a bootstrapper from a set of curve specifications.
    #[must_use]
    pub fn new(
        curve_specs: Vec<CurveConfiguration>,
        discount_policy: BootstrapDiscountPolicy,
    ) -> Self {
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
    /// * `selector`: the quote selector to resolve market quotes for the pillar instruments. The selector should be able to build the corresponding `CalibrationInstrumentType`s for each quote ID, as these are needed for bootstrapping.
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
        let mut resolved = HashMap::new();
        for spec in &self.curve_specs {
            let mut resolved_spec = (*spec).clone();
            // For cross-currency curve specs (Collateral), pass FX spot so
            // that xccy swap notionals are FX-adjusted at inception.
            let fx_spot = if let MarketIndex::Collateral(ccy, coll_ccy) = spec.market_index() {
                self.exchange_rate_store
                    .get_exchange_rate(*coll_ccy, *ccy)
                    .ok()
                    .map(|r| r.value())
            } else {
                None
            };
            resolved_spec.resolve(selector, level, fx_spot)?;
            resolved.insert(resolved_spec.market_index().clone(), resolved_spec);
        }

        // 2. Topological sort respecting curve dependencies.
        let order = dependency_order(&resolved, &self.discount_policy)?;

        // 3. Iteratively bootstrap in dependency order.
        let mut solved_curves: HashMap<MarketIndex, SolvedCurve> = HashMap::new();
        let mut pillar_values: HashMap<MarketIndex, Vec<ADReal>> = HashMap::new();

        for index in &order {
            let spec = resolved.get(index).ok_or_else(|| {
                QSError::NotFoundErr(format!("Missing resolved spec for {index}"))
            })?;
            let calibrated = self.bootstrap_next_curve(index, spec, &solved_curves)?;
            let calibrated_pillar_values = calibrated.pillar_values()?.to_vec();
            solved_curves.insert(index.clone(), calibrated);
            pillar_values.insert(index.clone(), calibrated_pillar_values);
        }

        // 4. Convert to DiscountCurveElements.
        let mut result = HashMap::new();
        for index in &order {
            let sc = solved_curves
                .get(index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Missing solved curve for {index}")))?;
            let spec = resolved.get(index).ok_or_else(|| {
                QSError::NotFoundErr(format!("Missing resolved spec for {index}"))
            })?;
            let pv = pillar_values.get(index).ok_or_else(|| {
                QSError::NotFoundErr(format!("Missing pillar values for {index}"))
            })?;

            // Build pillar dates: reference_date followed by each instrument's pillar.
            let reference_date = spec.reference_date();
            let mut dates = vec![reference_date];
            dates.extend(spec.pillar_dates());

            // Use IFT-connected AD discount factors when available,
            // otherwise fall back to unconnected AD nodes.
            let ad_dfs: Vec<ADReal> = if let Ok(ift_dfs) = sc.output_discount_factors() {
                ift_dfs.to_vec()
            } else {
                sc.discount_factors()
                    .iter()
                    .map(|&df| ADReal::new(df))
                    .collect()
            };

            let mut ts = DiscountTermStructure::<ADReal>::new(
                dates,
                ad_dfs,
                spec.day_counter(),
                spec.interpolator(),
                spec.enable_extrapolation(),
            )?;

            ts = ts.with_pillar_values(pv.clone())?;
            let labels = sc
                .pillar_labels()
                .map(|l| l.to_vec())
                .unwrap_or_else(|| spec.pillar_labels());
            ts = ts.with_pillar_labels(labels)?;
            if let Some(ift_sens) = sc.ift_sensitivities() {
                ts = ts.with_ift_sensitivities(ift_sens.clone());
            }

            let shared: SharedElement<dyn ADCurveElement> = Rc::new(RefCell::new(ts));
            let elem = DiscountCurveElement::new(index.clone(), shared);
            result.insert(index.clone(), elem);
        }
        Ok(result)
    }

    /// Bootstraps a single curve by solving for discount factors that
    /// reprice all its instruments to zero residual. After the Newton
    /// solver converges, applies the implicit function theorem (IFT) to
    /// attach exact sensitivities w.r.t. market quotes to the result.
    fn bootstrap_next_curve(
        &self,
        target_index: &MarketIndex,
        curve_config: &CurveConfiguration,
        other_curves: &HashMap<MarketIndex, SolvedCurve>,
    ) -> Result<SolvedCurve> {
        let reference_date = curve_config.reference_date();
        let dc = curve_config.day_counter();
        let interp = curve_config.interpolator();

        // Build pillar time grid: [0, t_1, t_2, …]
        let instruments = curve_config.instruments()?;
        let mut times = vec![0.0_f64];
        times.extend(
            instruments
                .iter()
                .map(|instr| dc.year_fraction(reference_date, instr.pillar_date())),
        );

        let n = instruments.len();

        // Initial guess: slight discount (safe for positive-rate environments).
        let x0 = vec![0.99; n];

        // Build the problem.
        let problem = BootstrapProblem {
            target_index: target_index.clone(),
            reference_date,
            times: times.clone(),
            day_counter: dc,
            interpolator: interp,
            instruments,
            other_curves,
            discount_policy: &self.discount_policy,
            exchange_rate_store: &self.exchange_rate_store,
        };

        // Solve.
        let solver = VectorNewton::new(1e-12, 200);
        let solution = solver.solve(&problem, &x0)?;

        // -----------------------------------------------------------------
        // IFT post-processing
        //
        // Given the implicit relation  F(x, q, z) = 0  where x are the
        // discount factors, q the market quotes, and z the parent curve
        // discount factors, the implicit function theorem gives:
        //
        //   dx/dq = −J⁻¹ G           (own-curve sensitivity)
        //   dx/dz = −J⁻¹ (∂F/∂z)    (cross-curve sensitivity)
        //
        // where J = ∂F/∂x is the Jacobian at the solution and
        // G = ∂F/∂q is diagonal (quote q_i only enters residual F_i).
        //
        // Downstream pricers that call `backward()` on the AD tape will
        // propagate through DF → own quotes AND DF → parent DFs → parent
        // quotes correctly.
        // -----------------------------------------------------------------
        let converged_x = &solution.x;

        let mut solved_dfs = vec![1.0_f64];
        solved_dfs.extend(converged_x.iter().copied());

        // Retrieve quote values and the Jacobian J = ∂F/∂x.
        let quote_vals = curve_config.quote_values();
        let j_raw = solution
            .jacobian
            .ok_or_else(|| QSError::SolverErr("Newton solver did not return a Jacobian".into()))?;

        // Compute diagonal of G = ∂F/∂q analytically.
        let g_diag = Self::compute_quote_sensitivities(&problem, converged_x)?;

        // Build nalgebra objects and solve  J · S_col = −g_col  for each
        // quote j (only one non-zero entry per column).
        let j_data: Vec<f64> = j_raw.iter().flat_map(|row| row.iter().copied()).collect();
        let j_mat = DMatrix::from_row_slice(n, n, &j_data);
        let lu = j_mat.lu();

        // sensitivity[i][j] = ∂DF_{i+1}/∂q_j
        let mut sensitivity = vec![vec![0.0_f64; n]; n];
        for j in 0..n {
            let mut rhs = DVector::zeros(n);
            rhs[j] = g_diag[j];
            if let Some(col) = lu.solve(&rhs) {
                for i in 0..n {
                    sensitivity[i][j] = -col[i];
                }
            }
        }

        // -----------------------------------------------------------------
        // Cross-curve IFT: compute ∂DF_self/∂DF_parent for each parent.
        //
        // For each parent curve present in `other_curves`, compute the
        // matrix ∂F/∂z (how residuals depend on parent DFs) via central
        // finite differences, then solve  J · cross_col = −(∂F/∂z)_col.
        // -----------------------------------------------------------------
        let _base_residual = problem.call(converged_x)?;
        let mut cross_deps: Vec<CrossCurveDep> = Vec::new();

        for (parent_idx, parent_curve) in other_curves {
            let parent_dfs = parent_curve.discount_factors();
            let parent_n = parent_dfs.len() - 1; // excluding DF(0) = 1

            // Skip parent curves that provide no pillar IFT data
            let parent_ift = match parent_curve.ift_sensitivities() {
                Some(ift) => ift.clone(),
                None => continue,
            };
            let parent_labels: Vec<String> = parent_curve
                .pillar_labels()
                .map(|l| l.to_vec())
                .unwrap_or_else(|| {
                    parent_curve
                        .pillar_values()
                        .map(|pv| pv.iter().map(|_| String::new()).collect())
                        .unwrap_or_default()
                });

            // Compute ∂F/∂z by bumping each parent DF (indices 1..=parent_n)
            // dF_dz[row][col] = ∂F_row / ∂z_{col+1}
            let mut df_dz = vec![vec![0.0_f64; parent_n]; n];

            for m in 0..parent_n {
                let bump = (parent_dfs[m + 1].abs() * 1e-6).max(1e-10);

                let (up_res, up_bump) = self.bumped_residual(
                    &problem,
                    parent_idx,
                    parent_curve,
                    m + 1,
                    bump,
                    converged_x,
                )?;
                let (dn_res, dn_bump) = self.bumped_residual(
                    &problem,
                    parent_idx,
                    parent_curve,
                    m + 1,
                    -bump,
                    converged_x,
                )?;

                let denom = up_bump - dn_bump;
                for row in 0..n {
                    df_dz[row][m] = (up_res[row] - dn_res[row]) / denom;
                }
            }

            // Solve  J · cross_S_col = −(∂F/∂z)_col  for each parent DF
            // cross_df_sens[i][m] = ∂DF_self(i+1)/∂DF_parent(m+1)
            let mut cross_df_sens = vec![vec![0.0_f64; parent_n]; n];
            let mut has_nonzero = false;
            for m in 0..parent_n {
                let mut rhs = DVector::zeros(n);
                for row in 0..n {
                    rhs[row] = df_dz[row][m];
                }
                if let Some(col) = lu.solve(&rhs) {
                    for i in 0..n {
                        let val = -col[i];
                        if val.abs() > 1e-16 {
                            cross_df_sens[i][m] = val;
                            has_nonzero = true;
                        }
                    }
                }
            }

            if has_nonzero {
                // Retrieve parent quote values
                let parent_quote_vals: Vec<f64> = parent_curve
                    .pillar_values()
                    .map(|pv| pv.iter().map(|v| v.value()).collect())
                    .unwrap_or_default();

                cross_deps.push(CrossCurveDep {
                    cross_df_sens,
                    parent_ift_sens: parent_ift,
                    parent_quote_values: parent_quote_vals,
                    parent_pillar_labels: parent_labels,
                });
            }
        }

        // Build ADReal discount factors whose derivatives flow to the
        // quote ADReals via the computed sensitivities.
        let quote_ad: Vec<ADReal> = quote_vals.iter().map(|&v| ADReal::new(v)).collect();

        // Pre-compose cross-curve dependencies into the IFT matrix.
        // For each parent, compose:  combined[i][k] = Σ_m cross_df_sens[i][m] * parent_ift_sens[m][k]
        // Extend pillar_values and pillar_labels with parent quotes.
        let mut full_sensitivity = sensitivity.clone();
        let mut full_quotes = quote_ad.clone();
        let mut full_labels = curve_config.pillar_labels();

        for dep in &cross_deps {
            let parent_n_quotes = dep.parent_quote_values.len();
            let m_count = dep.parent_ift_sens.len();
            for i in 0..n {
                let mut row_ext = Vec::with_capacity(parent_n_quotes);
                for k in 0..parent_n_quotes {
                    let mut combined = 0.0_f64;
                    for m in 0..m_count {
                        combined += dep.cross_df_sens[i][m] * dep.parent_ift_sens[m][k];
                    }
                    row_ext.push(combined);
                }
                full_sensitivity[i].extend(row_ext);
            }
            for &v in &dep.parent_quote_values {
                full_quotes.push(ADReal::new(v));
            }
            full_labels.extend(dep.parent_pillar_labels.clone());
        }

        // Build output discount factors: DF_0 = 1 (constant), then each
        // DF_{i+1} = converged_value + Σ_j  S[i][j] * (q_j − q_j_value).
        // Since (q_j − q_j_value) = 0 in value, the numeric result is
        // exact; but the AD graph records ∂DF/∂q correctly.
        let n_total = full_quotes.len();
        let mut ad_dfs: Vec<ADReal> = Vec::with_capacity(n + 1);
        ad_dfs.push(ADReal::new(1.0)); // DF(0) = 1
        for i in 0..n {
            let mut df_ad = ADReal::new(converged_x[i]);
            for j in 0..n_total {
                let s = full_sensitivity[i][j];
                if s.abs() > 1e-16 {
                    let delta = full_quotes[j] - ADReal::new(full_quotes[j].value());
                    df_ad = (df_ad + ADReal::new(s) * delta).into();
                }
            }
            ad_dfs.push(df_ad);
        }

        Ok(SolvedCurve::new(
            target_index.clone(),
            reference_date,
            times,
            solved_dfs,
            dc,
            interp,
        )
        .with_pillar_values(full_quotes)
        .with_pillar_labels(full_labels)
        .with_output_discount_factors(ad_dfs)
        .with_ift_sensitivities(full_sensitivity))
    }

    /// Computes the diagonal of the G = ∂F/∂q matrix analytically.
    ///
    /// Each quote only enters its own residual, so only the diagonal is
    /// non-zero.  The per-instrument `quote_sensitivity` returns ∂F_j/∂q_j
    /// directly.
    fn compute_quote_sensitivities(problem: &BootstrapProblem, x: &[f64]) -> Result<Vec<f64>> {
        let trial = problem.create_trial_curve(x);
        let curves = BootstrapCurveSet::new(
            &trial,
            problem.other_curves,
            problem.discount_policy,
            problem.exchange_rate_store,
        );

        problem
            .instruments
            .iter()
            .map(|instr| instr.quote_sensitivity(&curves))
            .collect()
    }

    fn bumped_residual(
        &self,
        problem: &BootstrapProblem,
        parent_idx: &MarketIndex,
        parent_curve: &SolvedCurve,
        parent_df_idx: usize,
        bump: f64,
        x: &[f64],
    ) -> Result<(Vec<f64>, f64)> {
        let original_df = parent_curve.discount_factors()[parent_df_idx];
        let bumped_df = (original_df + bump).max(1e-10);

        let mut bumped_parent = parent_curve.clone();
        bumped_parent.discount_factors_mut()[parent_df_idx] = bumped_df;

        let mut bumped_others = problem.other_curves.clone();
        bumped_others.insert(parent_idx.clone(), bumped_parent);

        let bumped_problem = BootstrapProblem {
            target_index: problem.target_index.clone(),
            reference_date: problem.reference_date,
            times: problem.times.clone(),
            day_counter: problem.day_counter,
            interpolator: problem.interpolator,
            instruments: problem.instruments,
            other_curves: &bumped_others,
            discount_policy: problem.discount_policy,
            exchange_rate_store: problem.exchange_rate_store,
        };

        Ok((bumped_problem.call(x)?, bumped_df - original_df))
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
    pub target_index: MarketIndex,
    pub reference_date: Date,
    pub times: Vec<f64>,
    pub day_counter: DayCounter,
    pub interpolator: Interpolator,
    pub instruments: &'a [CalibrationInstrument],
    pub other_curves: &'a HashMap<MarketIndex, SolvedCurve>,
    pub discount_policy: &'a BootstrapDiscountPolicy,
    pub exchange_rate_store: &'a ExchangeRateStore,
}

impl BootstrapProblem<'_> {
    fn create_trial_curve(&self, x: &[f64]) -> SolvedCurve {
        let mut dfs = Vec::with_capacity(self.times.len());
        dfs.push(1.0_f64); // DF(0) = 1
        dfs.extend_from_slice(x);
        SolvedCurve::new(
            self.target_index.clone(),
            self.reference_date,
            self.times.clone(),
            dfs,
            self.day_counter,
            self.interpolator,
        )
    }
}

impl ContFunc<[f64], Vec<f64>> for BootstrapProblem<'_> {
    fn call(&self, x: &[f64]) -> Result<Vec<f64>> {
        let trial = self.create_trial_curve(x);
        let curves = BootstrapCurveSet::new(
            &trial,
            self.other_curves,
            self.discount_policy,
            self.exchange_rate_store,
        );
        self.instruments
            .iter()
            .map(|instr| instr.residual(&curves))
            .collect()
    }
}

impl JacobianFunc<f64, f64, f64> for BootstrapProblem<'_> {
    fn jacobian(&self, x: &[f64]) -> Result<Vec<Vec<f64>>> {
        let n = x.len();
        let mut jacobian = vec![vec![0.0; n]; n];

        for col in 0..n {
            let base_bump = (x[col].abs().max(1.0) * 1e-6).max(1e-8);
            let bump = base_bump.min((x[col] * 0.25).max(1e-8));

            let mut up = x.to_vec();
            let mut dn = x.to_vec();
            up[col] += bump;
            dn[col] = (dn[col] - bump).max(1e-8);

            let up_res = self.call(&up)?;
            let dn_res = self.call(&dn)?;
            let denom = up[col] - dn[col];

            for row in 0..n {
                jacobian[row][col] = (up_res[row] - dn_res[row]) / denom;
            }
        }

        Ok(jacobian)
    }
}

impl VectorFunc<f64, f64> for BootstrapProblem<'_> {}
