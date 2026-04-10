mod utils;

use quantsupport::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::path::PathBuf;

/// Box-Muller standard normal sample.
fn std_normal(rng: &mut impl Rng) -> f64 {
    let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
    let u2: f64 = rng.gen_range(0.0..std::f64::consts::TAU);
    (-2.0 * u1.ln()).sqrt() * u2.cos()
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let data_dir = PathBuf::from("data");

    // 1. Load market data from JSON
    let quote_store = utils::load_quotes(&data_dir.join("quotes.json"))?;
    let curve_specs = utils::load_curve_specs(&data_dir.join("curve_specs.json"))?;
    let hw_config = utils::load_hw_calibration(&data_dir.join("hw_calibration.json"))?;
    let ref_date = quote_store.reference_date();
    let dc = DayCounter::Actual365;

    println!("Reference date: {ref_date}");
    println!(
        "Loaded {} quotes, {} curve spec(s), {} calibration quote(s)",
        quote_store.quotes().len(),
        curve_specs.len(),
        hw_config.quote_ids().len(),
    );

    // 2. Bootstrap SOFR discount curve
    let csa_index = curve_specs[0].market_index().clone();
    let policy = BootstrapDiscountPolicy::new(csa_index, Currency::USD);
    let bootstrapper = MultiCurveBootstrapper::new(curve_specs, policy);
    let curves = bootstrapper.bootstrap(&quote_store, Level::Mid)?;

    let sofr_element = curves.get(&MarketIndex::SOFR).expect("SOFR curve");
    let curve = utils::extract_f64_curve(sofr_element, dc)?;
    println!("Bootstrapped SOFR curve ({} nodes)", curve.dates().len());

    // 3. HW calibration from configuration
    let alpha = hw_config.alpha();
    let mut hw = HullWhite::new(alpha, &curve);

    // 4. Calibrate HW to market caplet vols
    hw.calibrate(hw_config.quote_ids(), &quote_store, &curve, Level::Mid)
        .expect("calibration should converge");

    let quality = hw
        .calibration_quality()
        .expect("calibration quality should be set after calibrate");

    // 6. Print calibration quality table
    println!("\n=== Calibration Quality ===");
    println!(
        "{:<8} {:>8} {:>10} {:>12} {:>14} {:>14} {:>10}",
        "Expiry", "t", "Mkt Vol", "Model Vol", "Mkt Price", "Model Price", "Error"
    );
    println!("{:-<82}", "");
    for rec in &quality.records {
        let model_vol = hw.zcb_price_volatility(rec.calibrated_sigma, rec.t, rec.big_t);
        let err = (rec.model_price - rec.market_price).abs();
        println!(
            "{:<8} {:>8.4} {:>10.6} {:>12.6} {:>14.8} {:>14.8} {:>10.2e}",
            rec.expiry, rec.t, rec.market_vol, model_vol, rec.market_price, rec.model_price, err,
        );
    }

    // 7. ATM cap prices table
    println!("\n=== ATM Cap Prices ===");
    println!(
        "{:<10} {:>8} {:>14} {:>14}",
        "Cap End", "t_end", "Caplet Price", "Cumul Cap"
    );
    println!("{:-<52}", "");

    let mut cumul_cap = 0.0;
    for rec in &quality.records {
        cumul_cap += rec.model_price;
        println!(
            "{:<10} {:>8.4} {:>14.8} {:>14.8}",
            rec.expiry, rec.big_t, rec.model_price, cumul_cap,
        );
    }

    // 8. Simulate 100 paths

    let n_paths = 100_usize;
    let t_end = 10.0;
    let n_steps = 120;

    let mut times = Vec::with_capacity(n_steps);
    for i in 1..=n_steps {
        times.push(t_end * i as f64 / n_steps as f64);
    }

    let mut rng = StdRng::seed_from_u64(42);
    let mut all_paths: Vec<Vec<f64>> = Vec::with_capacity(n_paths);

    for _ in 0..n_paths {
        let mut draws = vec![0.0_f64; n_steps];
        let mut scenario = vec![0.0_f64; n_steps];
        for d in &mut draws {
            *d = std_normal(&mut rng);
        }
        hw.generate(&times, &draws, &mut scenario).unwrap();
        all_paths.push(scenario);
    }

    // 9. Plot simulations
    let r0 = curve
        .forward_rate(
            ref_date,
            ref_date.advance(1, TimeUnit::Days),
            Compounding::Continuous,
            Frequency::Annual,
        )
        .unwrap();

    utils::plot_simulations(&times, &all_paths, r0)?;

    // 10. Plot calibration quality
    utils::plot_calibration_quality(&quality, &hw, &curve, ref_date, dc)?;

    println!("\nPlots saved: hw_simulations.png, hw_calibration.png");

    Ok(())
}
