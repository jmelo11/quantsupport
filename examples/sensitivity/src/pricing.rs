use std::collections::HashMap;

use quantsupport::prelude::*;

use crate::output::{extract_cashflows, CashflowEntry, ProductOutput, SensitivityEntry};

// ---------------------------------------------------------------------------
// Pricing + result collection
// ---------------------------------------------------------------------------

pub fn price_product<I, T>(
    label: &str,
    trade: &T,
    context: &PricingContext,
    csa_index: MarketIndex,
    csa_currency: Currency,
    curve_lookup: &HashMap<MarketIndex, DiscountCurveElement>,
) -> std::result::Result<ProductOutput, Box<dyn std::error::Error>>
where
    I: Instrument,
    T: LegsProvider<DualFwd> + Trade<I> + Send + Sync,
{
    let mut pricer = DiscountedCashflowPricer::<I, T>::new();
    pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
        csa_index.clone(),
        csa_currency,
    )));

    let requests = vec![Request::Value, Request::Sensitivities, Request::Cashflows];
    let results = pricer.evaluate(trade, &requests, context)?;

    let npv = results.price().unwrap_or(f64::NAN);

    // Sensitivities (raw exposure + DV01)
    let sensitivities: Vec<SensitivityEntry> = if let Some(sens) = results.sensitivities() {
        sens.instrument_keys()
            .iter()
            .zip(sens.exposure().iter())
            .map(|(key, &exp)| SensitivityEntry {
                pillar: key.clone(),
                exposure: exp,
                dv01: exp * 1e-4,
            })
            .collect()
    } else {
        Vec::new()
    };

    // Cashflow details extracted from trade legs
    let cashflows: Vec<CashflowEntry> =
        extract_cashflows(trade.legs(), curve_lookup, csa_index, csa_currency);

    // Console output
    print_product_summary(label, npv, &sensitivities);

    Ok(ProductOutput {
        label: label.to_string(),
        npv,
        sensitivities,
        cashflows,
    })
}

fn print_product_summary(label: &str, npv: f64, sensitivities: &[SensitivityEntry]) {
    println!("═══════════════════════════════════════════════════════════════");
    println!("  {label}");
    println!("  NPV = {npv:>14.2} USD");
    println!("═══════════════════════════════════════════════════════════════\n");
    println!(
        "  {:<45} {:>14} {:>14}",
        "Pillar", "Exposure", "DV01 (USD/bp)"
    );
    println!("  {}", "-".repeat(75));
    let mut total_dv01 = 0.0_f64;
    for s in sensitivities {
        println!("  {:<45} {:>14.4} {:>14.2}", s.pillar, s.exposure, s.dv01);
        total_dv01 += s.dv01;
    }
    println!("  {}", "-".repeat(75));
    println!("  {:<45} {:>14} {:>14.2}", "TOTAL", "", total_dv01);
    println!();
}
