use std::collections::HashMap;

use quantsupport::prelude::*;
use scripting_examples::{discount_curve, native_swap, pricing_context, reference_date};

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut curve = discount_curve()?;

    // Price the ordinary library swap with the standard cashflow pricer.
    let native_trade: SwapTrade<DualFwd> = native_swap()?.into();
    let native_results = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()
        .evaluate(
            &native_trade,
            &[Request::Value, Request::Sensitivities],
            &pricing_context(&curve),
        )?;
    let native_npv = native_results
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("native swap NPV".to_string()))?;
    let native_sensitivities = native_results
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("native swap sensitivities".to_string()))?;

    // Price the equivalent script with the same curve through the LGM market model.
    // Zero volatility makes this an exact representation comparison rather than
    // a Monte Carlo convergence test.
    Tape::start_recording_fwd();
    curve.put_pillars_on_tape();
    let rate_model = LgmRateModel::new(DualFwd::scalar(0.03), DualFwd::zero(), &curve);
    let mut model = LgmMarketModel::new(
        Currency::USD,
        MarketIndex::SOFR,
        reference_date(),
        DayCounter::Actual360,
    )
    .with_n_paths(1)
    .with_seed(42);
    model.add_curve_model(MarketIndex::SOFR, rate_model);

    let script = ScriptEngine::new(
        scripting_examples::scripted_swap_events()?,
        reference_date(),
        Currency::USD,
        MarketIndex::SOFR,
    )?;
    let script_results = script.evaluate(&mut model)?;
    let scripted_value = match script_results.get("swap") {
        Some(ScriptValue::Number(value)) => *value,
        other => return Err(format!("expected numeric scripted swap NPV, got {other:?}").into()),
    };
    scripted_value.backward()?;
    let scripted_npv = scripted_value.value();
    let scripted_sensitivities: HashMap<String, f64> = curve
        .pillars()
        .ok_or_else(|| QSError::ValueNotSetErr("scripted curve pillars".to_string()))?
        .into_iter()
        .map(|(label, pillar)| pillar.adjoint().map(|adjoint| (label, adjoint.value())))
        .collect::<Result<HashMap<_, _>>>()?;
    Tape::stop_recording_fwd();

    let difference = (native_npv - scripted_npv).abs();
    println!("Native swap NPV:     {native_npv:.8}");
    println!("Scripted swap NPV:   {scripted_npv:.8}");
    println!("Absolute difference: {difference:.3e}");

    const TOLERANCE: f64 = 1.0e-8;
    if difference > TOLERANCE {
        return Err(format!(
            "native and scripted swap NPVs differ by {difference}, tolerance is {TOLERANCE}"
        )
        .into());
    }
    println!("Aligned within {TOLERANCE:.0e}: yes");

    if native_sensitivities.instrument_keys().len() != scripted_sensitivities.len() {
        return Err("native and scripted sensitivity sets have different sizes".into());
    }

    println!("\nDiscount-factor sensitivities:");
    println!(
        "  {:<12} {:>18} {:>18} {:>12}",
        "Pillar", "Native", "Scripted", "Difference"
    );
    let mut maximum_sensitivity_difference = 0.0_f64;
    for (label, native) in native_sensitivities
        .instrument_keys()
        .iter()
        .zip(native_sensitivities.exposure())
    {
        let scripted = scripted_sensitivities.get(label).ok_or_else(|| {
            QSError::NotFoundErr(format!("scripted sensitivity for pillar {label}"))
        })?;
        let sensitivity_difference = (native - scripted).abs();
        maximum_sensitivity_difference = maximum_sensitivity_difference.max(sensitivity_difference);
        println!("  {label:<12} {native:>18.8} {scripted:>18.8} {sensitivity_difference:>12.3e}");
    }

    const SENSITIVITY_TOLERANCE: f64 = 1.0e-6;
    if maximum_sensitivity_difference > SENSITIVITY_TOLERANCE {
        return Err(format!(
            "native and scripted sensitivities differ by {maximum_sensitivity_difference}, tolerance is {SENSITIVITY_TOLERANCE}"
        )
        .into());
    }
    println!(
        "Maximum sensitivity difference: {maximum_sensitivity_difference:.3e} (aligned within {SENSITIVITY_TOLERANCE:.0e})"
    );

    Ok(())
}
