//! Report the same swap's risk against deposits, OIS quotes, and zero rates.
//!
//! Run from the repository root:
//! `cargo run -p sensitivity --bin risk-reprojection`.

use quantsupport::prelude::*;
use quantsupport::utils::tools::sort_sensitivities_by_tenor;

/// Constructs the two-deposit EUR market used by all three reporting bases.
///
/// # Errors
/// Propagates quote parsing and market initialization errors.
fn pricing_context() -> Result<PricingContext> {
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
    Ok(context)
}

/// Builds the swap whose price and risk are reported in each basis.
///
/// # Errors
/// Propagates tenor parsing and instrument construction errors.
fn swap_trade(date: Date) -> Result<OvernightIndexSwapTrade<Recorded>> {
    let notional = 1_000_000.0;
    let swap =
        MakeOvernightIndexSwap::<Recorded>::new(MarketIndex::ESTR, date, "18M".parse()?, 0.035)
            .with_identifier("EUR_ESTR_OIS_18M".into())
            .with_notional(notional)
            .build()?;
    Ok(OvernightIndexSwapTrade::new(
        swap,
        date,
        notional,
        Side::LongReceive,
    ))
}

/// Displays how the same swap's risk changes with its reporting coordinates.
///
/// # Errors
/// Propagates construction, pricing and risk-basis transformation errors.
fn main() -> Result<()> {
    let context = pricing_context()?;
    let trade = swap_trade(context.evaluation_date())?;
    let mut pricer = DiscountedCashflowPricer::<
        OvernightIndexSwap<Recorded>,
        OvernightIndexSwapTrade<Recorded>,
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

    println!(
        "\nNPV change from the original basis: OIS = {:.6} EUR, zero rates = {:.6} EUR",
        projected_price - native_price,
        zero_price - native_price
    );
    Ok(())
}

/// Prints a valuation's price and labelled sensitivities, returning the price.
///
/// # Errors
/// Returns an error when the valuation has no price or sensitivity result.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that both reporting transformations preserve the example swap's exact value.
    #[test]
    fn both_risk_bases_preserve_the_original_swap_price() -> Result<()> {
        let context = pricing_context()?;
        let trade = swap_trade(context.evaluation_date())?;
        let mut pricer = DiscountedCashflowPricer::<
            OvernightIndexSwap<Recorded>,
            OvernightIndexSwapTrade<Recorded>,
        >::new();
        pricer.set_discount_policy(Box::new(context.csa_policy()));
        let requests = [Request::Value, Request::Sensitivities];
        let native = pricer.evaluate(&trade, &requests, &context)?;
        let price = native
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Original swap price".into()))?;
        let instruments = ["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"]
            .iter()
            .map(|label| label.parse())
            .collect::<Result<Vec<_>>>()?;
        let projected = context.reproject_risk(&instruments)?;
        let zero = context.with_zero_rate_risk(&[ZeroRateRisk::new(
            MarketIndex::ESTR,
            RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Continuous,
                Frequency::Annual,
            ),
        )])?;
        for view in [projected, zero] {
            let result = pricer.evaluate(&trade, &requests, &view)?;
            assert_eq!(result.price(), Some(price));
        }
        Ok(())
    }
}
