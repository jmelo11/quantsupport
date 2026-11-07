//! Report the same swap's risk against deposits, OIS quotes, and zero rates.
//!
//! Run from the repository root:
//! `cargo run -p sensitivity --bin risk-reprojection`.

use quantsupport::prelude::*;
use quantsupport::utils::tools::sort_sensitivities_by_tenor;

fn main() -> Result<()> {
    let date = Date::new(2026, 1, 15);
    let mut quotes = QuoteStore::new(date);
    let deposits = [
        ("FixedRateDeposit_EUR_ESTR_1Y", 0.03),
        ("FixedRateDeposit_EUR_ESTR_2Y", 0.04),
    ];
    for (label, rate) in deposits {
        quotes.add_quote(Quote::new(label.parse()?, QuoteLevels::with_mid(rate)));
    }
    let configuration = CurveConfiguration::new(
        MarketIndex::ESTR,
        DayCounter::Actual365,
        Interpolator::LogLinear,
        true,
        deposits.iter().map(|(label, _)| (*label).into()).collect(),
    );
    let mut context = PricingContext::new()
        .with_quote_store(quotes)
        .with_curve_configurations(vec![configuration])
        .with_base_currency(Currency::EUR)
        .with_base_index(MarketIndex::ESTR);
    context.initialize()?;

    let notional = 1_000_000.0;
    let swap =
        MakeOvernightIndexSwap::<DualFwd>::new(MarketIndex::ESTR, date, "18M".parse()?, 0.035)
            .with_identifier("EUR_ESTR_OIS_18M".into())
            .with_notional(notional)
            .build()?;
    let trade = OvernightIndexSwapTrade::new(swap, date, notional, Side::LongReceive);
    let mut pricer = DiscountedCashflowPricer::<
        OvernightIndexSwap<DualFwd>,
        OvernightIndexSwapTrade<DualFwd>,
    >::new();
    pricer.set_discount_policy(Box::new(context.csa_policy()));
    let requests = [Request::Value, Request::Sensitivities];

    let native = pricer.evaluate(&trade, &requests, &context)?;
    let native_price = print_result("Original deposit quote risk", &native)?;

    // These reporting contracts have no observations in the quote store.
    // Their implied par rates are solved from the original deposit-built curve.
    let reporting_instruments: Vec<QuoteDetails> = ["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"]
        .iter()
        .map(|label| label.parse())
        .collect::<Result<_>>()?;
    let projected_context = context.reproject_risk(&reporting_instruments)?;
    println!("\nImplied reporting quotes:");
    for (label, value) in projected_context
        .target_labels()
        .iter()
        .zip(projected_context.target_values())
    {
        println!("  {label}: {:.6}%", value * 100.0);
    }
    let projected = pricer.evaluate(&trade, &requests, &projected_context)?;
    let projected_price = print_result("Reprojected OIS quote risk", &projected)?;

    // Keep the pricing curve's discount-factor interpolation and report its
    // post-bootstrap zero rates at the original future nodes.
    let zero_convention = RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    );
    let zero_context =
        context.with_zero_rate_risk(&[ZeroRateRisk::new(MarketIndex::ESTR, zero_convention)])?;
    let zero = pricer.evaluate(&trade, &requests, &zero_context)?;
    let zero_price = print_result("Post-bootstrap zero-rate risk", &zero)?;

    assert_eq!(native_price, projected_price);
    assert_eq!(native_price, zero_price);
    println!("\nVerified: all three contexts produce exactly the same NPV.");
    Ok(())
}

fn print_result(label: &str, result: &EvaluationResults) -> Result<f64> {
    let price = result
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("Example NPV".into()))?;
    let sensitivities = result
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("Example sensitivities".into()))?;
    let sensitivities = sort_sensitivities_by_tenor(sensitivities, result.evaluation_date());
    println!("\n{label}: NPV = {price:.6} EUR");
    println!(
        "  {:<65} {:>16} {:>16}",
        "Pillar", "dV/dq", "+1 bp P&L (EUR)"
    );
    for (key, exposure) in sensitivities
        .instrument_keys()
        .iter()
        .zip(sensitivities.exposure())
    {
        println!("  {key:<65} {exposure:>16.6} {:>16.6}", exposure * 1e-4);
    }
    Ok(price)
}
