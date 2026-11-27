//! Black cap and caplet valuation. Run: cargo run -p valuation-caps

use quantsupport::prelude::*;

type ExampleResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(serde::Deserialize)]
struct CurveFile {
    curve_specs: Vec<CurveConfiguration>,
}

#[derive(serde::Deserialize)]
struct VolatilityFile {
    volatility_surfaces: Vec<VolatilitySurfaceConfiguration>,
}

/// Evaluates a SOFR caplet and cap using the example's curves and volatility surface.
///
/// # Errors
/// Propagates input decoding, contract, market construction, and pricing errors.
fn evaluate_caplet_and_cap() -> ExampleResult<(EvaluationResults, EvaluationResults)> {
    // These illustrative inputs are also used by the Python caps walkthrough.
    let records: QuoteStoreRecords =
        serde_json::from_str(include_str!("../../../hullwhite/data/quotes.json"))?;
    let curves: CurveFile =
        serde_json::from_str(include_str!("../../../hullwhite/data/curve_specs.json"))?;
    let volatilities: VolatilityFile =
        serde_json::from_str(include_str!("../../../hullwhite/data/vol_specs.json"))?;
    let reference = records.reference_date;
    let mut context = PricingContext::new()
        .with_quote_store(records.try_into()?)
        .with_curve_configurations(curves.curve_specs)
        .with_volatility_surface_configurations(volatilities.volatility_surfaces)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR);
    context.initialize()?;

    let fixing = reference.advance(1, TimeUnit::Years)?;
    let payment = fixing.advance(3, TimeUnit::Months)?;
    let caplet = CapletFloorlet::new(
        "USD-CAPLET-1Y".into(),
        MarketIndex::SOFR,
        Currency::USD,
        fixing,
        fixing,
        payment,
        payment,
        CapletFloorletType::Caplet,
        Strike::Absolute(0.045),
    );
    let cap = MakeCapFloor::default()
        .with_identifier("USD-CAP-1Y-3Y".into())
        .with_start_date(fixing)
        .with_maturity_date(reference.advance(3, TimeUnit::Years)?)
        .with_strike(0.045)
        .with_notional(1_000_000.0)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_cap_floor_type(CapFloorType::Cap)
        .with_frequency(Frequency::Quarterly)
        .build()?;

    let caplet_trade = CapletFloorletTrade::new(caplet, reference, 1_000_000.0, Side::LongReceive);
    let cap_trade = CapFloorTrade::new(cap, reference, 1_000_000.0, Side::LongReceive);
    let requests = [Request::Value, Request::Sensitivities];
    let caplet_result =
        ClosedFormBlackCapletPricer::new().evaluate(&caplet_trade, &requests, &context)?;
    let cap_result = ClosedFormBlackCapPricer::new().evaluate(&cap_trade, &requests, &context)?;
    Ok((caplet_result, cap_result))
}

/// Displays prices and quote sensitivities for a SOFR caplet and cap.
///
/// # Errors
/// Propagates market construction and pricing errors, or a missing price result.
fn main() -> ExampleResult<()> {
    let (caplet_result, cap_result) = evaluate_caplet_and_cap()?;
    let caplet_pv = caplet_result
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("Caplet value".into()))?;
    let cap_pv = cap_result
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("Cap value".into()))?;
    println!(
        "Illustrative SOFR market as of {}",
        caplet_result.evaluation_date()
    );
    println!("Single caplet PV: USD {caplet_pv:.4}");
    println!("Quarterly cap PV: USD {cap_pv:.4}");
    if let Some(risk) = caplet_result.sensitivities() {
        println!("\nCaplet quote risk (derivative per unit quote):");
        for (pillar, derivative) in risk.instrument_keys().iter().zip(risk.exposure()) {
            println!("  {pillar:<70} {derivative:>12.4}");
        }
    }
    println!("\nScale rate derivatives by 1e-4 for +1bp P&L; Black volatility derivatives by 0.01 for +1 vol point.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that the example's multi-period cap is worth more than its first caplet.
    #[test]
    fn cap_value_exceeds_its_positive_first_caplet() -> ExampleResult<()> {
        let (caplet, cap) = evaluate_caplet_and_cap()?;
        let caplet_pv = caplet
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Caplet value".into()))?;
        let cap_pv = cap
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Cap value".into()))?;
        assert!(caplet_pv > 0.0);
        assert!(cap_pv > caplet_pv);
        Ok(())
    }
}
