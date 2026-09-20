mod utils;

use quantsupport::prelude::*;
use std::path::PathBuf;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let data_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");

    // 1. Load market data from JSON
    let quote_store = utils::load_quotes(&data_dir.join("quotes.json"))?;
    let curve_specs = utils::load_curve_specs(&data_dir.join("curve_specs.json"))?;
    let vol_specs = utils::load_vol_specs(&data_dir.join("vol_specs.json"))?;
    let sim_config = utils::load_simulation_config(&data_dir.join("simulation.json"))?;
    let ref_date = quote_store.reference_date();
    let dc = DayCounter::Actual365;

    println!("Reference date: {ref_date}");
    println!(
        "Loaded {} quotes, {} curve spec(s), {} volatility surface spec(s)",
        quote_store.quotes().len(),
        curve_specs.len(),
        vol_specs.volatility_surfaces.len(),
    );

    // 2. Bootstrap SOFR discount curve
    let csa_index = curve_specs[0].market_index().clone();
    let policy = BootstrapDiscountPolicy::new(csa_index, Currency::USD);
    let bootstrapper = MultiCurveBootstrapper::new(curve_specs, policy);
    let curves = bootstrapper.bootstrap(&quote_store, Level::Mid)?;

    let sofr_element = curves.get(&MarketIndex::SOFR).expect("SOFR curve");
    let curve = sofr_element.to_f64_term_structure(dc)?;
    println!("Bootstrapped SOFR curve ({} nodes)", curve.dates().len());

    // 3. Build the SOFR caplet vol surface. This is the only configuration
    // that owns the quote identifiers. Model calibration selects its
    // calibration basket
    // from the constructed surface.
    let surfaces = VolatilitySurfaceBuilder::new(vol_specs.volatility_surfaces)
        .build(&quote_store, Level::Mid)?;
    println!("Built {} volatility surface(s)", surfaces.len());

    // Constructed element store shared by calibration and simulation.
    let mut store = ConstructedElementStore::default();
    for (index, element) in &curves {
        store
            .discount_curves_mut()
            .insert(index.clone(), element.clone());
    }
    for (index, element) in surfaces {
        store.volatility_surfaces_mut().insert(index, element);
    }

    // 4. Resolve the model definition embedded in the simulation config and
    // calibrate HW to caplet vols interpolated from its referenced surface.
    let ModelConfiguration::HullWhite {
        alpha,
        parameter_source,
    } = sim_config.model()
    else {
        return Err("Hull-White example requires a HullWhite model configuration".into());
    };
    let ParameterSource::Calibrated(hw_config) = parameter_source else {
        return Err("Hull-White example requires calibrated model parameters".into());
    };

    let mut hw = HullWhite::new(*alpha, &curve);

    hw.calibrate_with_configuration(&hw_config, &store, &quote_store, &curve, Level::Mid)
        .expect("calibration should converge");

    let quality = hw
        .calibration_quality()
        .expect("calibration quality should be set after calibrate");
    println!(
        "Selected {} surface node(s), calibrated {} instrument pillar(s)",
        hw_config.resolve_instrument_ids(&store)?.len(),
        quality.records.len()
    );

    // 6. Print calibration quality table
    println!("\n=== Calibration Quality ===");
    println!(
        "{:<8} {:>8} {:>10} {:>12} {:>14} {:>14} {:>10}",
        "Expiry", "t", "Mkt Vol", "Model Vol", "Mkt Price", "Model Price", "Error"
    );
    println!("{:-<82}", "");
    for rec in &quality.records {
        let model_vol = utils::implied_black_vol(rec, &curve)?;
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

    // 8. Build the Monte Carlo simulation from the JSON configuration
    let builder = SimulationBuilder::new(vec![sim_config]);
    let simulations = builder.build(&store, &quote_store, &FixingStore::default(), Level::Mid)?;
    let simulation_element = simulations
        .get(&MarketIndex::SOFR)
        .expect("SOFR simulation");
    let simulation = simulation_element.simulation().borrow();

    let times: Vec<f64> = simulation
        .dates()
        .iter()
        .map(|d| dc.year_fraction(ref_date, *d))
        .collect();
    let all_paths: Vec<Vec<f64>> = simulation
        .path()
        .iter()
        .map(|path| path.iter().map(|v| v.value()).collect())
        .collect();
    println!(
        "\nSimulated {} paths over {} monthly steps from JSON config",
        all_paths.len(),
        times.len()
    );

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
    utils::plot_calibration_quality(&quality, &curve, ref_date, dc)?;

    println!("\nPlots saved: hw_simulations.png, hw_calibration.png");

    Ok(())
}
