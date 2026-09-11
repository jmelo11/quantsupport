//! Correlated path generation for LGM rates, FX spots, and equity spots.
//!
//! `LgmMarketModel` combines the rate, FX, and equity components defined in
//! `lgmcomponents`. It owns the simulation dates, quasi-random sequence,
//! factor correlations, path state, and market responses consumed by pricing
//! and exposure evaluators.
//!
//! The configured [`DayCounter`](crate::time::daycounter::DayCounter) converts
//! each simulation date into a year fraction from `reference_date`. The
//! domestic rate factor defines the pricing measure and path numeraire.
//! Foreign rate drifts are transformed into that domestic measure.

use std::collections::HashMap;

use sobol_burley::sample as sobol_sample;

use crate::{
    ad::scalar::Scalar,
    core::marketdatahandling::{
        discountrequest::DiscountRequest, forwardraterequest::ForwardRateRequest,
        fxrequest::FxRequest, pathdependentrequest::PathDependentRequest, spotrequest::SpotRequest,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    math::linalg::cholesky,
    math::solvers::solvertraits::Matrix,
    models::lgm::lgmcomponents::{LgmEquityModel, LgmFxModel, LgmRateModel},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
    xva::visitors::{
        marketmodel::{MarketModel, PathScenario, SimulationResponse},
        preprocessorexecutor::SimulationRequest,
    },
};

/// Cached simulation state: Gaussian factors and FX spots keyed by date.
#[derive(Default)]
struct LgmMarketModelState {
    rates: HashMap<MarketIndex, HashMap<Date, f64>>,
    fx: HashMap<Currency, HashMap<Date, f64>>,
}

/// Path generator for an LGM market expressed in one domestic currency.
///
/// # Components
///
/// The model contains four component types:
///
/// - one domestic [`LgmRateModel`], which supplies the simulation measure and
///   numeraire;
/// - one [`LgmRateModel`] for each simulated foreign currency;
/// - one [`LgmFxModel`] for each simulated foreign currency;
/// - zero or more [`LgmEquityModel`] instances using the domestic rate model.
///
/// A derived curve reuses a registered rate factor and combines it with its
/// own initial term structure. This represents a deterministic basis between
/// the two curves.
///
/// # Model state and dynamics
///
/// At simulation time `t`, the factor vector is
///
/// ```text
/// Y(t) = [z_d(t), z_f1(t), ..., z_fF(t), log X_1(t), ..., log X_F(t),
///         log S_1(t), ..., log S_E(t)]
/// ```
///
/// Here `z_d` is the domestic Gaussian rate state, each `z_fi` is a foreign
/// Gaussian rate state, `X_i` is domestic currency per unit of foreign
/// currency, and `S_j` is an equity spot in domestic currency. Rate states
/// start at zero. Spot states start at the values passed to [`LgmFxModel::new`]
/// and [`LgmEquityModel::new`].
///
/// The components evolve according to
///
/// ```text
/// dz_d     = alpha_d(t) dW_d
/// dz_fi    = gamma_i(t) dt + alpha_i(t) dW_fi
/// d log Xi = (mu_Xi(t) - 0.5 sigma_Xi^2) dt + sigma_Xi dW_Xi
/// d log Sj = (mu_Sj(t) - 0.5 sigma_Sj^2) dt + sigma_Sj dW_Sj
/// ```
///
/// The domestic-measure drifts are
///
/// ```text
/// gamma_i = rho(fi,d) alpha_i alpha_d H_d
///           - alpha_i^2 H_i
///           - rho(fi,Xi) sigma_Xi alpha_i
/// mu_Xi   = r_d - r_i + rho(d,Xi) alpha_d H_d sigma_Xi
/// mu_Sj   = r_d - q_j + rho(d,Sj) alpha_d H_d sigma_Sj
/// ```
///
/// `r_d` and `r_i` are domestic and foreign short rates, `q_j` is the equity
/// dividend yield, and each `rho(a,b)` is the correlation between the named
/// Brownian shocks. The correlation matrix couples every `dW` in the system.
/// [`LgmRateModel`], [`LgmFxModel`], and [`LgmEquityModel`] document the
/// component parameters and their units.
///
/// # State and request mapping
///
/// | Registration | Simulated state | Requests resolved from that state |
/// |---|---|---|
/// | `add_curve_model(index, model)` | Gaussian rate factor `z_index(t)` | discount factors and forward rates for `index` |
/// | `add_fx_model(currency, model)` | foreign rate factor and FX spot `X_currency(t)` | FX conversion rates for `currency` |
/// | `register_fx_spot_index(index, currency)` | existing `X_currency(t)` | spot observations addressed by `index` |
/// | `add_equity_model(name, model)` | equity spot `S_name(t)` | equity spot observations addressed by `name` |
/// | `set_curve_driver(index, driver)` | existing `z_driver(t)` | discount factors and forwards on the derived curve `index` |
///
/// Component models are stored by value and borrow their initial term
/// structures. Those term structures live for the market model's lifetime
/// `'a`.
///
/// # Path algorithm
///
/// For each path, the model:
///
/// 1. generates Owen-scrambled Sobol standard normals;
/// 2. pairs consecutive paths with opposite normal signs;
/// 3. applies the Cholesky factor of the correlation matrix;
/// 4. evolves rate states and lognormal spots across the date grid;
/// 5. resolves the requested forwards, discount factors, FX rates, spots, and
///    path numeraires at each date.
///
/// # Correlation matrix
///
/// Rows and columns use this factor order:
///
/// 1. domestic rate;
/// 2. foreign rates, sorted by foreign ISO currency code;
/// 3. FX spots, in the same currency order;
/// 4. equities, sorted by underlying name.
///
/// With USD domestic, EUR foreign, and AAPL equity, the order is
/// `[USD rate, EUR rate, EUR/USD spot, AAPL spot]`. For `F` foreign currencies
/// and `E` equities, the matrix dimension is `1 + 2 * F + E`. An omitted
/// matrix produces an identity correlation matrix. Derived curves reuse their
/// driver's factor.
///
/// # Construction
///
/// 1. Create the container with [`Self::new`] and configure paths, seed, and
///    correlations with the `with_*` methods.
/// 2. Register the domestic and foreign rate components with
///    [`Self::add_curve_model`].
/// 3. Register FX and equity models with [`Self::add_fx_model`] and
///    [`Self::add_equity_model`].
/// 4. Register derived curves and FX spot aliases as required.
/// 5. Supply evaluation dates and requests through the [`MarketModel`] trait.
///
/// # Single-currency example
///
/// ```
/// use quantsupport::prelude::*;
///
/// let reference_date = Date::new(2026, 9, 9);
/// let curve = FlatForwardTermStructure::new(
///     reference_date,
///     0.04,
///     RateDefinition::default(),
/// );
/// let rate_model = LgmRateModel::new(
///     0.03, // lambda = 0.03 / year
///     0.01, // sigma = 100 bp / sqrt(year)
///     &curve,
/// );
///
/// let mut market = LgmMarketModel::new(
///     Currency::USD,
///     MarketIndex::SOFR,
///     reference_date,
///     DayCounter::Actual365,
/// )
/// .with_n_paths(2_048)
/// .with_seed(42);
/// market.add_curve_model(MarketIndex::SOFR, rate_model);
/// market.set_evaluation_dates(vec![Date::new(2027, 9, 9)]);
///
/// assert_eq!(market.n_paths(), 2_048);
/// ```
pub struct LgmMarketModel<'a, T: Scalar> {
    domestic_currency: Currency,
    domestic_index: MarketIndex,
    curve_models: HashMap<MarketIndex, LgmRateModel<'a, T>>,
    fx_models: HashMap<Currency, LgmFxModel<'a, T>>,
    equity_models: HashMap<String, LgmEquityModel<'a, T>>,
    currency_to_index: HashMap<Currency, MarketIndex>,
    fx_spot_indices: HashMap<MarketIndex, Currency>,
    /// Derived curve → driving rate model. FX-implied collateral curves share
    /// the driver's simulated factor. Their discount factors use the derived
    /// curve's initial term structure, producing a deterministic basis.
    curve_drivers: HashMap<MarketIndex, MarketIndex>,
    dates: Vec<Date>,
    requests: Vec<SimulationRequest>,
    request_dates: Vec<Option<Date>>,
    compact_dated_requests: bool,
    path_times: Vec<f64>,
    path_rate_indices: Vec<MarketIndex>,
    path_fx_currencies: Vec<Currency>,
    path_equity_names: Vec<String>,
    path_cholesky_l: Vec<Vec<f64>>,
    path_n_factors: usize,
    request_indices_by_step: Vec<Vec<usize>>,
    reference_date: Date,
    day_counter: DayCounter,
    n_paths: usize,
    seed: u64,
    correlation_matrix: Option<Matrix<f64>>,
    state: LgmMarketModelState,
}

impl<'a, T: Scalar> LgmMarketModel<'a, T> {
    /// Creates an empty market model in the domestic pricing measure.
    ///
    /// # Arguments
    ///
    /// - `domestic_currency` — numeraire and reporting currency. FX spots are
    ///   expressed as units of this currency per unit of foreign currency.
    /// - `domestic_index` — rate index whose LGM factor drives the domestic
    ///   discount curve. Register its component model with
    ///   [`Self::add_curve_model`] before path generation.
    /// - `reference_date` — time-zero date for curves, model states, and all
    ///   year-fraction calculations.
    /// - `day_counter` — convention used to convert simulation dates into
    ///   model times. Curves, calibration parameters, and path times share
    ///   this convention.
    ///
    /// The default simulation settings are 1,000 paths and scramble seed 42.
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
            equity_models: HashMap::new(),
            currency_to_index: HashMap::new(),
            fx_spot_indices: HashMap::new(),
            curve_drivers: HashMap::new(),
            dates: Vec::new(),
            requests: Vec::new(),
            request_dates: Vec::new(),
            compact_dated_requests: false,
            path_times: Vec::new(),
            path_rate_indices: Vec::new(),
            path_fx_currencies: Vec::new(),
            path_equity_names: Vec::new(),
            path_cholesky_l: Vec::new(),
            path_n_factors: 0,
            request_indices_by_step: Vec::new(),
            reference_date,
            day_counter,
            n_paths: 1000,
            seed: 42,
            correlation_matrix: None,
            state: LgmMarketModelState::default(),
        }
    }

    /// Sets the number of Monte Carlo paths generated by the model.
    ///
    /// Consecutive paths form `(Sobol draw, antithetic draw)` pairs. Even path
    /// counts preserve complete pairs. The pricing service uses at least 2,048
    /// paths for optional payoffs.
    #[must_use]
    pub const fn with_n_paths(mut self, n: usize) -> Self {
        self.n_paths = n;
        self
    }

    /// Sets the Owen-scrambling seed used by the Sobol sequence.
    ///
    /// Equal seeds, grids, models, and path indices produce repeatable draws.
    /// Path count and antithetic pairing are configured independently.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Sets the Brownian-factor correlation matrix.
    ///
    /// Rows and columns follow the ordering documented under
    /// [`LgmMarketModel`]'s **Correlation matrix** section. The matrix is square
    /// and symmetric positive semidefinite, with unit diagonal and entries in
    /// `[-1, 1]`. [`LgmFxModel`] and [`LgmEquityModel`] use the corresponding
    /// rate/spot entries when calculating measure-change drifts.
    ///
    /// The matrix is decomposed when the path layout is rebuilt after models
    /// or evaluation dates are registered.
    #[must_use]
    pub fn with_correlation_matrix(mut self, corr: Matrix<f64>) -> Self {
        self.correlation_matrix = Some(corr);
        self
    }

    /// Registers the LGM dynamics and initial curve for a rate index.
    ///
    /// `market_index` is the curve identifier used by simulation requests.
    /// Its currency metadata populates the currency-to-rate-factor mapping.
    /// `model` supplies that curve's dynamics and initial term structure.
    /// Registering an existing index replaces its component model.
    pub fn add_curve_model(&mut self, market_index: MarketIndex, model: LgmRateModel<'a, T>) {
        if let Ok(details) = market_index.rate_index_details() {
            self.currency_to_index
                .insert(details.currency(), market_index.clone());
        }
        self.curve_models.insert(market_index, model);
        self.rebuild_path_layout();
    }

    /// Registers an FX model for one foreign currency.
    ///
    /// `currency` identifies the foreign currency. `model` contains its
    /// foreign rate model and a spot quoted in domestic-currency units per one
    /// foreign-currency unit. The corresponding foreign curve registration
    /// supplies the currency-to-rate-factor mapping. Each FX registration adds
    /// one foreign-rate factor and one FX-spot factor to the path layout.
    pub fn add_fx_model(&mut self, currency: Currency, model: LgmFxModel<'a, T>) {
        self.fx_models.insert(currency, model);
        self.rebuild_path_layout();
    }

    /// Registers an equity model under its script-visible underlying name.
    ///
    /// [`SpotRequest`]s referencing [`MarketIndex::Equity`] with this name
    /// are resolved from the simulated equity spot. `model` references the
    /// domestic rate component used for the equity drift. Registering an
    /// existing name replaces its component model.
    pub fn add_equity_model(&mut self, name: String, model: LgmEquityModel<'a, T>) {
        self.equity_models.insert(name, model);
        self.rebuild_path_layout();
    }

    /// Declares a curve as derived from another curve's stochastic factor.
    ///
    /// The derived curve's rate model carries the driver's `lambda` and sigma
    /// schedule together with the derived curve's initial term structure.
    /// Simulation reconstructs its discount factors from the driver's Gaussian
    /// state, preserving the time-zero basis deterministically.
    ///
    /// `index` is the derived curve used by requests. `driver` identifies the
    /// registered curve whose Gaussian state drives it. The derived curve
    /// retains its own time-zero discount factors and reuses the driver's
    /// Brownian factor.
    pub fn set_curve_driver(&mut self, index: MarketIndex, driver: MarketIndex) {
        self.curve_drivers.insert(index, driver);
    }

    /// Resolves the index whose simulated factor drives `index`.
    fn factor_index<'b>(&'b self, index: &'b MarketIndex) -> &'b MarketIndex {
        self.curve_drivers.get(index).unwrap_or(index)
    }

    /// Registers a [`MarketIndex`] as an FX spot index so that
    /// [`SpotRequest`]s referencing it are resolved from the FX state.
    ///
    /// `index` is the script/request identifier and `currency` selects the
    /// foreign-currency FX model registered with [`Self::add_fx_model`].
    pub fn register_fx_spot_index(&mut self, index: MarketIndex, currency: Currency) {
        self.fx_spot_indices.insert(index, currency);
    }

    /// Replaces the market-data requests evaluated on every simulation date.
    ///
    /// Each request may ask for a forward, discount factor, FX rate, spot, or
    /// path-dependent observation. This method assigns every request to every
    /// date. Script pricing subsequently calls [`MarketModel::set_request_dates`]
    /// to associate requests with their event dates.
    pub fn set_requests(&mut self, requests: Vec<SimulationRequest>) {
        self.request_dates = vec![None; requests.len()];
        self.compact_dated_requests = false;
        self.requests = requests;
        self.rebuild_request_layout();
    }

    fn time_from_date(&self, date: Date) -> f64 {
        self.day_counter.year_fraction(self.reference_date, date)
    }

    fn rate_index_for_currency(&self, ccy: Currency) -> Option<MarketIndex> {
        self.currency_to_index.get(&ccy).cloned()
    }

    /// Factor ordering:
    ///   [`z_dom` (0), `z_for_1` (1), ..., `z_for_N` (N), `x_1` (N+1), ..., `x_N` (2N),
    ///    `s_1` (2N+1), ..., `s_M` (2N+M)]
    ///
    /// Returns (`rate_indices`, `fx_currencies`, `equity_names`) where
    /// `rate_indices`[0] = domestic. Equity names are sorted for determinism.
    fn build_factor_ordering(&self) -> (Vec<MarketIndex>, Vec<Currency>, Vec<String>) {
        let mut rate_indices = vec![self.domestic_index.clone()];
        let mut fx_currencies: Vec<Currency> = self.fx_models.keys().copied().collect();
        fx_currencies.sort_by_key(ToString::to_string);
        for currency in &fx_currencies {
            if let Some(index) = self.rate_index_for_currency(*currency) {
                if !rate_indices.contains(&index) {
                    rate_indices.push(index);
                }
            }
        }

        let mut equity_names: Vec<String> = self.equity_models.keys().cloned().collect();
        equity_names.sort();

        (rate_indices, fx_currencies, equity_names)
    }

    /// Rebuilds the cached simulation grid and Brownian-factor layout.
    ///
    /// The method derives `path_times` from `dates`, starting with time zero,
    /// and refreshes the ordered rate, FX, and equity factor lists returned by
    /// [`Self::build_factor_ordering`]. It then sets `path_n_factors` and
    /// computes the Cholesky factor used to correlate path increments. An
    /// identity matrix supplies independent factors when no correlation matrix
    /// is configured.
    ///
    /// The resulting caches satisfy these invariants:
    ///
    /// - `path_times.len() == dates.len() + 1`;
    /// - `path_times[0] == 0.0`;
    /// - `path_n_factors` equals the combined number of cached rate, FX, and
    ///   equity factors;
    /// - `path_cholesky_l` has one row and column per factor.
    ///
    /// Rebuilding the path layout also rebuilds the request-to-date mapping
    /// because changes to the simulation dates affect both caches.
    fn rebuild_path_layout(&mut self) {
        self.path_times.clear();
        self.path_times.reserve(self.dates.len() + 1);
        self.path_times.push(0.0);
        for date in &self.dates {
            self.path_times
                .push(self.day_counter.year_fraction(self.reference_date, *date));
        }

        let (rate_indices, fx_currencies, equity_names) = self.build_factor_ordering();
        self.path_rate_indices = rate_indices;
        self.path_fx_currencies = fx_currencies;
        self.path_equity_names = equity_names;
        self.path_n_factors = self.path_rate_indices.len()
            + self.path_fx_currencies.len()
            + self.path_equity_names.len();
        self.path_cholesky_l = self.correlation_matrix.as_ref().map_or_else(
            || {
                let mut identity = vec![vec![0.0; self.path_n_factors]; self.path_n_factors];
                for (index, row) in identity.iter_mut().enumerate() {
                    row[index] = 1.0;
                }
                identity
            },
            |correlation| cholesky(correlation),
        );
        self.rebuild_request_layout();
    }

    /// Rebuilds the request indices evaluated at each simulation date.
    ///
    /// `request_indices_by_step[step]` contains indices into `requests` for
    /// `dates[step]`, preserving the original request order. Standard layout
    /// assigns every request to every date. Compact dated layout assigns a
    /// request to its matching date; a `None` entry in `request_dates` assigns
    /// that request to every date.
    ///
    /// [`Self::set_requests`] initializes `request_dates` to the same length as
    /// `requests`. [`MarketModel::set_request_dates`] accepts a replacement
    /// only when its length matches, keeping indexed access valid here.
    fn rebuild_request_layout(&mut self) {
        self.request_indices_by_step = self
            .dates
            .iter()
            .map(|event_date| {
                self.requests
                    .iter()
                    .enumerate()
                    .filter_map(|(index, _)| {
                        (!self.compact_dated_requests
                            || self.request_dates[index].is_none_or(|date| date == *event_date))
                        .then_some(index)
                    })
                    .collect()
            })
            .collect();
    }
}

const SOBOL_DIMENSIONS: usize = 256;
const SOBOL_SEQUENCE_LENGTH: usize = 1 << 16;

/// Fold a public 64-bit simulation seed into the 32-bit scramble accepted by
/// `sobol_burley`, preserving changes in both halves.
const fn fold_seed(seed: u64) -> u32 {
    let bytes = seed.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        ^ u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
}

/// One Owen-scrambled Sobol coordinate, padded beyond the native 256
/// dimensions by starting an independently scrambled dimension block.
fn sobol_uniform(sample_index: u32, raw_dimension: usize, sequence_block: usize, seed: u64) -> f64 {
    let dimension = u32::try_from(raw_dimension % SOBOL_DIMENSIONS).unwrap_or(0);
    let dimension_block = u32::try_from(raw_dimension / SOBOL_DIMENSIONS).unwrap_or(u32::MAX);
    let sequence_block = u32::try_from(sequence_block).unwrap_or(u32::MAX);
    let scramble = fold_seed(seed)
        .wrapping_add(dimension_block.wrapping_mul(0x9e37_79b9))
        .wrapping_add(sequence_block.wrapping_mul(0x85eb_ca6b));
    // Keep the inverse transform away from exactly 0 and 1. Scrambled Sobol
    // output is f32, so this clamp is well below its effective resolution.
    f64::from(sobol_sample(sample_index, dimension, scramble)).clamp(1.0e-12, 1.0 - 1.0e-12)
}

/// Standard-normal coordinate obtained from two Sobol dimensions through a
/// Box–Muller transform. The caller supplies the sign for antithetic pairing.
fn sobol_normal(
    sample_index: u32,
    normal_dimension: usize,
    sequence_block: usize,
    seed: u64,
    antithetic_sign: f64,
) -> f64 {
    let uniform_dimension = normal_dimension.saturating_mul(2);
    let u1 = sobol_uniform(sample_index, uniform_dimension, sequence_block, seed);
    let u2 = sobol_uniform(
        sample_index,
        uniform_dimension.saturating_add(1),
        sequence_block,
        seed,
    );
    antithetic_sign * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Read-only pre-computed data shared by all path generators.
struct LgmPathContext<'a, T: Scalar> {
    model: &'a LgmMarketModel<'a, T>,
    times: &'a [f64],
    rate_indices: &'a [MarketIndex],
    fx_currencies: &'a [Currency],
    equity_names: &'a [String],
    cholesky_l: &'a [Vec<f64>],
    n_factors: usize,
    request_indices_by_step: &'a [Vec<usize>],
}

// SAFETY: `LgmPathContext` is read-only during parallel path generation.
// The `&dyn InterestRatesTermStructure` references inside `LgmRateModel`
// are only used for discount-factor lookups (pure reads).
unsafe impl<T: Scalar> Sync for LgmPathContext<'_, T> {}
unsafe impl<T: Scalar> Send for LgmPathContext<'_, T> {}

impl<'a, T: Scalar> LgmPathContext<'a, T> {
    fn new(model: &'a LgmMarketModel<'a, T>) -> Self {
        Self {
            model,
            times: &model.path_times,
            rate_indices: &model.path_rate_indices,
            fx_currencies: &model.path_fx_currencies,
            equity_names: &model.path_equity_names,
            cholesky_l: &model.path_cholesky_l,
            n_factors: model.path_n_factors,
            request_indices_by_step: &model.request_indices_by_step,
        }
    }

    fn correlated_increments(
        &self,
        step: usize,
        sample_index: u32,
        sequence_block: usize,
        antithetic_sign: f64,
        sqrt_time_step: f64,
    ) -> Vec<f64> {
        let independent_normals: Vec<f64> = (0..self.n_factors)
            .map(|factor| {
                let normal_dimension = step.saturating_mul(self.n_factors).saturating_add(factor);
                sobol_normal(
                    sample_index,
                    normal_dimension,
                    sequence_block,
                    self.model.seed,
                    antithetic_sign,
                )
            })
            .collect();

        (0..self.n_factors)
            .map(|factor| {
                self.cholesky_l[factor]
                    .iter()
                    .zip(&independent_normals)
                    .take(factor + 1)
                    .map(|(loading, normal)| loading * normal)
                    .sum::<f64>()
                    * sqrt_time_step
            })
            .collect()
    }

    fn evolve_market_state(
        &self,
        time: f64,
        time_step: f64,
        increments: &[f64],
        rate_factors: &mut [T],
        fx_spots: &mut HashMap<Currency, T>,
        equity_spots: &mut HashMap<String, T>,
    ) -> Result<()> {
        if time_step <= 1e-14 {
            return Ok(());
        }

        let rate_count = self.rate_indices.len();
        let domestic_model = &self.model.curve_models[&self.rate_indices[0]];
        rate_factors[0] = domestic_model.evolve_domestic_factor_euler(
            time,
            rate_factors[0],
            time_step,
            increments[0],
        );

        for (foreign_position, currency) in self.fx_currencies.iter().enumerate() {
            let fx_model = &self.model.fx_models[currency];
            let foreign_index = self
                .model
                .rate_index_for_currency(*currency)
                .ok_or_else(|| {
                    QSError::NotFoundErr(format!("Rate index for currency {currency}"))
                })?;
            let rate_position = self
                .rate_indices
                .iter()
                .position(|index| *index == foreign_index)
                .ok_or_else(|| {
                    QSError::NotFoundErr(format!("Rate position for {foreign_index}"))
                })?;
            let fx_position = rate_count + foreign_position;
            let foreign_model = &self.model.curve_models[&foreign_index];
            let (rate_correlation, fx_correlation) =
                self.model
                    .correlation_matrix
                    .as_ref()
                    .map_or((0.0, 0.0), |correlation| {
                        (
                            correlation[rate_position][0],
                            correlation[rate_position][fx_position],
                        )
                    });

            rate_factors[rate_position] = foreign_model
                .evolve_foreign_factor_under_domestic_measure_euler(
                    time,
                    rate_factors[rate_position],
                    time_step,
                    increments[rate_position],
                    domestic_model,
                    fx_model.fx_vol().value(),
                    fx_correlation,
                    rate_correlation,
                );

            let current_spot = fx_spots[currency];
            let evolved_spot = fx_model.evolve_fx_spot_log_euler(
                time,
                current_spot,
                rate_factors[0],
                rate_factors[rate_position],
                time_step,
                increments[fx_position],
            )?;
            fx_spots.insert(*currency, evolved_spot);
        }

        for (equity_position, name) in self.equity_names.iter().enumerate() {
            let equity_model = &self.model.equity_models[name];
            let factor_position = rate_count + self.fx_currencies.len() + equity_position;
            let evolved_spot = equity_model.evolve_spot_log_euler(
                time,
                equity_spots[name],
                rate_factors[0],
                time_step,
                increments[factor_position],
            )?;
            equity_spots.insert(name.clone(), evolved_spot);
        }
        Ok(())
    }

    fn responses_for_step(
        &self,
        step: usize,
        next_time: f64,
        rate_factors: &[T],
        fx_spots: &HashMap<Currency, T>,
    ) -> Result<Vec<SimulationResponse<T>>> {
        let evaluation_date = self.model.dates[step];
        let domestic_model = &self.model.curve_models[&self.rate_indices[0]];
        let mut responses = Vec::with_capacity(self.request_indices_by_step[step].len());

        for &request_index in &self.request_indices_by_step[step] {
            let request = &self.model.requests[request_index];
            let mut response = SimulationResponse::new();

            if let Some(forward_request) = &request.forward_rate_request {
                let index = forward_request.market_index();
                if let Some(curve_model) = self.model.curve_models.get(&index) {
                    let factor_index = self.model.factor_index(&index);
                    let rate_position = self
                        .rate_indices
                        .iter()
                        .position(|rate_index| rate_index == factor_index)
                        .unwrap_or(0);
                    let rate_factor = rate_factors[rate_position];
                    let start = forward_request
                        .start_date()
                        .unwrap_or_else(|| forward_request.fixing_date());
                    let end = forward_request
                        .end_date()
                        .unwrap_or_else(|| forward_request.fixing_date());
                    let evaluation_time = self.model.time_from_date(evaluation_date);
                    let start_time = self.model.time_from_date(start);
                    let end_time = self.model.time_from_date(end);

                    response.forward_rates = if (end_time - start_time).abs() < 1e-14 {
                        Some(curve_model.instantaneous_forward_rate(
                            evaluation_time,
                            start_time,
                            rate_factor,
                        )?)
                    } else {
                        let start_discount =
                            curve_model.P_discount(evaluation_time, start_time, rate_factor)?;
                        let end_discount =
                            curve_model.P_discount(evaluation_time, end_time, rate_factor)?;
                        Some(
                            start_discount
                                .div_val(end_discount)
                                .sub_val(T::one())
                                .div_val(T::scalar(end_time - start_time)),
                        )
                    };
                }
            }

            if let Some(fx_request) = &request.fx_request {
                let base = fx_request.base();
                response.fx_rates = if base == self.model.domestic_currency {
                    Some(T::one())
                } else {
                    Some(fx_spots.get(&base).copied().unwrap_or_else(T::one))
                };
            }

            if let Some(discount_request) = &request.discount_request {
                let index = discount_request.market_index();
                if let Some(curve_model) = self.model.curve_models.get(&index) {
                    let factor_index = self.model.factor_index(&index);
                    let rate_position = self
                        .rate_indices
                        .iter()
                        .position(|rate_index| rate_index == factor_index)
                        .unwrap_or(0);
                    let evaluation_time = self.model.time_from_date(evaluation_date);
                    let payment_time = self.model.time_from_date(discount_request.date());
                    response.discounts = if payment_time > evaluation_time {
                        curve_model
                            .P_discount(evaluation_time, payment_time, rate_factors[rate_position])
                            .ok()
                    } else {
                        Some(T::one())
                    };
                }
            }

            response.numeraire = Some(domestic_model.numeraire(next_time, rate_factors[0])?);
            if request.path_dependent_request.is_some() {
                response.path_dependent_observations = None;
            }
            responses.push(response);
        }
        Ok(responses)
    }

    fn populate_spot_responses(
        &self,
        scenario: &mut PathScenario<T>,
        fx_history: &[HashMap<Currency, T>],
        equity_history: &[HashMap<String, T>],
    ) {
        for (step, step_responses) in scenario.iter_mut().enumerate() {
            for (local_index, &request_index) in
                self.request_indices_by_step[step].iter().enumerate()
            {
                let Some(spot_request) = &self.model.requests[request_index].spot_request else {
                    continue;
                };
                let index = spot_request.market_index();
                let observation_step = self
                    .model
                    .dates
                    .iter()
                    .rposition(|date| *date <= spot_request.date())
                    .unwrap_or(step)
                    .min(self.model.dates.len() - 1);
                if let Some(currency) = self.model.fx_spot_indices.get(&index) {
                    if let Some(&spot) = fx_history[observation_step].get(currency) {
                        step_responses[local_index].spots = Some(spot);
                    }
                } else if let MarketIndex::Equity(name) = &index {
                    if let Some(&spot) = equity_history[observation_step].get(name) {
                        step_responses[local_index].spots = Some(spot);
                    }
                }
            }
        }
    }

    /// Generate one randomized quasi-Monte Carlo path. Consecutive path
    /// indices share the same Sobol point with opposite normal signs.
    fn generate_path(
        &self,
        sample_index: u32,
        sequence_block: usize,
        antithetic_sign: f64,
    ) -> Result<PathScenario<T>> {
        let n_dates = self.model.dates.len();
        let mut rate_factors = vec![T::zero(); self.rate_indices.len()];
        let mut fx_spots: HashMap<Currency, T> = self
            .fx_currencies
            .iter()
            .map(|currency| {
                let spot = self.model.fx_models[currency].initial_spot();
                (*currency, spot)
            })
            .collect();
        let mut equity_spots: HashMap<String, T> = self
            .equity_names
            .iter()
            .map(|name| (name.clone(), self.model.equity_models[name].initial_spot()))
            .collect();
        let mut fx_history = Vec::with_capacity(n_dates);
        let mut equity_history = Vec::with_capacity(n_dates);
        let mut scenario = Vec::with_capacity(n_dates);

        for step in 0..n_dates {
            let time = self.times[step];
            let next_time = self.times[step + 1];
            let time_step = next_time - time;
            let increments = self.correlated_increments(
                step,
                sample_index,
                sequence_block,
                antithetic_sign,
                time_step.sqrt(),
            );
            self.evolve_market_state(
                time,
                time_step,
                &increments,
                &mut rate_factors,
                &mut fx_spots,
                &mut equity_spots,
            )?;
            fx_history.push(fx_spots.clone());
            equity_history.push(equity_spots.clone());
            scenario.push(self.responses_for_step(step, next_time, &rate_factors, &fx_spots)?);
        }
        self.populate_spot_responses(&mut scenario, &fx_history, &equity_history);
        Ok(scenario)
    }
}

// SAFETY: LgmMarketModel is immutable during simulation — all &dyn
// InterestRatesTermStructure references are used for read-only discount-factor
// lookups.  No interior mutability is involved.
unsafe impl<T: Scalar> Sync for LgmMarketModel<'_, T> {}
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T: Scalar> Send for LgmMarketModel<'_, T> {}

// ---------------------------------------------------------------------------
// MarketModel implementation
// ---------------------------------------------------------------------------
impl<T: Scalar + 'static> MarketModel<T> for LgmMarketModel<'_, T> {
    fn n_paths(&self) -> usize {
        self.n_paths
    }

    fn generate_path(&self, index: usize) -> Option<PathScenario<T>> {
        let ctx = LgmPathContext::new(self);
        let pair_index = index / 2;
        let sample_index = u32::try_from(pair_index % SOBOL_SEQUENCE_LENGTH).ok()?;
        let sequence_block = pair_index / SOBOL_SEQUENCE_LENGTH;
        let antithetic_sign = if index.is_multiple_of(2) { 1.0 } else { -1.0 };
        ctx.generate_path(sample_index, sequence_block, antithetic_sign)
            .ok()
    }

    fn set_evaluation_dates(&mut self, dates: Vec<Date>) {
        self.dates = dates;
        self.rebuild_path_layout();
    }

    fn set_requests(&mut self, requests: Vec<SimulationRequest>) {
        self.request_dates = vec![None; requests.len()];
        self.compact_dated_requests = false;
        self.requests = requests;
        self.rebuild_request_layout();
    }

    fn set_request_dates(&mut self, request_dates: Vec<Option<Date>>) {
        if request_dates.len() == self.requests.len() {
            self.request_dates = request_dates;
            self.compact_dated_requests = true;
            self.rebuild_request_layout();
        }
    }

    fn uses_compact_dated_requests(&self) -> bool {
        self.compact_dated_requests
    }

    fn resolve_discount_request(&self, eval_date: Date, request: &DiscountRequest) -> Result<T> {
        let idx = request.market_index();
        let curve_model = self
            .curve_models
            .get(&idx)
            .ok_or_else(|| QSError::NotFoundErr(format!("Curve model for {idx}")))?;
        let t_eval = self.time_from_date(eval_date);
        let t_pay = self.time_from_date(request.date());
        let z_t = T::scalar(self.state_z(&idx, eval_date).unwrap_or(0.0));
        curve_model.P_discount(t_eval, t_pay, z_t)
    }

    fn resolve_forward_rate_request(
        &self,
        eval_date: Date,
        request: &ForwardRateRequest,
    ) -> Result<T> {
        let idx = request.market_index();
        let curve_model = self
            .curve_models
            .get(&idx)
            .ok_or_else(|| QSError::NotFoundErr(format!("Curve model for {idx}")))?;
        let t_eval = self.time_from_date(eval_date);
        let z_t = T::scalar(self.state_z(&idx, eval_date).unwrap_or(0.0));

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
            Ok(p_s.div_val(p_e).sub_val(T::one()).div_val(T::scalar(tau)))
        }
    }

    fn resolve_fx_request(&self, eval_date: Date, request: &FxRequest) -> Result<T> {
        let base = request.base();
        if base == self.domestic_currency {
            return Ok(T::one());
        }
        self.state_fx(base, eval_date)
            .map(|v| T::scalar(v))
            .ok_or_else(|| QSError::NotFoundErr(format!("FX state for {base} at {eval_date}")))
    }

    fn resolve_spot_request(&self, eval_date: Date, request: &SpotRequest) -> Result<T> {
        let idx = request.market_index();
        self.fx_spot_indices.get(&idx).map_or_else(
            || {
                Err(QSError::NotImplementedErr(
                    "Spot request not supported in LGM rate model".into(),
                ))
            },
            |ccy| {
                self.state_fx(*ccy, eval_date)
                    .map(|v| T::scalar(v))
                    .ok_or_else(|| {
                        QSError::NotFoundErr(format!("FX state for {ccy} at {eval_date}"))
                    })
            },
        )
    }

    fn resolve_path_dependent_request(
        &self,
        _eval_date: Date,
        _request: &PathDependentRequest,
    ) -> Result<T> {
        Err(QSError::NotImplementedErr(
            "Path-dependent request not supported in LGM rate model".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// State accessors (used by resolve methods when state is populated externally)
// ---------------------------------------------------------------------------
impl<T: Scalar> LgmMarketModel<'_, T> {
    fn state_z(&self, index: &MarketIndex, date: Date) -> Option<f64> {
        self.state
            .rates
            .get(self.factor_index(index))?
            .get(&date)
            .copied()
    }

    fn state_fx(&self, currency: Currency, date: Date) -> Option<f64> {
        self.state.fx.get(&currency)?.get(&date).copied()
    }
}

#[cfg(test)]
mod sampling_tests {
    use super::*;
    use crate::{
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::date::Date,
    };

    #[test]
    fn sobol_normals_are_repeatable_antithetic_pairs() {
        for dimension in 0..32 {
            let positive = sobol_normal(17, dimension, 0, 42, 1.0);
            let repeated = sobol_normal(17, dimension, 0, 42, 1.0);
            let antithetic = sobol_normal(17, dimension, 0, 42, -1.0);
            assert!(positive.is_finite());
            assert_eq!(positive, repeated);
            assert!((positive + antithetic).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn factor_order_is_stable_and_sorted_by_currency() {
        let reference_date = Date::new(2026, 9, 9);
        let curve = FlatForwardTermStructure::new(reference_date, 0.03, RateDefinition::default());
        let domestic_fx_rate = LgmRateModel::new(0.03, 0.01, &curve);
        let eur_fx_rate = LgmRateModel::new(0.03, 0.01, &curve);
        let jpy_fx_rate = LgmRateModel::new(0.03, 0.01, &curve);
        let eur_fx = LgmFxModel::new(&domestic_fx_rate, &eur_fx_rate, 0.10, 1.15, 0.0);
        let jpy_fx = LgmFxModel::new(&domestic_fx_rate, &jpy_fx_rate, 0.10, 0.007, 0.0);

        let mut model = LgmMarketModel::new(
            Currency::USD,
            MarketIndex::SOFR,
            reference_date,
            DayCounter::Actual365,
        );
        model.add_curve_model(MarketIndex::SOFR, LgmRateModel::new(0.03, 0.01, &curve));
        model.add_curve_model(MarketIndex::TONAR, LgmRateModel::new(0.03, 0.01, &curve));
        model.add_curve_model(MarketIndex::ESTR, LgmRateModel::new(0.03, 0.01, &curve));
        model.add_fx_model(Currency::JPY, jpy_fx);
        model.add_fx_model(Currency::EUR, eur_fx);

        let (rate_indices, fx_currencies, _) = model.build_factor_ordering();
        assert_eq!(fx_currencies, vec![Currency::EUR, Currency::JPY]);
        assert_eq!(
            rate_indices,
            vec![MarketIndex::SOFR, MarketIndex::ESTR, MarketIndex::TONAR]
        );
    }
}
