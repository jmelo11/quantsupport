mod utils;

use quantsupport::prelude::*;
use std::path::PathBuf;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // 1. Load quotes from JSON.
    let path = PathBuf::from("data/quotes.json");
    let quote_store = utils::load_quotes(&path)?;

    // 2. Collect all quote identifiers for the surface configuration.
    let quote_ids: Vec<String> = quote_store
        .quotes()
        .keys()
        .cloned()
        .collect();

    println!("Loaded {} caplet vol quotes", quote_ids.len());

    // 3. Build a VolatilitySurfaceConfiguration for SOFR caplet Black vols.
    let config = VolatilitySurfaceConfiguration::new(
        MarketIndex::SOFR,
        VolatilityType::Black,
        SmileType::Strike,
        quote_ids,
    );

    // 4. Use the builder to construct the surface.
    let builder = VolatilitySurfaceBuilder::new(vec![config]);
    let surfaces = builder.build(&quote_store, Level::Mid)?;
    let element = surfaces
        .get(&MarketIndex::SOFR)
        .expect("SOFR surface should exist");

    let surface_ref = element.surface();

    // 5. Query a few interior points and print.
    println!("\n{:<12} {:>8} {:>12}", "Expiry", "Strike", "Black Vol");
    println!("{:-<12} {:->8} {:->12}", "", "", "");

    let queries = [
        (Period::new(6, TimeUnit::Months), 0.030),
        (Period::new(1, TimeUnit::Years), 0.035),
        (Period::new(2, TimeUnit::Years), 0.040),
        (Period::new(3, TimeUnit::Years), 0.035),
        (Period::new(5, TimeUnit::Years), 0.030),
        (Period::new(7, TimeUnit::Years), 0.050),
        // interpolated mid-points
        (Period::new(9, TimeUnit::Months), 0.0325),
        (Period::new(4, TimeUnit::Years), 0.045),
    ];

    for (expiry, strike) in &queries {
        let vol = surface_ref.volatility_from_period(*expiry, *strike)?;
        println!("{:<12} {:>8.4} {:>12.6}", expiry, strike, vol.value());
    }

    println!("\nVolatility type: {:?}", surface_ref.volatility_type());
    println!("Smile type:      {:?}", surface_ref.smile_type());

    Ok(())
}
