use std::collections::{BTreeMap, HashMap};

use quantsupport::prelude::*;
use scripting_examples::{
    discount_curve, native_swap, pricing_context, reference_date, scripted_swap_events,
};

fn csa_terms() -> CsaTerms {
    CsaTerms {
        collateral_index: MarketIndex::SOFR,
        collateral_currency: Currency::USD,
        credit_spread: 0.01,
        recovery: 0.4,
        funding_spread: 0.005,
        funding_spread_curve: None,
        funding_index: None,
        credit_index: None,
    }
}

fn run_xva(
    context: &PricingContext,
    config: XvaEngineConfig,
    claims: Vec<ContingentClaim>,
) -> Result<ExposureResult> {
    let mut netting_sets = HashMap::from([(
        "swap".to_string(),
        NettingSet::with_csa_terms(claims, csa_terms()),
    )]);
    XvaEngine::new(context, config)?.run(&mut netting_sets)
}

fn epe(result: &ExposureResult) -> Result<Vec<f64>> {
    result
        .cubes
        .iter()
        .find(|cube| cube.trade_id == "swap")
        .map(NpvCube::epe)
        .ok_or_else(|| QSError::NotFoundErr("swap exposure cube".to_string()))
}

fn sensitivities(result: &ExposureResult) -> Result<BTreeMap<String, f64>> {
    result
        .sensitivities
        .as_ref()
        .map(|values| values.iter().cloned().collect())
        .ok_or_else(|| QSError::ValueNotSetErr("XVA sensitivities".to_string()))
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let curve = discount_curve()?;
    let context = pricing_context(&curve);

    // Both products enter XVA as ordinary contingent claims. Running them
    // independently makes both exposure and AAD sensitivity comparisons explicit.
    let native_claims = native_swap()?.into_contingent_claims()?;
    let scripted_claims = ScriptedProduct::new(
        "scripted_swap",
        scripted_swap_events()?,
        reference_date(),
        Currency::USD,
        MarketIndex::SOFR,
    )?
    .contingent_claims()?;

    let config = XvaEngineConfig {
        model_configs: vec![LgmModelConfig {
            market_index: MarketIndex::SOFR,
            lambda: Some(0.05),
            sigma: Some(0.01),
            volatility: None,
            driver: None,
        }],
        fx_configs: Vec::new(),
        n_paths: 1_000,
        seed: 42,
        frequency: Frequency::Quarterly,
    };

    let native_result = run_xva(&context, config.clone(), native_claims)?;
    let scripted_result = run_xva(&context, config, scripted_claims)?;
    let native_epe = epe(&native_result)?;
    let scripted_epe = epe(&scripted_result)?;
    let maximum_epe_difference = native_epe
        .iter()
        .zip(&scripted_epe)
        .map(|(native, scripted)| (native - scripted).abs())
        .fold(0.0_f64, f64::max);

    println!("Native swap EPE:     {native_epe:?}");
    println!("Scripted swap EPE:   {scripted_epe:?}");
    println!("Maximum difference:  {maximum_epe_difference:.3e}");

    const EPE_TOLERANCE: f64 = 1.0e-8;
    if maximum_epe_difference > EPE_TOLERANCE {
        return Err(format!(
            "native and scripted swap EPE differ by {maximum_epe_difference}, tolerance is {EPE_TOLERANCE}"
        )
        .into());
    }

    let native_sensitivities = sensitivities(&native_result)?;
    let scripted_sensitivities = sensitivities(&scripted_result)?;
    if native_sensitivities
        .keys()
        .ne(scripted_sensitivities.keys())
    {
        return Err("native and scripted XVA sensitivity sets differ".into());
    }

    println!("\nCombined CVA/FVA sensitivities:");
    println!(
        "  {:<24} {:>18} {:>18} {:>12}",
        "Risk factor", "Native", "Scripted", "Difference"
    );
    let mut maximum_sensitivity_difference = 0.0_f64;
    for (label, native) in &native_sensitivities {
        let scripted = scripted_sensitivities
            .get(label)
            .ok_or_else(|| QSError::NotFoundErr(format!("scripted XVA sensitivity for {label}")))?;
        let difference = (native - scripted).abs();
        maximum_sensitivity_difference = maximum_sensitivity_difference.max(difference);
        println!("  {label:<24} {native:>18.8} {scripted:>18.8} {difference:>12.3e}");
    }

    const SENSITIVITY_TOLERANCE: f64 = 1.0e-8;
    if maximum_sensitivity_difference > SENSITIVITY_TOLERANCE {
        return Err(format!(
            "native and scripted XVA sensitivities differ by {maximum_sensitivity_difference}, tolerance is {SENSITIVITY_TOLERANCE}"
        )
        .into());
    }
    println!(
        "\nEPE aligned within {EPE_TOLERANCE:.0e}; maximum sensitivity difference: {maximum_sensitivity_difference:.3e} (aligned within {SENSITIVITY_TOLERANCE:.0e})"
    );

    Ok(())
}
