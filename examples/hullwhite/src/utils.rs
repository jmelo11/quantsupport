use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use plotters::prelude::*;
use quantsupport::prelude::*;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// JSON deserialization helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct QuoteRecord {
    identifier: String,
    mid: f64,
}

#[derive(Deserialize)]
struct JsonQuotes {
    reference_date: Date,
    quotes: Vec<QuoteRecord>,
}

#[derive(Deserialize)]
struct JsonCurveSpecs {
    curve_specs: Vec<CurveConfiguration>,
}

/// Loads quotes from a JSON file into a [`QuoteStore`].
pub fn load_quotes(path: &PathBuf) -> Result<QuoteStore> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonQuotes =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;

    let mut store = QuoteStore::new(json.reference_date);
    for rec in json.quotes {
        let details = QuoteDetails::from_str(&rec.identifier)?;
        let levels = QuoteLevels::with_mid(rec.mid);
        store.add_quote(Quote::new(details, levels));
    }
    Ok(store)
}

/// Loads curve specifications from a JSON file.
pub fn load_curve_specs(path: &PathBuf) -> Result<Vec<CurveConfiguration>> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    let json: JsonCurveSpecs =
        serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))?;
    Ok(json.curve_specs)
}

/// Loads a Hull-White calibration configuration from a JSON file.
pub fn load_hw_calibration(path: &PathBuf) -> Result<ModelCalibrationConfiguration> {
    let file =
        File::open(path).map_err(|e| QSError::NotFoundErr(format!("{}: {e}", path.display())))?;
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(|e| QSError::InvalidValueErr(e.to_string()))
}

// ---------------------------------------------------------------------------
// Curve / surface construction helpers
// ---------------------------------------------------------------------------

/// Extracts an `f64` discount term structure from a bootstrapped
/// `DiscountCurveElement` (which internally holds `DualFwd` values).
pub fn extract_f64_curve(
    element: &DiscountCurveElement,
    day_counter: DayCounter,
) -> Result<DiscountTermStructure<f64>> {
    let curve = element.curve();
    let nodes = curve
        .nodes()
        .ok_or_else(|| QSError::InvalidValueErr("Bootstrapped curve has no nodes".into()))?;
    let (dates, dfs): (Vec<Date>, Vec<f64>) =
        nodes.into_iter().map(|(d, df)| (d, f64::from(df))).unzip();
    DiscountTermStructure::<f64>::new(dates, dfs, day_counter, Interpolator::LogLinear, true)
}

// ---------------------------------------------------------------------------
// Plotting
// ---------------------------------------------------------------------------

pub fn plot_simulations(
    times: &[f64],
    paths: &[Vec<f64>],
    r0: f64,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("hw_simulations.png", (900, 500)).into_drawing_area();
    root.fill(&WHITE)?;

    let y_min = paths
        .iter()
        .flat_map(|p| p.iter())
        .cloned()
        .fold(r0, f64::min)
        - 0.005;
    let y_max = paths
        .iter()
        .flat_map(|p| p.iter())
        .cloned()
        .fold(r0, f64::max)
        + 0.005;

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Hull-White Short Rate — 100 Simulations",
            ("sans-serif", 22),
        )
        .margin(10)
        .x_label_area_size(35)
        .y_label_area_size(55)
        .build_cartesian_2d(0.0..times[times.len() - 1], y_min..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Time (years)")
        .y_desc("Short rate r(t)")
        .draw()?;

    let colors = [
        RGBColor(31, 119, 180),
        RGBColor(255, 127, 14),
        RGBColor(44, 160, 44),
        RGBColor(214, 39, 40),
        RGBColor(148, 103, 189),
        RGBColor(140, 86, 75),
        RGBColor(227, 119, 194),
        RGBColor(127, 127, 127),
        RGBColor(188, 189, 34),
        RGBColor(23, 190, 207),
    ];

    let n = paths.len();
    for (i, path) in paths.iter().enumerate() {
        let color = colors[i % colors.len()].mix(0.4 + 0.6 * (i as f64 / n as f64));
        chart.draw_series(LineSeries::new(
            std::iter::once((0.0, r0)).chain(times.iter().zip(path.iter()).map(|(&t, &r)| (t, r))),
            color.stroke_width(1),
        ))?;
    }

    root.present()?;
    Ok(())
}

pub fn plot_calibration_quality(
    quality: &HullWhiteCalibrationQuality,
    hw: &HullWhite<f64>,
    curve: &DiscountTermStructure<f64>,
    ref_date: Date,
    dc: DayCounter,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("hw_calibration.png", (1200, 500)).into_drawing_area();
    root.fill(&WHITE)?;
    let (left, right) = root.split_horizontally(600);

    // ── Left panel: market vol vs model-implied Black vol ───────
    {
        let ts: Vec<f64> = quality.records.iter().map(|r| r.t).collect();
        let mkt_vols: Vec<f64> = quality.records.iter().map(|r| r.market_vol).collect();
        let model_vols: Vec<f64> = quality
            .records
            .iter()
            .map(|r| hw.zcb_price_volatility(r.calibrated_sigma, r.t, r.big_t))
            .collect();

        let y_min = mkt_vols
            .iter()
            .chain(model_vols.iter())
            .cloned()
            .fold(f64::MAX, f64::min)
            * 0.9;
        let y_max = mkt_vols
            .iter()
            .chain(model_vols.iter())
            .cloned()
            .fold(f64::MIN, f64::max)
            * 1.1;
        let x_max = *ts.last().unwrap() * 1.1;

        let mut chart = ChartBuilder::on(&left)
            .caption("Caplet Implied Vol: Market vs Model", ("sans-serif", 18))
            .margin(10)
            .x_label_area_size(35)
            .y_label_area_size(55)
            .build_cartesian_2d(0.0..x_max, y_min..y_max)?;

        chart
            .configure_mesh()
            .x_desc("Expiry (years)")
            .y_desc("Black Vol")
            .draw()?;

        chart
            .draw_series(LineSeries::new(
                ts.iter().zip(mkt_vols.iter()).map(|(&t, &v)| (t, v)),
                BLUE.stroke_width(2),
            ))?
            .label("Market")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 15, y)], BLUE.stroke_width(2)));

        chart
            .draw_series(
                ts.iter()
                    .zip(model_vols.iter())
                    .map(|(&t, &v)| Circle::new((t, v), 4, RED.filled())),
            )?
            .label("Model")
            .legend(|(x, y)| Circle::new((x + 7, y), 4, RED.filled()));

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .border_style(BLACK)
            .draw()?;
    }

    // ── Right panel: bootstrapped zero rates vs pillar quotes ───
    {
        let n_pts = 120;
        let t_max = 32.0;
        let ts: Vec<f64> = (1..=n_pts)
            .map(|i| t_max * i as f64 / n_pts as f64)
            .collect();
        let zeros: Vec<f64> = ts
            .iter()
            .map(|&t| {
                let df = curve.discount_factor_from_time(t).unwrap();
                -df.ln() / t
            })
            .collect();

        let y_min = zeros.iter().cloned().fold(f64::MAX, f64::min) * 0.95;
        let y_max = zeros.iter().cloned().fold(f64::MIN, f64::max) * 1.05;

        let mut chart = ChartBuilder::on(&right)
            .caption("Bootstrapped SOFR Zero Curve", ("sans-serif", 18))
            .margin(10)
            .x_label_area_size(35)
            .y_label_area_size(55)
            .build_cartesian_2d(0.0..t_max, y_min..y_max)?;

        chart
            .configure_mesh()
            .x_desc("Maturity (years)")
            .y_desc("Zero Rate")
            .draw()?;

        chart
            .draw_series(LineSeries::new(
                ts.iter().zip(zeros.iter()).map(|(&t, &z)| (t, z)),
                GREEN.stroke_width(2),
            ))?
            .label("Continuous Zero")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 15, y)], GREEN.stroke_width(2)));

        // Overlay pillar DF-implied zero rates
        let pillar_tenors = ["1D", "1Y", "2Y", "3Y", "5Y", "7Y", "10Y", "30Y"];
        let mut pillar_pts = Vec::new();
        for tenor_str in &pillar_tenors {
            let d = ref_date + Period::from_str(tenor_str).unwrap();
            let t = dc.year_fraction(ref_date, d);
            if t > 0.0 {
                let df = curve.discount_factor(d).unwrap();
                let z = -df.ln() / t;
                pillar_pts.push((t, z));
            }
        }

        chart
            .draw_series(
                pillar_pts
                    .iter()
                    .map(|&(t, z)| Circle::new((t, z), 5, RED.filled())),
            )?
            .label("Pillar")
            .legend(|(x, y)| Circle::new((x + 7, y), 5, RED.filled()));

        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .border_style(BLACK)
            .draw()?;
    }

    root.present()?;
    Ok(())
}
