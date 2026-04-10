use std::collections::HashMap;

use rayon::iter::ParallelBridge;
use rayon::iter::ParallelIterator;

use crate::{
    ad::scalar::Scalar,
    time::date::Date,
    xva::{
        contigentclaim::ContingentClaim,
        visitors::marketgenerator::{MarketModel, SimulationResponse},
    },
};

pub struct ExposureEvaluation {
    identifier: String,
    dates: Vec<Date>,
    epe: Vec<f64>,
    ene: Vec<f64>,
    ee: Vec<f64>,
}

impl ExposureEvaluation {
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }
    pub fn epe(&self) -> &[f64] {
        &self.epe
    }
    pub fn ene(&self) -> &[f64] {
        &self.ene
    }
    pub fn ee(&self) -> &[f64] {
        &self.ee
    }
}

/// Per-trade accumulator across Monte Carlo paths.
struct TradeAccumulator {
    n_dates: usize,
    epe_sum: Vec<f64>,
    ene_sum: Vec<f64>,
    ee_sum: Vec<f64>,
    n_paths: usize,
}

impl TradeAccumulator {
    fn new(n_dates: usize) -> Self {
        Self {
            n_dates,
            epe_sum: vec![0.0; n_dates],
            ene_sum: vec![0.0; n_dates],
            ee_sum: vec![0.0; n_dates],
            n_paths: 0,
        }
    }

    fn add_path(&mut self, npvs: &[f64]) {
        for (d, &npv) in npvs.iter().enumerate() {
            self.ee_sum[d] += npv;
            self.epe_sum[d] += npv.max(0.0);
            self.ene_sum[d] += npv.min(0.0);
        }
        self.n_paths += 1;
    }

    fn merge(&mut self, other: &Self) {
        for d in 0..self.n_dates {
            self.ee_sum[d] += other.ee_sum[d];
            self.epe_sum[d] += other.epe_sum[d];
            self.ene_sum[d] += other.ene_sum[d];
        }
        self.n_paths += other.n_paths;
    }

    fn into_evaluation(self, identifier: String, dates: Vec<Date>) -> ExposureEvaluation {
        let n = self.n_paths.max(1) as f64;
        ExposureEvaluation {
            identifier,
            dates,
            epe: self.epe_sum.iter().map(|s| s / n).collect(),
            ene: self.ene_sum.iter().map(|s| s / n).collect(),
            ee: self.ee_sum.iter().map(|s| s / n).collect(),
        }
    }
}

/// Converts a `SimulationResponse<T>` to `SimulationResponse<f64>` by extracting
/// the underlying value from each scalar field.
fn response_to_f64<T: Scalar + 'static>(
    response: &SimulationResponse<T>,
) -> SimulationResponse<f64> {
    SimulationResponse {
        discounts: response.discounts.map(|v| v.value()),
        forward_rates: response.forward_rates.map(|v| v.value()),
        fx_rates: response.fx_rates.map(|v| v.value()),
        spots: response.spots.map(|v| v.value()),
        path_dependent_observations: response.path_dependent_observations.map(|v| v.value()),
    }
}

pub struct ExposureEvaluator<'a, T>
where
    T: Scalar,
{
    dates: Vec<Date>,
    model: &'a dyn MarketModel<T>,
}

impl<'a, T> ExposureEvaluator<'a, T>
where
    T: Scalar + 'static,
{
    pub fn new(dates: Vec<Date>, model: &'a dyn MarketModel<T>) -> Self {
        Self { dates, model }
    }

    pub fn evaluate(
        &self,
        trades: &HashMap<String, &[ContingentClaim]>,
    ) -> Vec<ExposureEvaluation> {
        let n_dates = self.dates.len();

        // Parallel fold + reduce over lazily-generated paths.
        let accumulators: HashMap<String, TradeAccumulator> = self
            .model
            .path_iter()
            .par_bridge()
            .fold(
                || -> HashMap<String, TradeAccumulator> {
                    trades
                        .keys()
                        .map(|id| (id.clone(), TradeAccumulator::new(n_dates)))
                        .collect()
                },
                |mut acc, scenario| {
                    // scenario: Vec<Vec<SimulationResponse<T>>>
                    //           dates x claims
                    for (trade_id, claims) in trades {
                        let mut npvs = vec![0.0_f64; n_dates];
                        for (d, date_responses) in scenario.iter().enumerate() {
                            for claim in *claims {
                                if let Some(idx) = claim.idx() {
                                    if idx < date_responses.len() {
                                        let resp_f64 = response_to_f64(&date_responses[idx]);
                                        if let Ok(v) = claim.evaluate_f64(&resp_f64) {
                                            npvs[d] += v;
                                        }
                                    }
                                }
                            }
                        }
                        acc.get_mut(trade_id).unwrap().add_path(&npvs);
                    }
                    acc
                },
            )
            .reduce(
                || {
                    trades
                        .keys()
                        .map(|id| (id.clone(), TradeAccumulator::new(n_dates)))
                        .collect()
                },
                |mut a, b| {
                    for (id, other) in &b {
                        a.get_mut(id).unwrap().merge(other);
                    }
                    a
                },
            );

        // Convert accumulators into final ExposureEvaluation results.
        let dates = &self.dates;
        accumulators
            .into_iter()
            .map(|(trade_id, acc)| acc.into_evaluation(trade_id, dates.clone()))
            .collect()
    }
}
