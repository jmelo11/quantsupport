//! Monte Carlo exposure evaluator.
//!
//! [`ExposureEvaluator`] takes a [`MarketModel`] and a set of trades
//! (each represented as a slice of [`ContingentClaim`]s) and computes
//! per-trade exposure profiles — EPE, ENE, and EE — by averaging over
//! simulated paths using parallel fold + reduce.

use std::collections::HashMap;

use rayon::iter::ParallelBridge;
use rayon::iter::ParallelIterator;

use crate::{
    ad::scalar::Scalar,
    time::date::Date,
    xva::{
        contigentclaim::ContingentClaim,
        visitors::marketmodel::{MarketModel, PathScenario},
    },
};

/// Per-trade exposure profile computed by [`ExposureEvaluator`].
///
/// Contains the expected positive exposure (EPE), expected negative
/// exposure (ENE), and expected exposure (EE) at each simulation date.
pub struct ExposureEvaluation<T: Scalar> {
    identifier: String,
    dates: Vec<Date>,
    epe: Vec<T>,
    ene: Vec<T>,
    ee: Vec<T>,
}

impl<T: Scalar> ExposureEvaluation<T> {
    /// Returns the trade identifier.
    #[must_use] 
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
    /// Returns the simulation dates.
    #[must_use] 
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }
    /// Returns the expected positive exposure at each date: `E[max(V, 0)]`.
    #[must_use] 
    pub fn epe(&self) -> &[T] {
        &self.epe
    }
    /// Returns the expected negative exposure at each date: `E[min(V, 0)]`.
    #[must_use] 
    pub fn ene(&self) -> &[T] {
        &self.ene
    }
    /// Returns the expected exposure at each date: `E[V]`.
    #[must_use] 
    pub fn ee(&self) -> &[T] {
        &self.ee
    }
}

struct TradeAccumulator<T: Scalar> {
    n_dates: usize,
    epe_sum: Vec<T>,
    ene_sum: Vec<T>,
    ee_sum: Vec<T>,
    n_paths: usize,
}

impl<T: Scalar> TradeAccumulator<T> {
    fn new(n_dates: usize) -> Self {
        Self {
            n_dates,
            epe_sum: vec![T::zero(); n_dates],
            ene_sum: vec![T::zero(); n_dates],
            ee_sum: vec![T::zero(); n_dates],
            n_paths: 0,
        }
    }

    fn add_path(&mut self, npvs: &[T]) {
        let zero = T::zero();
        for (d, &npv) in npvs.iter().enumerate() {
            self.ee_sum[d] = self.ee_sum[d].add_val(npv);
            self.epe_sum[d] = self.epe_sum[d].add_val(npv.max_val(zero));
            self.ene_sum[d] = self.ene_sum[d].add_val(npv.min_val(zero));
        }
        self.n_paths += 1;
    }

    fn merge(&mut self, other: &Self) {
        for d in 0..self.n_dates {
            self.ee_sum[d] = self.ee_sum[d].add_val(other.ee_sum[d]);
            self.epe_sum[d] = self.epe_sum[d].add_val(other.epe_sum[d]);
            self.ene_sum[d] = self.ene_sum[d].add_val(other.ene_sum[d]);
        }
        self.n_paths += other.n_paths;
    }

    fn into_evaluation(self, identifier: String, dates: Vec<Date>) -> ExposureEvaluation<T> {
        #[allow(clippy::cast_precision_loss)]
        let n = T::scalar(self.n_paths.max(1) as f64);
        ExposureEvaluation {
            identifier,
            dates,
            epe: self.epe_sum.into_iter().map(|s| s.div_val(n)).collect(),
            ene: self.ene_sum.into_iter().map(|s| s.div_val(n)).collect(),
            ee: self.ee_sum.into_iter().map(|s| s.div_val(n)).collect(),
        }
    }
}

/// Computes per-trade exposure profiles over Monte Carlo paths.
///
/// The evaluator iterates (in parallel) over the paths produced by a
/// [`MarketModel`], evaluates every [`ContingentClaim`] at each
/// simulation date, and accumulates EPE / ENE / EE statistics.
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
    /// Creates a new evaluator for the given simulation dates and market model.
    pub fn new(dates: Vec<Date>, model: &'a dyn MarketModel<T>) -> Self {
        Self { dates, model }
    }

    /// Runs the Monte Carlo evaluation and returns one [`ExposureEvaluation`]
    /// per trade.
    ///
    /// `trades` maps trade identifiers to their contingent-claim slices.
    /// The claims must have been previously visited by an [`Inspector`](super::inspector::Inspector)
    /// so that their flat-vector indices are set.
    #[must_use] 
    pub fn evaluate(
        &self,
        trades: &HashMap<String, &[ContingentClaim]>,
    ) -> Vec<ExposureEvaluation<T>> {
        let n_dates = self.dates.len();
        let dates = self.dates.clone();

        // Parallel fold + reduce over lazily-generated paths.
        let accumulators = self
            .model
            .path_iter()
            .par_bridge()
            .fold(
                || -> HashMap<String, TradeAccumulator<T>> {
                    trades
                        .keys()
                        .map(|id| (id.clone(), TradeAccumulator::new(n_dates)))
                        .collect()
                },
                |mut acc, scenario| {
                    Self::process_scenario(&mut acc, &scenario, trades, n_dates, &dates);
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
                        if let Some(entry) = a.get_mut(id) {
                            entry.merge(other);
                        }
                    }
                    a
                },
            );

        // Convert accumulators into final ExposureEvaluation results.
        accumulators
            .into_iter()
            .map(|(trade_id, acc)| acc.into_evaluation(trade_id, self.dates.clone()))
            .collect()
    }

    /// Evaluates each contingent claim in a given path.
    fn process_scenario(
        acc: &mut HashMap<String, TradeAccumulator<T>>,
        scenario: &PathScenario<T>,
        trades: &HashMap<String, &[ContingentClaim]>,
        n_dates: usize,
        dates: &[Date],
    ) {
        for (trade_id, claims) in trades {
            let mut npvs = vec![T::zero(); n_dates];
            for (d, date_responses) in scenario.iter().enumerate() {
                let eval_date = dates[d];
                for claim in *claims {
                    // Skip claims whose payment has already occurred.
                    if claim.payment_date() <= eval_date {
                        continue;
                    }
                    if let Some(idx) = claim.idx() {
                        if idx < date_responses.len() {
                            if let Ok(v) = claim.evaluate(&date_responses[idx]) {
                                npvs[d] = npvs[d].add_val(v);
                            }
                        }
                    }
                }
            }
            if let Some(entry) = acc.get_mut(trade_id) {
                entry.add_path(&npvs);
            }
        }
    }
}

#[cfg(feature = "plot")]
impl crate::utils::plot::Plot for ExposureEvaluation<f64> {
    #[allow(clippy::too_many_lines)]
    fn plot(&self, path: &str) -> crate::utils::errors::Result<()> {
        use crate::time::daycounter::DayCounter;
        use crate::utils::errors::QSError;
        use crate::utils::plot::plotting::{IntoDrawingArea, BitMapBackend, BG_COLOR, ChartBuilder, IntoFont, BLACK, GRID_COLOR, Color, LineSeries, ZERO_LINE_COLOR, AreaSeries, EPE_COLOR, PathElement, ENE_COLOR, EE_COLOR, SeriesLabelPosition};

        let dc = DayCounter::Actual365;
        let ref_date = self.dates[0];

        // Find the last date with non-zero exposure, then include one more
        // point so the plot shows the drop to zero at expiry.
        let last_active = self
            .epe()
            .iter()
            .zip(self.ene().iter())
            .zip(self.ee().iter())
            .rposition(|((e, n), ee)| e.abs() > 1e-12 || n.abs() > 1e-12 || ee.abs() > 1e-12)
            .map_or(self.dates.len(), |i| (i + 2).min(self.dates.len()))
            .min(self.dates.len());

        let dates = &self.dates[..last_active];
        let times: Vec<f64> = dates
            .iter()
            .map(|d| dc.year_fraction(ref_date, *d))
            .collect();
        let epe = &self.epe()[..last_active];
        let ene = &self.ene()[..last_active];
        let ee = &self.ee()[..last_active];

        let all_vals: Vec<f64> = epe
            .iter()
            .chain(ene.iter())
            .chain(ee.iter())
            .copied()
            .collect();
        let y_min_raw = all_vals.iter().copied().fold(f64::MAX, f64::min);
        let y_max_raw = all_vals.iter().copied().fold(f64::MIN, f64::max);

        // Add 10% padding, ensure range is never degenerate
        let margin = ((y_max_raw - y_min_raw).abs() * 0.10).max(1.0);
        let y_min = y_min_raw - margin;
        let y_max = y_max_raw + margin;
        let t_max = times.last().copied().unwrap_or(1.0);

        let root = BitMapBackend::new(path, (1024, 576)).into_drawing_area();
        root.fill(&BG_COLOR)
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        let mut chart = ChartBuilder::on(&root)
            .caption(
                format!("Exposure Profile — {}", self.identifier()),
                ("sans-serif", 24).into_font().color(&BLACK),
            )
            .margin(20)
            .x_label_area_size(45)
            .y_label_area_size(75)
            .build_cartesian_2d(0.0..t_max, y_min..y_max)
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        chart
            .configure_mesh()
            .x_desc("Time (years)")
            .y_desc("Exposure")
            .axis_desc_style(("sans-serif", 16))
            .label_style(("sans-serif", 13))
            .light_line_style(GRID_COLOR)
            .bold_line_style(GRID_COLOR.mix(0.6))
            .draw()
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        // y = 0 reference line
        chart
            .draw_series(LineSeries::new(
                [(0.0, 0.0), (t_max, 0.0)],
                ZERO_LINE_COLOR.stroke_width(1),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        // EPE (filled area + line)
        chart
            .draw_series(AreaSeries::new(
                times.iter().zip(epe.iter()).map(|(&t, &v)| (t, v)),
                0.0,
                EPE_COLOR.mix(0.15),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;
        chart
            .draw_series(LineSeries::new(
                times.iter().zip(epe.iter()).map(|(&t, &v)| (t, v)),
                EPE_COLOR.stroke_width(2),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?
            .label("EPE")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 18, y)], EPE_COLOR.stroke_width(2))
            });

        // ENE (filled area + line)
        chart
            .draw_series(AreaSeries::new(
                times.iter().zip(ene.iter()).map(|(&t, &v)| (t, v)),
                0.0,
                ENE_COLOR.mix(0.15),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;
        chart
            .draw_series(LineSeries::new(
                times.iter().zip(ene.iter()).map(|(&t, &v)| (t, v)),
                ENE_COLOR.stroke_width(2),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?
            .label("ENE")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 18, y)], ENE_COLOR.stroke_width(2))
            });

        // EE (dashed-style thinner line)
        chart
            .draw_series(LineSeries::new(
                times.iter().zip(ee.iter()).map(|(&t, &v)| (t, v)),
                EE_COLOR.stroke_width(1),
            ))
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?
            .label("EE")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 18, y)], EE_COLOR.stroke_width(1))
            });

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .margin(10)
            .background_style(BG_COLOR.mix(0.9))
            .border_style(GRID_COLOR)
            .label_font(("sans-serif", 14))
            .draw()
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;

        root.present()
            .map_err(|e| QSError::EvaluationErr(e.to_string()))?;
        Ok(())
    }
}
