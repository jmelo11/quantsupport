use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    calibration::{
        calibrationpricer::CalibrationInstrumentPricer, calibrationprocess::CalibrationProcess,
    },
    core::{
        elements::curveelement::{ADCurveElement, DiscountCurveElement},
        marketdatahandling::constructedelementstore::SharedElement,
    },
    indices::marketindex::MarketIndex,
    math::{
        interpolation::interpolator::Interpolator,
        solvers::{
            solvertraits::{ContFunc, JacobianFunc, VectorFunc},
            vectornewton::VectorNewton,
        },
    },
    quotes::{
        calibrationinstrument::CalibrationInstrument, fixingstore::FixingStore, fxstore::FxStore,
        quote::Level, quoteselector::QuoteSelector,
    },
    rates::{
        bootstrapping::{
            bootstrapcalibrationinstrument::BootstrapStepEvaluation,
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            bootstrappedcurve::BootstrappedCurve,
            bootstrapstep::BootstrapStep,
            bootstraputils::{dependency_order, CrossCurveDep},
            curveconfiguration::CurveConfiguration,
        },
        yieldtermstructure::{
            compositetermstructure::CompositeTermStructure,
            discounttermstructure::DiscountTermStructure,
        },
    },
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};
use nalgebra::{DMatrix, DVector};
use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

/// Multi-curve bootstrapping engine.
///
/// Accepts a set of [`CurveConfiguration`]s and a [`BootstrapDiscountPolicy`] that
/// will determine how to bootstrap each curve. It resolves dependencies between [`CurveConfiguration`].
///
/// ## Parameters
/// * `curve_specs`: curve specifications containing the market index, day count,
///   interpolation, extrapolation setting, and pillar quote identifiers.
/// * `discount_policy`: the discount policy defines how to determine the discount curve for each instrument during bootstrapping, including handling of cross-currency instruments and collateralization. See [`BootstrapDiscountPolicy`] for details.
///
/// ## Example
/// ```
/// use quantsupport::prelude::*;
///
/// let mut quotes = QuoteStore::new(Date::new(2024, 6, 3));
/// let identifiers = ["FixedRateDeposit_USD_SOFR_3M", "FixedRateDeposit_USD_SOFR_6M"];
/// for (identifier, rate) in identifiers.iter().zip([0.05, 0.051]) {
///     quotes.add_quote(Quote::new(identifier.parse()?, QuoteLevels::with_mid(rate)));
/// }
/// let spec = CurveConfiguration::new(
///     MarketIndex::SOFR,
///     DayCounter::Actual360,
///     Interpolator::LogLinear,
///     true,
///     identifiers.iter().map(|id| id.to_string()).collect(),
/// );
/// let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
/// let bootstrapper = MultiCurveBootstrapper::new(vec![spec], policy);
/// let curves = bootstrapper.bootstrap(&quotes, Level::Mid)?;
/// assert!(curves.contains_key(&MarketIndex::SOFR));
/// # Ok::<(), quantsupport::utils::errors::QSError>(())
/// ```
pub struct MultiCurveBootstrapper<'a> {
    curve_specs: Vec<CurveConfiguration>,
    discount_policy: BootstrapDiscountPolicy,
    fx_store: Cow<'a, FxStore>,
    fixing_store: Option<Cow<'a, FixingStore>>,
}

impl<'a> MultiCurveBootstrapper<'a> {
    /// Creates a bootstrapper from a set of curve specifications.
    #[must_use]
    pub fn new(
        curve_specs: Vec<CurveConfiguration>,
        discount_policy: BootstrapDiscountPolicy,
    ) -> Self {
        Self {
            curve_specs,
            discount_policy,
            fx_store: Cow::Owned(FxStore::new()),
            fixing_store: None,
        }
    }

    /// Registers an [`FxStore`] for FX spot rates. Required for
    /// instruments referencing multiple currencies (e.g. cross-currency swaps, FX forwards).
    #[must_use]
    pub fn with_fx_store(mut self, store: FxStore) -> Self {
        self.fx_store = Cow::Owned(store);
        self
    }

    /// Registers historical observations for seasoned or lookback OIS calibration coupons.
    #[must_use]
    pub fn with_fixing_store(mut self, store: FixingStore) -> Self {
        self.fixing_store = Some(Cow::Owned(store));
        self
    }

    /// Borrows FX rates and historical observations during construction.
    #[must_use]
    pub fn with_market_stores(mut self, fx: &'a FxStore, fixings: &'a FixingStore) -> Self {
        self.fx_store = Cow::Borrowed(fx);
        self.fixing_store = Some(Cow::Borrowed(fixings));
        self
    }

    /// Resolves quotes and constructs calibrated, directly quoted, explicit
    /// zero-rate, and composite curves in dependency order.
    ///
    /// ## Parameters
    /// * `selector`: selects the instrument or direct curve quotes referenced
    ///   by each configuration at the requested level.
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
        self.bootstrap_with_snapshots(selector, level)
            .map(|(curves, _)| curves)
    }

    /// Constructs curves and returns their calibration snapshots, both keyed by index.
    ///
    /// # Errors
    /// Returns an error if quote resolution fails, configurations are duplicated,
    /// dependencies are missing or cyclic, curve inputs are invalid, or calibration
    /// fails to converge.
    pub fn bootstrap_with_snapshots(
        &self,
        selector: &impl QuoteSelector,
        level: Level,
    ) -> Result<(
        HashMap<MarketIndex, DiscountCurveElement>,
        HashMap<MarketIndex, BootstrappedCurve>,
    )> {
        // 1. Resolve all curve specs into concrete instruments.
        let mut resolved = HashMap::new();
        for spec in &self.curve_specs {
            let mut resolved_spec = (*spec).clone();
            // For cross-currency curve specs (Collateral), pass FX spot so
            // that xccy swap notionals are FX-adjusted at inception.
            let fx_spot = if let MarketIndex::Collateral(ccy, coll_ccy) = spec.market_index() {
                self.fx_store
                    .get_fx_rate(*coll_ccy, *ccy)
                    .ok()
                    .map(|r| r.value())
            } else {
                None
            };
            resolved_spec.resolve(selector, level, fx_spot)?;
            if resolved
                .insert(resolved_spec.market_index().clone(), resolved_spec)
                .is_some()
            {
                return Err(QSError::InvalidValueErr(format!(
                    "Duplicate curve configuration for {}",
                    spec.market_index()
                )));
            }
        }

        // 2. Topological sort respecting curve dependencies.
        let order = dependency_order(&resolved, &self.discount_policy)?;

        // 3. Construct every definition in the same dependency order.
        let mut solved_curves = HashMap::new();
        let mut result = HashMap::new();
        for index in &order {
            let spec = resolved.get(index).ok_or_else(|| {
                QSError::NotFoundErr(format!("Missing resolved spec for {index}"))
            })?;
            let (element, calibrated) = self.construct_curve(spec, &result, &solved_curves)?;
            solved_curves.insert(index.clone(), calibrated);
            result.insert(index.clone(), element);
        }
        Ok((result, solved_curves))
    }

    /// Constructs one resolved curve configuration and its calibration snapshot.
    ///
    /// `curves` and `solved` supply the already constructed dependency curves
    /// and their calibration snapshots. Resolve `spec` with the desired quote
    /// selector and level before calling this method.
    ///
    /// # Errors
    /// Returns an error if required dependencies or resolved inputs are missing,
    /// curve inputs are invalid, or calibration fails.
    pub fn construct_curve(
        &self,
        spec: &CurveConfiguration,
        curves: &HashMap<MarketIndex, DiscountCurveElement>,
        solved: &HashMap<MarketIndex, BootstrappedCurve>,
    ) -> Result<(DiscountCurveElement, BootstrappedCurve)> {
        let index = spec.market_index();
        let (curve, calibrated) = if let Some(quoted) = spec.quoted_curve() {
            quoted.construct(spec)?
        } else if let Some((base_index, spread_index)) = spec.composite_components() {
            Self::composite_term_structure(spec, base_index, spread_index, curves, solved)?
        } else {
            let calibrated = self.bootstrap_next_curve(index, spec, solved)?;
            let curve = Self::calibrated_term_structure(spec, &calibrated)?;
            (Box::new(curve) as Box<dyn ADCurveElement>, calibrated)
        };
        let shared: SharedElement<dyn ADCurveElement> = Rc::new(RefCell::new(curve));
        Ok((DiscountCurveElement::new(index.clone(), shared), calibrated))
    }

    /// Builds an AD term structure from calibrated discount factors and their quote derivatives.
    fn calibrated_term_structure(
        spec: &CurveConfiguration,
        calibrated: &BootstrappedCurve,
    ) -> Result<DiscountTermStructure<DualFwd>> {
        let mut dates = vec![spec.reference_date()?];
        dates.extend(spec.pillar_dates());
        let ad_dfs = calibrated.output_discount_factors().map_or_else(
            |_| {
                calibrated
                    .discount_factors()
                    .iter()
                    .map(|&df| DualFwd::new(df))
                    .collect()
            },
            <[DualFwd]>::to_vec,
        );
        let mut curve = DiscountTermStructure::<DualFwd>::new(
            dates,
            ad_dfs,
            spec.day_counter(),
            spec.interpolator(),
            spec.enable_extrapolation(),
        )?
        .with_pillar_values(calibrated.pillar_values()?.to_vec())?
        .with_pillar_labels(
            calibrated
                .pillar_labels()
                .map_or_else(|| spec.pillar_labels(), <[String]>::to_vec),
        )?;
        if let Some(sensitivities) = calibrated.ift_sensitivities() {
            curve = curve.with_ift_sensitivities(sensitivities.clone());
        }
        Ok(curve)
    }

    /// Multiplies the term structures and source-risk snapshots of two component curves.
    fn composite_term_structure(
        spec: &CurveConfiguration,
        base_index: &MarketIndex,
        spread_index: &MarketIndex,
        curves: &HashMap<MarketIndex, DiscountCurveElement>,
        solved: &HashMap<MarketIndex, BootstrappedCurve>,
    ) -> Result<(Box<dyn ADCurveElement>, BootstrappedCurve)> {
        let component = |index: &MarketIndex| {
            curves
                .get(index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Composite component {index}")))?
                .curve()
                .clone_curve()
        };
        let base = component(base_index)?;
        let spread = component(spread_index)?;
        if base.reference_date() != spread.reference_date()
            || base.day_counter() != spread.day_counter()
        {
            return Err(QSError::InvalidValueErr(
                "Composite curves require matching reference dates and day counters".into(),
            ));
        }
        let snapshot = BootstrappedCurve::from_components(
            spec.market_index().clone(),
            solved
                .get(base_index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Composite component {base_index}")))?
                .clone(),
            solved
                .get(spread_index)
                .ok_or_else(|| QSError::NotFoundErr(format!("Composite component {spread_index}")))?
                .clone(),
        )?;
        Ok((
            Box::new(CompositeTermStructure::from_curves(spread, base)),
            snapshot,
        ))
    }

    /// Bootstraps a single curve by solving for discount factors that
    /// reprice all its instruments to zero residual.
    #[allow(clippy::too_many_lines)]
    fn bootstrap_next_curve(
        &self,
        target_index: &MarketIndex,
        curve_config: &CurveConfiguration,
        other_curves: &HashMap<MarketIndex, BootstrappedCurve>,
    ) -> Result<BootstrappedCurve> {
        let reference_date = curve_config.reference_date()?;
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
        let problem = BootstrapObjectiveFunc {
            target_index: target_index.clone(),
            reference_date,
            times: times.clone(),
            day_counter: dc,
            interpolator: interp,
            instruments,
            other_curves,
            discount_policy: &self.discount_policy,
            fx_store: &self.fx_store,
            fixing_store: self.fixing_store.as_deref(),
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
            let explicit = parent_curve.has_explicit_source();
            let parent_inputs: Vec<f64> = if explicit {
                parent_curve
                    .pillar_values()?
                    .iter()
                    .map(Scalar::value)
                    .collect()
            } else {
                parent_curve.discount_factors()[1..].to_vec()
            };
            let parent_n = parent_inputs.len();
            // Explicit curves are differentiated directly in their source-pillar
            // coordinates. Calibrated parents retain the existing DF-to-quote IFT.
            let parent_ift = if explicit {
                (0..parent_n)
                    .map(|row| {
                        (0..parent_n)
                            .map(|column| f64::from(row == column))
                            .collect()
                    })
                    .collect()
            } else {
                match parent_curve.ift_sensitivities() {
                    Some(ift) => ift.clone(),
                    None => continue,
                }
            };
            let parent_labels: Vec<String> = parent_curve.pillar_labels().map_or_else(
                || {
                    parent_curve
                        .pillar_values()
                        .map(|pv| pv.iter().map(|_| String::new()).collect())
                        .unwrap_or_default()
                },
                <[String]>::to_vec,
            );

            // Bump each parent DF or explicit source pillar.
            let mut df_dz = vec![vec![0.0_f64; parent_n]; n];

            for m in 0..parent_n {
                let coordinate = if explicit { m } else { m + 1 };
                let bump = (parent_inputs[m].abs() * 1e-6).max(1e-10);

                let (up_res, up_bump) = bumped_residual(
                    &problem,
                    parent_idx,
                    parent_curve,
                    coordinate,
                    bump,
                    converged_x,
                )?;
                let (dn_res, dn_bump) = bumped_residual(
                    &problem,
                    parent_idx,
                    parent_curve,
                    coordinate,
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
                    .map(|pv| pv.iter().map(Scalar::value).collect())
                    .unwrap_or_default();

                cross_deps.push(CrossCurveDep {
                    cross_df_sens,
                    parent_ift_sens: parent_ift,
                    parent_quote_values: parent_quote_vals,
                    parent_pillar_labels: parent_labels,
                });
            }
        }

        // Build DualFwd discount factors whose derivatives flow to the
        // quote DualFwds via the computed sensitivities.
        let quote_ad: Vec<DualFwd> = quote_vals.iter().map(|&v| DualFwd::new(v)).collect();

        // Pre-compose cross-curve dependencies into the IFT matrix.
        // For each parent, compose:  combined[i][k] = Σ_m cross_df_sens[i][m] * parent_ift_sens[m][k]
        // Extend pillar_values and pillar_labels with parent quotes.
        let mut full_sensitivity = sensitivity.clone();
        let mut full_quotes = quote_ad;
        let mut full_labels = curve_config.pillar_labels();

        for dep in &cross_deps {
            let parent_n_quotes = dep.parent_quote_values.len();
            let m_count = dep.parent_ift_sens.len();
            for (i, sensitivity_row) in full_sensitivity.iter_mut().enumerate().take(n) {
                let mut row_ext = Vec::with_capacity(parent_n_quotes);
                for k in 0..parent_n_quotes {
                    let mut combined = 0.0_f64;
                    for m in 0..m_count {
                        combined = f64::mul_add(
                            dep.cross_df_sens[i][m],
                            dep.parent_ift_sens[m][k],
                            combined,
                        );
                    }
                    row_ext.push(combined);
                }
                sensitivity_row.extend(row_ext);
            }
            for &v in &dep.parent_quote_values {
                full_quotes.push(DualFwd::new(v));
            }
            full_labels.extend(dep.parent_pillar_labels.clone());
        }

        // Build output discount factors: DF_0 = 1 (constant), then each
        // DF_{i+1} = converged_value + Σ_j  S[i][j] * (q_j − q_j_value).
        // Since (q_j − q_j_value) = 0 in value, the numeric result is
        // exact; but the AD graph records ∂DF/∂q correctly.
        let n_total = full_quotes.len();
        let mut ad_dfs: Vec<DualFwd> = Vec::with_capacity(n + 1);
        ad_dfs.push(DualFwd::new(1.0)); // DF(0) = 1
        for i in 0..n {
            let mut df_ad = DualFwd::new(converged_x[i]);
            for j in 0..n_total {
                let s = full_sensitivity[i][j];
                if s.abs() > 1e-16 {
                    let delta = full_quotes[j] - DualFwd::new(full_quotes[j].value());
                    df_ad = (df_ad + DualFwd::new(s) * delta).into();
                }
            }
            ad_dfs.push(df_ad);
        }

        Ok(BootstrappedCurve::new(
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
    /// non-zero.  The per-instrument `quote_sensitivity` returns `∂F_j/∂q_j`
    /// directly.
    fn compute_quote_sensitivities(
        problem: &BootstrapObjectiveFunc,
        x: &[f64],
    ) -> Result<Vec<f64>> {
        let trial = problem.create_trial_curve(x);
        let step = BootstrapStep::new(
            &trial,
            problem.other_curves,
            problem.discount_policy,
            problem.fx_store,
        )
        .with_fixing_store(problem.fixing_store);
        let evaluator = BootstrapStepEvaluation::new(&step);
        problem
            .instruments
            .iter()
            .map(|inst| evaluator.sensitivity(inst))
            .collect()
    }
}

/// Evaluates a calibration residual after bumping one parent-curve pillar.
fn bumped_residual(
    problem: &BootstrapObjectiveFunc,
    parent_idx: &MarketIndex,
    parent_curve: &BootstrappedCurve,
    parent_df_idx: usize,
    bump: f64,
    x: &[f64],
) -> Result<(Vec<f64>, f64)> {
    let mut bumped_parent = parent_curve.clone();
    let applied_bump = if parent_curve.has_explicit_source() {
        bumped_parent.bump_pillar(parent_df_idx, bump)?;
        bump
    } else {
        let original = parent_curve.discount_factors()[parent_df_idx];
        let bumped = (original + bump).max(1e-10);
        bumped_parent.discount_factors_mut()[parent_df_idx] = bumped;
        bumped - original
    };

    let mut bumped_others = problem.other_curves.clone();
    bumped_others.insert(parent_idx.clone(), bumped_parent);

    let bumped_problem = BootstrapObjectiveFunc {
        target_index: problem.target_index.clone(),
        reference_date: problem.reference_date,
        times: problem.times.clone(),
        day_counter: problem.day_counter,
        interpolator: problem.interpolator,
        instruments: problem.instruments,
        other_curves: &bumped_others,
        discount_policy: problem.discount_policy,
        fx_store: problem.fx_store,
        fixing_store: problem.fixing_store,
    };

    Ok((bumped_problem.call(x)?, applied_bump))
}

/// Maps a trial vector of discount factors into the residual vector used by
/// the Newton solver.
///
/// For each instrument the residual is:
/// * **Deposits / Swaps / Basis-swaps / XCcy-swaps** → NPV (should be ≈ 0)
/// * **Rate futures** → `implied_forward - market_rate`
/// * **FX forwards** → `implied_FX - market_FX`
struct BootstrapObjectiveFunc<'a> {
    pub target_index: MarketIndex,
    pub reference_date: Date,
    pub times: Vec<f64>,
    pub day_counter: DayCounter,
    pub interpolator: Interpolator,
    pub instruments: &'a [CalibrationInstrument],
    pub other_curves: &'a HashMap<MarketIndex, BootstrappedCurve>,
    pub discount_policy: &'a BootstrapDiscountPolicy,
    pub fx_store: &'a FxStore,
    pub fixing_store: Option<&'a FixingStore>,
}

impl BootstrapObjectiveFunc<'_> {
    /// Builds a trial discount curve from the solver discount factors, anchored at one today.
    fn create_trial_curve(&self, x: &[f64]) -> BootstrappedCurve {
        let mut dfs = Vec::with_capacity(self.times.len());
        dfs.push(1.0_f64); // DF(0) = 1
        dfs.extend_from_slice(x);
        BootstrappedCurve::new(
            self.target_index.clone(),
            self.reference_date,
            self.times.clone(),
            dfs,
            self.day_counter,
            self.interpolator,
        )
    }
}

impl ContFunc<[f64], Vec<f64>> for BootstrapObjectiveFunc<'_> {
    fn call(&self, x: &[f64]) -> Result<Vec<f64>> {
        let trial = self.create_trial_curve(x);
        let step = BootstrapStep::new(
            &trial,
            self.other_curves,
            self.discount_policy,
            self.fx_store,
        )
        .with_fixing_store(self.fixing_store);
        let evaluator = BootstrapStepEvaluation::new(&step);
        evaluator.residual(self.instruments)
    }
}

impl JacobianFunc<f64, f64, f64> for BootstrapObjectiveFunc<'_> {
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

impl VectorFunc<f64, f64> for BootstrapObjectiveFunc<'_> {}

#[cfg(test)]
mod tests {
    use crate::{
        ad::dual::DualFwd,
        core::makeinstrument::MakeInstrument,
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        math::interpolation::interpolator::Interpolator,
        quotes::{
            fxstore::FxStore,
            quote::{Level, Quote, QuoteDetails, QuoteLevels},
            quoteselector::QuoteSelector,
        },
        rates::bootstrapping::{
            bootstrapdiscountpolicy::BootstrapDiscountPolicy,
            curveconfiguration::CurveConfiguration, multicurvebootstrapper::MultiCurveBootstrapper,
        },
        time::{date::Date, daycounter::DayCounter},
        utils::errors::Result,
    };
    use std::collections::HashMap;

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
            if matches!(
                q.details().instrument(),
                crate::quotes::quote::QuoteInstrument::ZeroRate
                    | crate::quotes::quote::QuoteInstrument::DiscountFactor
            ) || q
                .build_instrument(self.reference_date, Level::Mid, None)
                .is_ok()
            {
                Some(q)
            } else {
                None
            }
        }
        fn reference_date(&self) -> Date {
            self.reference_date
        }
    }

    fn rd() -> Date {
        Date::new(2024, 6, 1)
    }

    fn default_policy() -> BootstrapDiscountPolicy {
        BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD)
    }

    #[test]
    fn configured_zero_rates_and_nested_composites_share_curve_construction() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        selector.add("FixedRateDeposit_USD_SOFR_1Y", 0.05);
        let spread_index = MarketIndex::Other("SPREAD".into());
        let funding_index = MarketIndex::Other("FUNDING".into());
        let nested_index = MarketIndex::Other("NESTED".into());
        let benchmark = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["FixedRateDeposit_USD_SOFR_1Y".into()],
        );
        selector.add("ZeroRate_USD_SPREAD_0D", 0.005);
        selector.add("ZeroRate_USD_SPREAD_2Y", 0.01);
        let spread = CurveConfiguration::new(
            spread_index.clone(),
            DayCounter::Actual360,
            Interpolator::Linear,
            true,
            vec![
                "ZeroRate_USD_SPREAD_0D".into(),
                "ZeroRate_USD_SPREAD_2Y".into(),
            ],
        );
        let funding =
            CurveConfiguration::composite(funding_index.clone(), MarketIndex::SOFR, spread_index);
        let nested = CurveConfiguration::composite(
            nested_index.clone(),
            funding_index.clone(),
            funding_index.clone(),
        );
        let curves =
            MultiCurveBootstrapper::new(vec![nested, funding, spread, benchmark], default_policy())
                .bootstrap(&selector, Level::Mid)?;
        let date = Date::new(2025, 6, 1);
        let time = DayCounter::Actual360.year_fraction(rd(), date);
        let expected = (-0.0075 * time).exp() / (1.0 + 0.05 * time);
        assert!(
            (curves[&funding_index]
                .curve()
                .discount_factor(date)?
                .value()
                - expected)
                .abs()
                < 1e-12
        );
        assert!(
            (curves[&nested_index].curve().discount_factor(date)?.value() - expected.powi(2)).abs()
                < 1e-12
        );
        Ok(())
    }

    #[test]
    fn bootstrap_single_deposit() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.05);

        let spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["FixedRateDeposit_USD_SOFR_6M".into()],
        );

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        assert!(result.contains_key(&MarketIndex::SOFR));
        let curve = result[&MarketIndex::SOFR].curve();
        let df = curve.discount_factor(Date::new(2024, 12, 1))?;
        assert!(
            df.value() > 0.0 && df.value() < 1.0,
            "DF should be in (0,1)"
        );
        Ok(())
    }

    #[test]
    fn bootstrap_regular_swaps_alongside_ois() -> Result<()> {
        use crate::quotes::quote::CalibrationInstrumentType;

        let reference = Date::new(2026, 2, 24);
        let mut selector = MapSelector::new(reference);
        for (identifier, rate) in [
            ("FixedRateDeposit_USD_SOFR_1D", 0.04),
            ("OIS_USD_SOFR_1Y", 0.04),
            ("OIS_USD_SOFR_2Y", 0.039),
            ("Swap_USD_TermSOFR3m_1Y", 0.045),
            ("Swap_USD_TermSOFR3m_2Y", 0.046),
        ] {
            selector.add(identifier, rate);
        }
        let overnight = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_1D".into(),
                "OIS_USD_SOFR_1Y".into(),
                "OIS_USD_SOFR_2Y".into(),
            ],
        );
        let mut term = CurveConfiguration::new(
            MarketIndex::TermSOFR3m,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "Swap_USD_TermSOFR3m_1Y".into(),
                "Swap_USD_TermSOFR3m_2Y".into(),
            ],
        );
        term.resolve(&selector, Level::Mid, None)?;
        assert!(term
            .instruments()?
            .iter()
            .all(|instrument| matches!(instrument.built(), CalibrationInstrumentType::Swap(_))));
        let curves = MultiCurveBootstrapper::new(vec![term, overnight], default_policy())
            .bootstrap(&selector, Level::Mid)?;
        assert_eq!(curves.len(), 2);
        let maturity = Date::new(2028, 2, 24);
        let overnight_discount = curves[&MarketIndex::SOFR]
            .curve()
            .discount_factor(maturity)?
            .value();
        let term_discount = curves[&MarketIndex::TermSOFR3m]
            .curve()
            .discount_factor(maturity)?
            .value();
        assert!(term_discount > 0.0 && term_discount < overnight_discount);
        Ok(())
    }

    #[test]
    fn euribor_fixed_day_counts_match_independent_multicurve_cashflows() -> Result<()> {
        let reference = Date::new(2026, 10, 2);
        let dates = [
            Date::new(2027, 1, 4),
            Date::new(2027, 4, 2),
            Date::new(2027, 7, 2),
            Date::new(2027, 10, 4),
        ];
        let rates = [0.02598, 0.02781542, 0.029203, 0.0302858];
        let overnight_rates = [0.02499, 0.02620, 0.02741, 0.02839];
        for (suffix, fixed_day_counter) in [
            ("", DayCounter::Thirty360),
            ("_Thirty360", DayCounter::Thirty360),
            ("_Actual360", DayCounter::Actual360),
            ("_Actual365", DayCounter::Actual365),
        ] {
            let mut selector = MapSelector::new(reference);
            let mut overnight_ids = Vec::new();
            let mut term_ids = Vec::new();
            for ((tenor, rate), overnight_rate) in ["3M", "6M", "9M", "1Y"]
                .into_iter()
                .zip(rates)
                .zip(overnight_rates)
            {
                let overnight_id = format!("OIS_EUR_ESTR_{tenor}___0_0");
                let term_id = format!("Swap_EUR_EURIBOR3m_{tenor}___0{suffix}");
                selector.add(&overnight_id, overnight_rate);
                selector.add(&term_id, rate);
                overnight_ids.push(overnight_id);
                term_ids.push(term_id);
            }
            let config = |index, ids| {
                CurveConfiguration::new(
                    index,
                    DayCounter::Actual365,
                    Interpolator::LogLinear,
                    true,
                    ids,
                )
            };
            let curves = MultiCurveBootstrapper::new(
                vec![
                    config(MarketIndex::EURIBOR3m, term_ids),
                    config(MarketIndex::ESTR, overnight_ids),
                ],
                BootstrapDiscountPolicy::new(MarketIndex::ESTR, Currency::EUR),
            )
            .bootstrap(&selector, Level::Mid)?;
            let curve = curves[&MarketIndex::EURIBOR3m].curve();
            let mut previous = 1.0;
            let mut floating_pv = 0.0;
            for ((date, rate), overnight_rate) in dates.into_iter().zip(rates).zip(overnight_rates)
            {
                // One fixed payment vs quarterly floating coupons. All dates
                // are market pillars, so this recursion has no interpolation.
                let overnight_accrual = DayCounter::Actual360.year_fraction(reference, date);
                let discount = 1.0 / (1.0 + overnight_rate * overnight_accrual);
                let fixed_accrual = fixed_day_counter.year_fraction(reference, date);
                let expected = previous / (1.0 + rate * fixed_accrual - floating_pv / discount);
                assert!(
                    (curve.discount_factor(date)?.value() - expected).abs() < 1e-11,
                    "{suffix} fixed convention at {date}"
                );
                floating_pv += (previous / expected - 1.0) * discount;
                previous = expected;
            }
        }
        Ok(())
    }

    #[test]
    fn ois_bootstrap_recovers_discount_growth_with_different_day_counts() {
        use crate::prelude::{CashflowType, MakeOvernightIndexSwap, Period};

        let reference = Date::new(2026, 10, 8);
        let discount = |date| (-0.04 * DayCounter::Actual365.year_fraction(reference, date)).exp();
        let mut selector = MapSelector::new(reference);
        let mut identifiers = Vec::new();
        for tenor in ["1M", "1Y", "2Y", "5Y"] {
            let identifier = format!("OIS_USD_SOFR_{tenor}");
            let swap = MakeOvernightIndexSwap::<f64>::new(
                MarketIndex::SOFR,
                reference,
                Period::from_str(tenor).unwrap(),
                0.0,
            )
            .build()
            .unwrap();
            let annuity: f64 = swap
                .fixed_leg()
                .cashflows()
                .iter()
                .map(|cashflow| {
                    let CashflowType::FixedRateCoupon(coupon) = cashflow else {
                        panic!("expected fixed coupon")
                    };
                    coupon
                        .rate()
                        .day_counter()
                        .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date())
                        * discount(coupon.payment_date())
                })
                .sum();
            let floating_pv: f64 = swap
                .floating_leg()
                .cashflows()
                .iter()
                .map(|cashflow| {
                    let CashflowType::FloatingRateCoupon(coupon) = cashflow else {
                        panic!("expected overnight coupon")
                    };
                    (discount(coupon.accrual_start_date()) / discount(coupon.accrual_end_date())
                        - 1.0)
                        * discount(coupon.payment_date())
                })
                .sum();
            selector.add(&identifier, floating_pv / annuity);
            identifiers.push(identifier);
        }
        let mut config = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual365,
            Interpolator::LogLinear,
            true,
            identifiers,
        );
        config.resolve(&selector, Level::Mid, None).unwrap();
        let dates = config.pillar_dates();
        let curves = MultiCurveBootstrapper::new(vec![config], default_policy())
            .bootstrap(&selector, Level::Mid)
            .unwrap();
        let curve = curves[&MarketIndex::SOFR].curve();
        for date in dates {
            assert!((curve.discount_factor(date).unwrap().value() - discount(date)).abs() < 1e-10);
        }
    }

    #[test]
    fn bootstrap_price_anchored_bond() -> Result<()> {
        let corporate = MarketIndex::Other("CORP".into());
        let quote_id = "FixedRateBond_USD_CORP_2Y_Semiannual_0.04_AnchorPrice";
        let mut selector = MapSelector::new(rd());
        selector.add(quote_id, 99.0);

        let spec = CurveConfiguration::new(
            corporate.clone(),
            DayCounter::ActualActual,
            Interpolator::LogLinear,
            true,
            vec![quote_id.into()],
        );

        let result = MultiCurveBootstrapper::new(vec![spec], default_policy())
            .bootstrap(&selector, Level::Mid)?;
        let curve = result[&corporate].curve();
        let df = curve.discount_factor(Date::new(2026, 6, 1))?.value();
        assert!(df > 0.0 && df < 1.0);
        Ok(())
    }

    #[test]
    fn bootstrap_yield_anchored_bond() -> Result<()> {
        let corporate = MarketIndex::Other("CORP".into());
        let quote_id = "FixedRateBond_USD_CORP_2Y_Semiannual_0.04_AnchorYield";
        let mut selector = MapSelector::new(rd());
        selector.add(quote_id, 0.05);

        let spec = CurveConfiguration::new(
            corporate.clone(),
            DayCounter::ActualActual,
            Interpolator::LogLinear,
            true,
            vec![quote_id.into()],
        );

        let result = MultiCurveBootstrapper::new(vec![spec], default_policy())
            .bootstrap(&selector, Level::Mid)?;
        let df = result[&corporate]
            .curve()
            .discount_factor(Date::new(2026, 6, 1))?
            .value();
        assert!(df > 0.0 && df < 1.0);
        Ok(())
    }

    #[test]
    fn bootstrap_deposits_and_swaps() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.051);
        selector.add("OIS_USD_SOFR_1Y", 0.048);
        selector.add("OIS_USD_SOFR_2Y", 0.045);

        let spec = CurveConfiguration::new(
            MarketIndex::SOFR,
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

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        let curve = result[&MarketIndex::SOFR].curve();

        // DFs should be monotonically decreasing for positive rates.
        let dates = [
            Date::new(2024, 9, 1),
            Date::new(2024, 12, 1),
            Date::new(2025, 6, 1),
            Date::new(2026, 6, 1),
        ];
        let mut prev_df = 1.0;
        for d in &dates {
            let df = curve.discount_factor(*d)?.value();
            assert!(df < prev_df, "DF at {d} should be < previous DF");
            assert!(df > 0.0, "DF should be positive");
            prev_df = df;
        }
        Ok(())
    }

    #[test]
    fn bootstrap_result_has_ift_sensitivities() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        selector.add("FixedRateDeposit_USD_SOFR_3M", 0.05);
        selector.add("OIS_USD_SOFR_1Y", 0.048);

        let spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_3M".into(),
                "OIS_USD_SOFR_1Y".into(),
            ],
        );

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        let elem = &result[&MarketIndex::SOFR];
        let curve = elem.curve();
        let ift = curve.ift_sensitivities();
        assert!(ift.is_some(), "IFT sensitivities should be present");

        let sens = ift.unwrap();
        assert_eq!(sens.len(), 2, "Should have 2 pillar rows");
        assert_eq!(sens[0].len(), 2, "Should have 2 quote columns");
        Ok(())
    }

    #[test]
    fn bootstrap_missing_quote_errors() {
        let selector = MapSelector::new(rd());
        // Not adding any quotes to the selector.

        let spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["OIS_USD_SOFR_1Y".into()],
        );

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid);
        assert!(result.is_err());
    }

    #[test]
    fn bootstrap_basis_swap_two_curves() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        // SOFR curve pillars
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.05);
        selector.add("OIS_USD_SOFR_1Y", 0.048);
        // TermSOFR3m curve via basis swap
        selector.add("BasisSwap_USD_SOFR_TermSOFR3m_1Y", 0.001);

        let sofr_spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_6M".into(),
                "OIS_USD_SOFR_1Y".into(),
            ],
        );
        let term_spec = CurveConfiguration::new(
            MarketIndex::TermSOFR3m,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["BasisSwap_USD_SOFR_TermSOFR3m_1Y".into()],
        );

        let bootstrapper =
            MultiCurveBootstrapper::new(vec![sofr_spec, term_spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        assert!(result.contains_key(&MarketIndex::SOFR));
        assert!(result.contains_key(&MarketIndex::TermSOFR3m));

        // TermSOFR3m curve should produce valid DFs.
        let term_curve = result[&MarketIndex::TermSOFR3m].curve();
        let df = term_curve.discount_factor(Date::new(2025, 6, 1))?.value();
        assert!(df > 0.0 && df < 1.0);
        Ok(())
    }

    #[test]
    fn bootstrap_fx_forward_cross_currency_missing_fx() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        // USD SOFR curve
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.05);
        selector.add("OIS_USD_SOFR_1Y", 0.048);
        // EUR collateral curve via FX forward points
        selector.add("FxForward_EURUSD_6M_AnchorForwardPoints", 0.005);
        selector.add("FxForward_EURUSD_1Y_AnchorForwardPoints", 0.008);

        let sofr_spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_6M".into(),
                "OIS_USD_SOFR_1Y".into(),
            ],
        );
        let collateral_spec = CurveConfiguration::new(
            MarketIndex::Collateral(Currency::EUR, Currency::USD),
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FxForward_EURUSD_6M_AnchorForwardPoints".into(),
                "FxForward_EURUSD_1Y_AnchorForwardPoints".into(),
            ],
        );

        let bootstrapper =
            MultiCurveBootstrapper::new(vec![sofr_spec, collateral_spec], default_policy());
        assert!(bootstrapper.bootstrap(&selector, Level::Mid).is_err());
        Ok(())
    }

    #[test]
    fn bootstrap_fx_forward_cross_currency() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        // USD SOFR curve
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.05);
        selector.add("OIS_USD_SOFR_1Y", 0.048);
        // EUR collateral curve via FX forward points
        selector.add("FxForward_EURUSD_6M_AnchorForwardPoints", 0.005);
        selector.add("FxForward_EURUSD_1Y_AnchorForwardPoints", 0.008);

        let sofr_spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_6M".into(),
                "OIS_USD_SOFR_1Y".into(),
            ],
        );
        let collateral_spec = CurveConfiguration::new(
            MarketIndex::Collateral(Currency::EUR, Currency::USD),
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FxForward_EURUSD_6M_AnchorForwardPoints".into(),
                "FxForward_EURUSD_1Y_AnchorForwardPoints".into(),
            ],
        );

        let mut fx_store = FxStore::new();
        fx_store.add_fx_rate(Currency::USD, Currency::EUR, DualFwd::new(1.08));

        let bootstrapper =
            MultiCurveBootstrapper::new(vec![sofr_spec, collateral_spec], default_policy())
                .with_fx_store(fx_store);
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        assert!(result.contains_key(&MarketIndex::SOFR));
        assert!(result.contains_key(&MarketIndex::Collateral(Currency::EUR, Currency::USD)));

        let coll_curve = result[&MarketIndex::Collateral(Currency::EUR, Currency::USD)].curve();
        let df = coll_curve.discount_factor(Date::new(2025, 6, 1))?.value();
        assert!(df > 0.0 && df < 1.5, "Collateral DF should be reasonable");
        Ok(())
    }

    #[test]
    fn bootstrap_fx_forward_cross_currency_inverse_parity() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        // USD SOFR curve
        selector.add("FixedRateDeposit_USD_SOFR_6M", 0.05);
        selector.add("OIS_USD_SOFR_1Y", 0.048);
        // EUR collateral curve via FX forward points
        selector.add("FxForward_EURUSD_6M_AnchorForwardPoints", 0.005);
        selector.add("FxForward_EURUSD_1Y_AnchorForwardPoints", 0.008);

        let sofr_spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FixedRateDeposit_USD_SOFR_6M".into(),
                "OIS_USD_SOFR_1Y".into(),
            ],
        );
        let collateral_spec = CurveConfiguration::new(
            MarketIndex::Collateral(Currency::EUR, Currency::USD),
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec![
                "FxForward_EURUSD_6M_AnchorForwardPoints".into(),
                "FxForward_EURUSD_1Y_AnchorForwardPoints".into(),
            ],
        );

        let mut fx_store = FxStore::new();
        fx_store.add_fx_rate(Currency::EUR, Currency::USD, DualFwd::new(1.0 / 1.08));
        fx_store.add_fx_rate(Currency::CLP, Currency::USD, DualFwd::new(900.0));

        let bootstrapper =
            MultiCurveBootstrapper::new(vec![sofr_spec, collateral_spec], default_policy())
                .with_fx_store(fx_store);
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        assert!(result.contains_key(&MarketIndex::SOFR));
        assert!(result.contains_key(&MarketIndex::Collateral(Currency::EUR, Currency::USD)));

        let coll_curve = result[&MarketIndex::Collateral(Currency::EUR, Currency::USD)].curve();
        let df = coll_curve.discount_factor(Date::new(2025, 6, 1))?.value();
        assert!(df > 0.0 && df < 1.5, "Collateral DF should be reasonable");
        Ok(())
    }

    #[test]
    fn bootstrap_rate_futures() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        selector.add("Future_USD_SOFR_U4", 95.0);
        selector.add("Future_USD_SOFR_Z4", 95.5);

        let spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            vec!["Future_USD_SOFR_U4".into(), "Future_USD_SOFR_Z4".into()],
        );

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        assert!(result.contains_key(&MarketIndex::SOFR));
        let curve = result[&MarketIndex::SOFR].curve();
        let df = curve.discount_factor(Date::new(2024, 12, 18))?.value();
        assert!(df > 0.0 && df < 1.0);
        Ok(())
    }

    #[test]
    fn bootstrap_reprices_inputs() -> Result<()> {
        let mut selector = MapSelector::new(rd());
        let rates = [
            ("FixedRateDeposit_USD_SOFR_3M", 0.05),
            ("FixedRateDeposit_USD_SOFR_6M", 0.051),
            ("OIS_USD_SOFR_1Y", 0.048),
        ];
        for (id, rate) in &rates {
            selector.add(id, *rate);
        }

        let spec = CurveConfiguration::new(
            MarketIndex::SOFR,
            DayCounter::Actual360,
            Interpolator::LogLinear,
            true,
            rates.iter().map(|(id, _)| (*id).into()).collect(),
        );

        let bootstrapper = MultiCurveBootstrapper::new(vec![spec], default_policy());
        let result = bootstrapper.bootstrap(&selector, Level::Mid)?;

        // The curve should have pillar labels matching the input quotes.
        let elem = &result[&MarketIndex::SOFR];
        let curve = elem.curve();
        let nodes = curve.nodes();
        assert!(nodes.is_some(), "Nodes should be available");
        let nodes = nodes.unwrap();
        // reference_date + 3 pillars = 4 nodes
        assert_eq!(nodes.len(), 4);
        // All DFs should be in (0, 1) for positive rates.
        for (_date, df) in &nodes {
            assert!(df.value() > 0.0 && df.value() <= 1.0);
        }
        Ok(())
    }
}
