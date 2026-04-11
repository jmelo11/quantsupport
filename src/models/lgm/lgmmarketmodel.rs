use std::collections::HashMap;
use std::marker::PhantomData;

use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;

use crate::{
    ad::scalar::Scalar,
    core::marketdatahandling::{
        discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
        fxrequest::FxRequest, pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::solvers::solvertraits::Matrix,
    models::lgm::lgmcomponents::{LgmFxModel, LgmRateModel},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
    xva::visitors::{
        inspector::SimulationRequest,
        marketmodel::{MarketModel, PathScenario, SimulationResponse},
    },
};

#[derive(Default)]
struct LgmMarketModelState {
    rates: HashMap<MarketIndex, HashMap<Date, f64>>,
    fx: HashMap<Currency, HashMap<Date, f64>>,
}

pub struct LgmMarketModel<'a> {
    domestic_currency: Currency,
    domestic_index: MarketIndex,
    curve_models: HashMap<MarketIndex, LgmRateModel<'a>>,
    fx_models: HashMap<Currency, LgmFxModel<'a>>,
    /// Precomputed currency → `MarketIndex` lookup (populated by `add_curve_model`).
    currency_to_index: HashMap<Currency, MarketIndex>,
    /// Maps a `MarketIndex` used in `SpotRequests` to a foreign currency
    /// so that the LGM FX state can fulfil spot-like payoffs (e.g. FX options).
    fx_spot_indices: HashMap<MarketIndex, Currency>,
    dates: Vec<Date>,
    requests: Vec<SimulationRequest>,
    reference_date: Date,
    day_counter: DayCounter,
    n_paths: usize,
    seed: u64,
    correlation_matrix: Option<Matrix<f64>>,
    state: LgmMarketModelState,
}

impl<'a> LgmMarketModel<'a> {
    #[must_use]
    pub fn new(
        domestic_currency: Currency,
        domestic_index: MarketIndex,
        reference_date: Date,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            domestic_currency,
            domestic_index,
            curve_models: HashMap::new(),
            fx_models: HashMap::new(),
            currency_to_index: HashMap::new(),
            fx_spot_indices: HashMap::new(),
            dates: Vec::new(),
            requests: Vec::new(),
            reference_date,
            day_counter,
            n_paths: 1000,
            seed: 42,
            correlation_matrix: None,
            state: LgmMarketModelState::default(),
        }
    }

    #[must_use]
    pub const fn with_n_paths(mut self, n: usize) -> Self {
        self.n_paths = n;
        self
    }

    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    #[must_use]
    pub fn with_correlation_matrix(mut self, corr: Matrix<f64>) -> Self {
        self.correlation_matrix = Some(corr);
        self
    }

    pub fn add_curve_model(&mut self, market_index: MarketIndex, model: LgmRateModel<'a>) {
        if let Ok(details) = market_index.rate_index_details() {
            self.currency_to_index
                .insert(details.currency(), market_index.clone());
        }
        self.curve_models.insert(market_index, model);
    }

    pub fn add_fx_model(&mut self, currency: Currency, model: LgmFxModel<'a>) {
        self.fx_models.insert(currency, model);
    }

    /// Registers a [`MarketIndex`] as an FX spot index so that
    /// [`SpotRequest`]s referencing it are resolved from the FX state.
    pub fn register_fx_spot_index(&mut self, index: MarketIndex, currency: Currency) {
        self.fx_spot_indices.insert(index, currency);
    }

    pub fn set_requests(&mut self, requests: Vec<SimulationRequest>) {
        self.requests = requests;
    }

    fn time_from_date(&self, date: Date) -> f64 {
        self.day_counter.year_fraction(self.reference_date, date)
    }

    fn rate_index_for_currency(&self, ccy: Currency) -> Option<MarketIndex> {
        self.currency_to_index.get(&ccy).cloned()
    }

    /// Factor ordering:
    ///   [`z_dom` (0), `z_for_1` (1), ..., `z_for_N` (N), `x_1` (N+1), ..., `x_N` (2N)]
    ///
    /// Returns (`rate_indices`, `fx_currencies`) where `rate_indices`[0] = domestic.
    fn build_factor_ordering(&self) -> (Vec<MarketIndex>, Vec<Currency>) {
        let mut rate_indices = vec![self.domestic_index.clone()];
        let mut fx_currencies = Vec::new();

        for ccy in self.fx_models.keys() {
            if let Some(idx) = self.rate_index_for_currency(*ccy) {
                if !rate_indices.contains(&idx) {
                    rate_indices.push(idx);
                }
            }
            fx_currencies.push(*ccy);
        }

        (rate_indices, fx_currencies)
    }
}

// ---------------------------------------------------------------------------
// Helper: Cholesky decomposition of a symmetric positive-definite matrix
// ---------------------------------------------------------------------------
#[allow(clippy::needless_range_loop)]
fn cholesky(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = matrix.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i][k] * l[j][k];
            }
            if i == j {
                l[i][j] = (matrix[i][i] - sum).max(0.0).sqrt();
            } else if l[j][j].abs() > 1e-14 {
                l[i][j] = (matrix[i][j] - sum) / l[j][j];
            }
        }
    }
    l
}

/// Box–Muller standard-normal sample.
fn std_normal(rng: &mut impl Rng) -> f64 {
    let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
    let u2: f64 = rng.gen_range(0.0..std::f64::consts::TAU);
    (-2.0 * u1.ln()).sqrt() * u2.cos()
}

// ---------------------------------------------------------------------------
// Path iterator
// ---------------------------------------------------------------------------
struct LgmPathIter<'a, T: Scalar> {
    model: &'a LgmMarketModel<'a>,
    rng: StdRng,
    paths_remaining: usize,
    // Pre-computed
    times: Vec<f64>,
    rate_indices: Vec<MarketIndex>,
    fx_currencies: Vec<Currency>,
    cholesky_l: Vec<Vec<f64>>,
    n_factors: usize,
    _phantom: PhantomData<T>,
}

impl<'a, T: Scalar> LgmPathIter<'a, T> {
    fn new(model: &'a LgmMarketModel<'a>) -> Self {
        let (rate_indices, fx_currencies) = model.build_factor_ordering();
        let n_rates = rate_indices.len();
        let n_fx = fx_currencies.len();
        let n_factors = n_rates + n_fx;

        // Build Cholesky factor
        let cholesky_l = model.correlation_matrix.as_ref().map_or_else(
            || {
                // Identity
                let mut id = vec![vec![0.0; n_factors]; n_factors];
                #[allow(clippy::needless_range_loop)]
                for i in 0..n_factors {
                    id[i][i] = 1.0;
                }
                id
            },
            |corr| cholesky(corr),
        );

        // Time grid: [0.0, t(date_0), t(date_1), ...]
        let mut times = Vec::with_capacity(model.dates.len() + 1);
        times.push(0.0);
        for d in &model.dates {
            times.push(model.time_from_date(*d));
        }

        Self {
            model,
            rng: StdRng::seed_from_u64(model.seed),
            paths_remaining: model.n_paths,
            times,
            rate_indices,
            fx_currencies,
            cholesky_l,
            n_factors,
            _phantom: PhantomData,
        }
    }

    /// Generate one MC path and build the full `PathScenario`.
    #[allow(clippy::needless_range_loop)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::similar_names)]
    fn generate_path(&mut self) -> Result<PathScenario<T>> {
        let n_dates = self.model.dates.len();
        let n_requests = self.model.requests.len();
        let n_rates = self.rate_indices.len();

        // State vectors
        let mut z: Vec<f64> = vec![0.0; n_rates]; // Gaussian factors (z_dom, z_for_1, ...)
        let mut x: HashMap<Currency, f64> = self
            .fx_currencies
            .iter()
            .map(|ccy| {
                let spot = self.model.fx_models[ccy].initial_spot();
                (*ccy, spot)
            })
            .collect();

        let mut x_history: Vec<HashMap<Currency, f64>> = Vec::with_capacity(n_dates);
        let mut scenario: Vec<Vec<SimulationResponse<T>>> = Vec::with_capacity(n_dates);

        for step in 0..n_dates {
            let t = self.times[step];
            let t_next = self.times[step + 1];
            let dt = t_next - t;
            let sqrt_dt = dt.sqrt();

            // 1. Generate independent normals
            let eps: Vec<f64> = (0..self.n_factors)
                .map(|_| std_normal(&mut self.rng))
                .collect();

            // 2. Apply Cholesky to get correlated increments (dW_i = sum_j L[i][j] * eps[j] * sqrt(dt))
            let dw: Vec<f64> = (0..self.n_factors)
                .map(|i| {
                    let mut w = 0.0;
                    for j in 0..=i {
                        w += self.cholesky_l[i][j] * eps[j];
                    }
                    w * sqrt_dt
                })
                .collect();

            // 3. Evolve domestic factor (index 0, drift = 0)
            let dom_model = &self.model.curve_models[&self.rate_indices[0]];
            if dt > 1e-14 {
                z[0] = dom_model.evolve_factor_euler(t, z[0], dt, 0.0, dw[0]);
            }

            // 4. Evolve foreign factors and FX spots
            if dt > 1e-14 {
                for (fi, ccy) in self.fx_currencies.iter().enumerate() {
                    let fx_model = &self.model.fx_models[ccy];
                    let for_index = self.model.rate_index_for_currency(*ccy).ok_or_else(|| {
                        QSError::NotFoundErr(format!("Rate index for currency {ccy}"))
                    })?;
                    let rate_pos = self
                        .rate_indices
                        .iter()
                        .position(|idx| *idx == for_index)
                        .ok_or_else(|| {
                            QSError::NotFoundErr(format!("Rate position for {for_index}"))
                        })?;
                    let fx_pos = n_rates + fi; // position in factor array

                    let for_model = &self.model.curve_models[&for_index];

                    // Correlations for the foreign factor drift (from original correlation matrix)
                    let (rho_zz_for_dom, rho_zx_for_fx) = self
                        .model
                        .correlation_matrix
                        .as_ref()
                        .map_or((0.0, 0.0), |c| (c[rate_pos][0], c[rate_pos][fx_pos]));

                    // Evolve foreign Gaussian factor under domestic measure
                    z[rate_pos] =
                        for_model.evolve_foreign_factor_under_domestic_measure_euler(
                            t,
                            z[rate_pos],
                            dt,
                            dw[rate_pos],
                            dom_model,
                            fx_model.fx_vol(),
                            rho_zx_for_fx,
                            rho_zz_for_dom,
                        );

                    // Evolve FX spot (log-Euler)
                    let x_curr = x[ccy];
                    let new_x = fx_model.evolve_fx_spot_log_euler(
                        t,
                        x_curr,
                        z[0],
                        z[rate_pos],
                        dt,
                        dw[fx_pos],
                    )?;
                    x.insert(*ccy, new_x);
                }
            }

            x_history.push(x.clone());

            // 5. Build SimulationResponse for each request at this date
            let eval_date = self.model.dates[step];
            let mut date_responses = Vec::with_capacity(n_requests);

            for req in &self.model.requests {
                let mut resp = SimulationResponse::new();

                // Forward rate
                if let Some(fwd_req) = &req.forward_rate_request {
                    let idx = fwd_req.market_index();
                    if let Some(curve_model) = self.model.curve_models.get(&idx) {
                        // Find the z_t for this curve
                        let rate_pos = self
                            .rate_indices
                            .iter()
                            .position(|ri| *ri == idx)
                            .unwrap_or(0);
                        let z_t = z[rate_pos];

                        let start = fwd_req
                            .start_date()
                            .unwrap_or_else(|| fwd_req.fixing_date());
                        let end = fwd_req.end_date().unwrap_or_else(|| fwd_req.fixing_date());
                        let t_eval = self.model.time_from_date(eval_date);
                        let t_start = self.model.time_from_date(start);
                        let t_end = self.model.time_from_date(end);

                        if (t_end - t_start).abs() < 1e-14 {
                            // Instantaneous forward rate
                            let rate =
                                curve_model.instantaneous_forward_rate(t_eval, t_start, z_t)?;
                            resp.forward_rates = Some(T::scalar(rate));
                        } else {
                            // Simply-compounded forward rate: L = (1/tau)*(P(t,S,z)/P(t,T,z) - 1)
                            let p_start = curve_model.P_discount(t_eval, t_start, z_t)?;
                            let p_end = curve_model.P_discount(t_eval, t_end, z_t)?;
                            let tau = t_end - t_start;
                            let rate = (p_start / p_end - 1.0) / tau;
                            resp.forward_rates = Some(T::scalar(rate));
                        }
                    }
                }

                // FX rate
                if let Some(fx_req) = &req.fx_request {
                    let base = fx_req.base();
                    if base == self.model.domestic_currency {
                        resp.fx_rates = Some(T::one());
                    } else if let Some(&spot) = x.get(&base) {
                        resp.fx_rates = Some(T::scalar(spot));
                    } else {
                        resp.fx_rates = Some(T::one());
                    }
                }

                // Discounting
                if let Some(disc_req) = &req.discount_request {
                    let idx = disc_req.market_index();
                    if let Some(curve_model) = self.model.curve_models.get(&idx) {
                        let rate_pos = self
                            .rate_indices
                            .iter()
                            .position(|ri| *ri == idx)
                            .unwrap_or(0);
                        let z_t = z[rate_pos];
                        let t_eval = self.model.time_from_date(eval_date);
                        let t_pay = self.model.time_from_date(disc_req.date());
                        if t_pay > t_eval {
                            if let Ok(df) = curve_model.P_discount(t_eval, t_pay, z_t) {
                                resp.discounts = Some(T::scalar(df));
                            }
                        } else {
                            resp.discounts = Some(T::one());
                        }
                    }
                }

                resp.numeraire = None;

                // Spot request — deferred to post-processing pass below
                // so the observation_date's simulated state can be used.

                // Path-dependent request (not modeled in LGM)
                if req.path_dependent_request.is_some() {
                    resp.path_dependent_observations = None;
                }

                date_responses.push(resp);
            }

            scenario.push(date_responses);
        }

        // Post-processing: resolve spot requests using S at the observation date.
        //
        // Spot requests (e.g. FX options) are skipped during the main loop
        // because the spot value at the observation date may belong to a
        // future simulation step that has not been computed yet.  Once the
        // full path is available in `x_history`, we go back and fill in the
        // spot field for every (step, request) pair.
        //
        // For each spot request we find the latest simulation step whose
        // date is <= the requested observation date (`rposition`) and read
        // the FX spot from `x_history` at that step.
        for step in 0..n_dates {
            for (ri, req) in self.model.requests.iter().enumerate() {
                if let Some(spot_req) = &req.spot_request {
                    let idx = spot_req.market_index();
                    let obs_date = spot_req.date();
                    if let Some(ccy) = self.model.fx_spot_indices.get(&idx) {
                        let obs_step = self
                            .model
                            .dates
                            .iter()
                            .rposition(|d| *d <= obs_date)
                            .unwrap_or(step)
                            .min(n_dates - 1);
                        if let Some(&fx_spot) = x_history[obs_step].get(ccy) {
                            scenario[step][ri].spots = Some(T::scalar(fx_spot));
                        }
                    }
                }
            }
        }

        Ok(scenario)
    }
}

impl<T: Scalar> Iterator for LgmPathIter<'_, T> {
    type Item = PathScenario<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.paths_remaining == 0 {
            return None;
        }
        self.paths_remaining -= 1;
        self.generate_path().ok()
    }
}

// SAFETY: The iterator only holds &LgmMarketModel (shared ref) and StdRng (Send).
// LgmMarketModel contains &dyn InterestRatesTermStructure which must be Sync for
// the shared reference to be Send.  This is sound because the trait objects are
// read-only during iteration.
unsafe impl<T: Scalar> Send for LgmPathIter<'_, T> {}

// ---------------------------------------------------------------------------
// MarketModel implementation
// ---------------------------------------------------------------------------
impl MarketModel<f64> for LgmMarketModel<'_> {
    fn path_iter(&self) -> Box<dyn Iterator<Item = PathScenario<f64>> + Send + '_> {
        Box::new(LgmPathIter::<f64>::new(self))
    }

    fn set_evaluation_dates(&mut self, dates: Vec<Date>) {
        self.dates = dates;
    }

    fn resolve_discount_request(&self, eval_date: Date, request: &DiscountRequest) -> Result<f64> {
        let idx = request.market_index();
        let curve_model = self
            .curve_models
            .get(&idx)
            .ok_or_else(|| QSError::NotFoundErr(format!("Curve model for {idx}")))?;
        let t_eval = self.time_from_date(eval_date);
        let t_pay = self.time_from_date(request.date());
        let z_t = self.state_z(&idx, eval_date).unwrap_or(0.0);
        curve_model.P_discount(t_eval, t_pay, z_t)
    }

    fn resolve_forward_rate_request(
        &self,
        eval_date: Date,
        request: &ForwardRateRequest,
    ) -> Result<f64> {
        let idx = request.market_index();
        let curve_model = self
            .curve_models
            .get(&idx)
            .ok_or_else(|| QSError::NotFoundErr(format!("Curve model for {idx}")))?;
        let t_eval = self.time_from_date(eval_date);
        let z_t = self.state_z(&idx, eval_date).unwrap_or(0.0);

        let start = request
            .start_date()
            .unwrap_or_else(|| request.fixing_date());
        let end = request.end_date().unwrap_or_else(|| request.fixing_date());
        let t_start = self.time_from_date(start);
        let t_end = self.time_from_date(end);

        if (t_end - t_start).abs() < 1e-14 {
            curve_model.instantaneous_forward_rate(t_eval, t_start, z_t)
        } else {
            let p_s = curve_model.P_discount(t_eval, t_start, z_t)?;
            let p_e = curve_model.P_discount(t_eval, t_end, z_t)?;
            let tau = t_end - t_start;
            Ok((p_s / p_e - 1.0) / tau)
        }
    }

    fn resolve_fx_request(&self, eval_date: Date, request: &FxRequest) -> Result<f64> {
        let base = request.base();
        if base == self.domestic_currency {
            return Ok(1.0);
        }
        self.state_fx(base, eval_date)
            .ok_or_else(|| QSError::NotFoundErr(format!("FX state for {base} at {eval_date}")))
    }

    fn resolve_spot_request(&self, eval_date: Date, request: &SpotRequest) -> Result<f64> {
        let idx = request.market_index();
        self.fx_spot_indices.get(&idx).map_or_else(
            || {
                Err(QSError::NotImplementedErr(
                    "Spot request not supported in LGM rate model".into(),
                ))
            },
            |ccy| {
                self.state_fx(*ccy, eval_date).ok_or_else(|| {
                    QSError::NotFoundErr(format!("FX state for {ccy} at {eval_date}"))
                })
            },
        )
    }

    fn resolve_path_dependent_request(
        &self,
        _eval_date: Date,
        _request: &PathDependentRequest,
    ) -> Result<f64> {
        Err(QSError::NotImplementedErr(
            "Path-dependent request not supported in LGM rate model".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// State accessors (used by resolve methods when state is populated externally)
// ---------------------------------------------------------------------------
impl LgmMarketModel<'_> {
    fn state_z(&self, index: &MarketIndex, date: Date) -> Option<f64> {
        self.state.rates.get(index)?.get(&date).copied()
    }

    fn state_fx(&self, currency: Currency, date: Date) -> Option<f64> {
        self.state.fx.get(&currency)?.get(&date).copied()
    }
}
