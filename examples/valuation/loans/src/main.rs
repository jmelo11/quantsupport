//! Thirty-year EUR mortgage contracts and pricer requests. Run: cargo run -p valuation-loans
use quantsupport::prelude::*;
use quantsupport::time::calendars::target::TARGET;
use std::collections::HashSet;

type ExampleResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Creates a fixed coupon with the mortgage's Actual/360 accrual convention.
///
/// The fair-rate example uses this template both before solving and when
/// rebuilding the loan, so changing the rate cannot change its day count.
fn fixed_mortgage_coupon(reference: Date, rate: f64) -> CashflowType<Recorded> {
    CashflowType::FixedRateCoupon(FixedRateCoupon::new(
        1.0,
        Box::new(InterestRate::new(
            Recorded::from(rate),
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Actual360,
        )),
        reference,
        reference,
        reference,
    ))
}

/// Loads shared EUR quotes/curve specs and initializes them with supplied fixings
/// using ESTR as the base discount curve.
///
/// # Errors
/// Propagates JSON decoding, quote conversion, and market construction errors.
fn build_initialized_loan_market(fixings: FixingStore) -> ExampleResult<PricingContext> {
    let records: QuoteStoreRecords =
        serde_json::from_str(include_str!("../data/loan_quotes.json"))?;
    let curves: Vec<CurveConfiguration> =
        serde_json::from_str(include_str!("../data/loan_curves.json"))?;
    let credit: Vec<CreditCurveConfiguration> =
        serde_json::from_str(include_str!("../data/loan_credit_curves.json"))?;
    let mut context = PricingContext::new()
        .with_quote_store(records.try_into()?)
        .with_curve_configurations(curves)
        .with_credit_curve_configurations(credit)
        .with_fixing_store(fixings)
        .with_base_currency(Currency::EUR)
        .with_base_index(MarketIndex::ESTR);
    context.initialize()?;
    Ok(context)
}
/// Supplies common 30-year EUR mortgage terms: 250,000 principal, 6% fixed coupon,
/// monthly French payments, Actual/360, TARGET, and a two-business-day fixing lag.
fn default_loan_terms(reference: Date, identifier: &str) -> Result<MakeLoan<Recorded>> {
    Ok(MakeLoan::default()
        .with_identifier(identifier.into())
        .with_start_date(reference)
        .with_maturity_date(reference.advance(30, TimeUnit::Years)?)
        .with_notional(250_000.0)
        .with_currency(Currency::EUR)
        .with_coupons(vec![fixed_mortgage_coupon(reference, 0.06)])
        .with_repayment_structure(LoanRepaymentStructure::French)
        .with_payment_frequency(Frequency::Monthly)
        .with_calendar(Calendar::TARGET(TARGET::new()))
        .with_business_day_convention(BusinessDayConvention::Following)
        .with_fixing_lag_business_days(2))
}
/// Selects ESTR discounting for EUR loan valuations.
fn estr_loan_pricer() -> LoanPricer {
    let mut pricer = LoanPricer::new();
    pricer.set_discount_policy(Box::new(
        FixedIncomeDiscountPolicy::new(false)
            .with_discount_curve_index(Currency::EUR, MarketIndex::ESTR),
    ));
    pricer
}
/// Extracts signed receipt PV from a Value request on an initialized context.
///
/// # Errors
/// Propagates pricing errors or a missing Value result.
fn evaluate_loan_receipt_present_value(
    pricer: &LoanPricer,
    trade: &LoanTrade<Recorded>,
    context: &impl MarketDataProvider,
) -> Result<f64> {
    pricer
        .evaluate(trade, &[Request::Value], context)?
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("Loan value".into()))
}

/// Reprices a solved coupon or spread while retaining the mortgage's conventions.
///
/// `amortization_rate` selects the principal schedule independently of the fair
/// coupon. A fixed coupon retains Actual/360; a floating coupon retains EURIBOR12m.
///
/// # Errors
/// Propagates loan construction and valuation errors, or a missing price result.
fn reprice_fair_loan(
    builder: MakeLoan<Recorded>,
    pricer: &LoanPricer,
    context: &impl MarketDataProvider,
    fair: f64,
    amortization_rate: f64,
    floating: bool,
) -> Result<f64> {
    let reference = context.evaluation_date();
    let coupon = if floating {
        CashflowType::floating(MarketIndex::EURIBOR12m, fair, None, None)
    } else {
        fixed_mortgage_coupon(reference, fair)
    };
    let par = LoanTrade::new(
        builder
            .with_coupons(vec![coupon])
            .with_initial_amortization_rate(amortization_rate)
            .build()?,
        reference,
        Side::LongReceive,
    );
    evaluate_loan_receipt_present_value(pricer, &par, context)
}

/// Runs loan construction, fixing, prepayment, credit, fair-rate, and risk examples.
///
/// # Errors
/// Propagates contract, market-data, and valuation errors.
fn main() -> ExampleResult<()> {
    let reference = Date::new(2026, 2, 24);
    let fixed_builder = default_loan_terms(reference, "FIXED")?;
    let floating_builder = default_loan_terms(reference, "FLOATING")?
        .with_coupons(vec![CashflowType::floating(
            MarketIndex::EURIBOR12m,
            0.01,
            None,
            None,
        )])
        .with_initial_amortization_rate(0.05); // Initial all-in rate sets a fixed principal schedule.
    let fixed = fixed_builder.clone().build()?;
    let floating = floating_builder.clone().build()?;
    let mixed = default_loan_terms(reference, "MIXED")?
        .with_coupons({
            let mut coupons = vec![CashflowType::fixed(0.04); 60];
            coupons.push(CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            ));
            coupons
        })
        .build()?;

    // Each monthly floating coupon carries its contractual annual reset request.
    // Reset frequency comes from index metadata, separately from compounding.
    let mut seen = HashSet::new();
    let mut fixings = FixingStore::default();
    for observation in floating.coupon_rate_observations().iter().flatten() {
        if seen.insert(observation.fixing_date()) {
            println!(
                "EURIBOR12m fixing {}: {:?} to {:?}",
                observation.fixing_date(),
                observation.start_date(),
                observation.end_date()
            );
            if observation.fixing_date() < reference {
                fixings.add_fixing(&observation.market_index(), observation.fixing_date(), 0.04);
            }
        }
    }
    let context = build_initialized_loan_market(fixings)?;
    let pricer = estr_loan_pricer();
    for loan in [fixed, floating.clone(), mixed] {
        let trade = LoanTrade::new(loan, reference, Side::LongReceive);
        let result = pricer.evaluate(
            &trade,
            &[
                Request::Value,
                Request::Cashflows(true),
                Request::Sensitivities,
            ],
            &context,
        )?;
        println!(
            "{} receipt PV: EUR {:.2}",
            trade.instrument().identifier(),
            result.price().unwrap_or_default()
        );
    }

    // Project expected receipts with a constant annual prepayment rate of 25%.
    let prepaying = fixed_builder
        .clone()
        .with_identifier("CPR".into())
        .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.25))
        .build()?;
    let result = pricer.evaluate(
        &LoanTrade::new(prepaying, reference, Side::LongReceive),
        &[Request::Value, Request::Cashflows(true)],
        &context,
    )?;
    let table = result
        .cashflows()
        .ok_or_else(|| QSError::ValueNotSetErr("CPR receipts".into()))?;
    let prepaid: f64 = table
        .amounts()
        .iter()
        .zip(table.leg_indices())
        .filter(|(_, leg)| **leg == 1)
        .map(|(amount, _)| amount)
        .sum();
    println!(
        "25% CPR: PV EUR {:.2}, projected prepaid principal EUR {prepaid:.2}",
        result.price().unwrap_or_default()
    );

    // Fair-rate calculation can preserve the original schedule or recompute it.
    for rebuild in [false, true] {
        let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(rebuild);
        for (builder, original_rate, floating) in [
            (fixed_builder.clone(), 0.06, false),
            (floating_builder.clone(), 0.05, true),
        ] {
            let trade = LoanTrade::new(builder.clone().build()?, reference, Side::LongReceive);
            let fair = pricer
                .evaluate(&trade, &[Request::FairRate(0)], &context)?
                .fair_rate()
                .ok_or_else(|| QSError::ValueNotSetErr("Fair rate".into()))?;
            let amortization_rate = if rebuild {
                if floating {
                    0.04 + fair
                } else {
                    fair
                }
            } else {
                original_rate
            };
            let pv = reprice_fair_loan(
                builder,
                &pricer,
                &context,
                fair,
                amortization_rate,
                floating,
            )?;
            println!(
                "{} rebuild_amortization_for_fair_rate={rebuild}: fair coupon/spread {:.6}%, repriced PV EUR {pv:.2}",
                trade.instrument().identifier(),
                fair * 100.0
            );
        }
    }

    let trade = LoanTrade::new(floating, reference, Side::LongReceive);
    let risk = pricer.evaluate(&trade, &[Request::Sensitivities], &context)?;
    let risk = risk
        .sensitivities()
        .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
    let (pillar, derivative) = risk
        .instrument_keys()
        .iter()
        .zip(risk.exposure())
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .ok_or_else(|| QSError::ValueNotSetErr("Risk pillar".into()))?;
    let bump = 1e-5;
    let up_scenario = Scenario::new(vec![Shock::new(pillar, bump, ScenarioType::Absolute)]);
    let down_scenario = Scenario::new(vec![Shock::new(pillar, -bump, ScenarioType::Absolute)]);
    let up = up_scenario.apply(&context)?;
    let down = down_scenario.apply(&context)?;
    let fd = (evaluate_loan_receipt_present_value(&pricer, &trade, &up)?
        - evaluate_loan_receipt_present_value(&pricer, &trade, &down)?)
        / (2.0 * bump);
    println!("{pillar}: AAD {derivative:.4}, central difference {fd:.4}");

    let credit = fixed_builder
        .clone()
        .with_identifier("CREDIT".into())
        .with_credit_curve_index(MarketIndex::Credit("BORROWER".into()))
        .with_recovery_fraction(0.4)
        .build()?;
    let result = pricer.evaluate(
        &LoanTrade::new(credit, reference, Side::LongReceive),
        &[Request::Value, Request::Cashflows(true)],
        &context,
    )?;
    println!(
        "Credit receipt PV: EUR {:.2}",
        result.price().unwrap_or_default()
    );
    let refinancing = fixed_builder
        .with_identifier("REFINANCING".into())
        .with_prepayment_configuration(PrepaymentModelConfiguration::Refinancing {
            base_cpr: 0.02,
            incentive_multiplier: 4.0,
            maximum_cpr: 0.4,
        })
        .build()?;
    let result = estr_loan_pricer()
        .with_refinancing_benchmark_index(MarketIndex::EURIBOR12m)
        .evaluate(
            &LoanTrade::new(refinancing, reference, Side::LongReceive),
            &[Request::Value, Request::Cashflows(true)],
            &context,
        )?;
    println!(
        "Refinancing-sensitive receipt PV: EUR {:.2}",
        result.price().unwrap_or_default()
    );
    // Treasury discounts the mortgage using EURIBOR plus its term funding spread.
    let spread_index = MarketIndex::Other("FUNDINGSPREAD".into());
    let funding_index = MarketIndex::Other("COSTOFFUNDS".into());
    let mut configurations = context.curve_configurations().clone();
    let mut quotes = context.quote_store().clone();
    let mut identifiers = Vec::new();
    for (tenor, rate) in ["0D", "1Y", "5Y", "10Y", "20Y", "30Y", "35Y"]
        .iter()
        .zip([0.005, 0.006, 0.008, 0.01, 0.012, 0.013, 0.0135])
    {
        let identifier = format!("ZeroRate_EUR_OTHER:FUNDINGSPREAD_{tenor}");
        quotes.add_quote(Quote::new(identifier.parse()?, QuoteLevels::with_mid(rate)));
        identifiers.push(identifier);
    }
    configurations.push(CurveConfiguration::new(
        spread_index.clone(),
        DayCounter::Actual360,
        Interpolator::Linear,
        false,
        identifiers,
    ));
    configurations.push(CurveConfiguration::composite(
        funding_index.clone(),
        MarketIndex::EURIBOR12m,
        spread_index,
    ));
    let funding_context = context
        .with_quote_store(quotes)
        .with_curve_configurations(configurations);
    let mut funding_pricer = LoanPricer::new();
    funding_pricer.set_discount_policy(Box::new(
        FixedIncomeDiscountPolicy::new(false)
            .with_discount_curve_index(Currency::EUR, funding_index),
    ));
    let base = funding_pricer.evaluate(
        &trade,
        &[Request::Value, Request::Sensitivities],
        &funding_context,
    )?;
    println!(
        "Floating mortgage, cost-of-funds value: EUR {:.2}",
        base.price().unwrap_or_default()
    );
    if let Some(risk) = base.sensitivities() {
        for (pillar, exposure) in risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .filter(|(pillar, _)| pillar.contains("_FUNDINGSPREAD_"))
        {
            println!("  {pillar}: EUR {:.2} per +1 bp", exposure * 0.0001);
        }
    }
    let scenario = Scenario::new(vec![
        Shock::new("EURIBOR12m", 0.01, ScenarioType::Absolute),
        Shock::new("OTHER:FUNDINGSPREAD", 0.005, ScenarioType::Absolute),
    ]);
    let stressed_market = scenario.apply(&funding_context)?;
    let stressed = funding_pricer.evaluate(&trade, &[Request::Value], &stressed_market)?;
    println!(
        "EURIBOR +100 bp and funding +50 bp: value EUR {:.2}, change EUR {:.2}",
        stressed.price().unwrap_or_default(),
        stressed.price().unwrap_or_default() - base.price().unwrap_or_default()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds the floating mortgage with the same initial all-in rate as the walkthrough.
    fn floating_mortgage_terms(reference: Date) -> Result<MakeLoan<Recorded>> {
        Ok(default_loan_terms(reference, "FLOATING")?
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )])
            .with_initial_amortization_rate(0.05))
    }

    /// Initializes the shared market with the mortgage's historical EURIBOR fixing.
    fn mortgage_market(reference: Date) -> ExampleResult<PricingContext> {
        let floating = floating_mortgage_terms(reference)?.build()?;
        let mut fixings = FixingStore::default();
        for observation in floating.coupon_rate_observations().iter().flatten() {
            if observation.fixing_date() < reference {
                fixings.add_fixing(&observation.market_index(), observation.fixing_date(), 0.04);
            }
        }
        build_initialized_loan_market(fixings)
    }

    /// Checks that the reported expected receipts discounted on ESTR reproduce the price.
    fn assert_discounted_receipts_match_value(
        result: &EvaluationResults,
        context: &PricingContext,
    ) -> Result<()> {
        let table = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan cashflows".into()))?;
        let price = result
            .price()
            .ok_or_else(|| QSError::ValueNotSetErr("Loan value".into()))?;
        let element = context
            .constructed_elements()
            .discount_curve(&MarketIndex::ESTR)
            .ok_or_else(|| QSError::NotFoundErr("ESTR curve".into()))?;
        let curve = element.curve();
        let mut pv = 0.0;
        for (date, amount) in table.payment_dates().iter().zip(table.amounts()) {
            pv += amount * curve.discount_factor(*date)?.value();
        }
        assert!((pv - price).abs() < 1e-6, "receipts PV {pv}, price {price}");
        Ok(())
    }

    /// Extracts contractual principal payments without dropping cashflow errors.
    fn contractual_redemptions(loan: &Loan<Recorded>) -> Result<Vec<f64>> {
        loan.contractual_leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::Redemption(principal) => Some(principal.amount()),
                _ => None,
            })
            .collect()
    }

    /// Checks that monthly floating payments share thirty annual EURIBOR reset dates.
    #[test]
    fn floating_mortgage_has_thirty_distinct_annual_fixings() -> Result<()> {
        let floating = floating_mortgage_terms(Date::new(2026, 2, 24))?.build()?;
        let dates: HashSet<_> = floating
            .coupon_rate_observations()
            .iter()
            .flatten()
            .map(ForwardRateRequest::fixing_date)
            .collect();
        assert_eq!(dates.len(), 30);
        Ok(())
    }

    /// Checks that prepayment changes expected receipts while retaining the contractual leg.
    #[test]
    fn prepayment_preserves_contractual_principal_and_produces_positive_receipts(
    ) -> ExampleResult<()> {
        let reference = Date::new(2026, 2, 24);
        let builder = default_loan_terms(reference, "FIXED")?;
        let original = builder.clone().build()?;
        let prepaying = builder
            .with_identifier("CPR".into())
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(0.25))
            .build()?;
        assert_eq!(
            contractual_redemptions(&original)?,
            contractual_redemptions(&prepaying)?
        );
        let context = mortgage_market(reference)?;
        let result = estr_loan_pricer().evaluate(
            &LoanTrade::new(prepaying, reference, Side::LongReceive),
            &[Request::Value, Request::Cashflows(true)],
            &context,
        )?;
        assert_discounted_receipts_match_value(&result, &context)?;
        let table = result
            .cashflows()
            .ok_or_else(|| QSError::ValueNotSetErr("CPR receipts".into()))?;
        let prepaid: f64 = table
            .amounts()
            .iter()
            .zip(table.leg_indices())
            .filter(|(_, leg)| **leg == 1)
            .map(|(amount, _)| amount)
            .sum();
        assert!(prepaid > 0.0);
        Ok(())
    }

    /// Reconciles fixed, floating, mixed, credit and refinancing values to their expected receipts.
    #[test]
    fn mortgage_values_reconcile_to_discounted_expected_receipts() -> ExampleResult<()> {
        let reference = Date::new(2026, 2, 24);
        let fixed = default_loan_terms(reference, "FIXED")?;
        let mut mixed_coupons = vec![CashflowType::fixed(0.04); 60];
        mixed_coupons.push(CashflowType::floating(
            MarketIndex::EURIBOR12m,
            0.01,
            None,
            None,
        ));
        let loans = [
            fixed.clone().build()?,
            floating_mortgage_terms(reference)?.build()?,
            fixed.clone().with_coupons(mixed_coupons).build()?,
            fixed
                .clone()
                .with_credit_curve_index(MarketIndex::Credit("BORROWER".into()))
                .with_recovery_fraction(0.4)
                .build()?,
            fixed
                .with_prepayment_configuration(PrepaymentModelConfiguration::Refinancing {
                    base_cpr: 0.02,
                    incentive_multiplier: 4.0,
                    maximum_cpr: 0.4,
                })
                .build()?,
        ];
        let context = mortgage_market(reference)?;
        let pricer = estr_loan_pricer().with_refinancing_benchmark_index(MarketIndex::EURIBOR12m);
        for loan in loans {
            let result = pricer.evaluate(
                &LoanTrade::new(loan, reference, Side::LongReceive),
                &[Request::Value, Request::Cashflows(true)],
                &context,
            )?;
            assert_discounted_receipts_match_value(&result, &context)?;
        }
        Ok(())
    }

    /// Reprices fixed Actual/360 coupons and floating spreads to par under both schedule policies.
    #[test]
    fn fair_rate_repricing_preserves_conventions_and_returns_par() -> ExampleResult<()> {
        let reference = Date::new(2026, 2, 24);
        let context = mortgage_market(reference)?;
        for rebuild in [false, true] {
            let pricer = estr_loan_pricer().with_fair_rate_amortization_rebuild(rebuild);
            for (builder, original_rate, floating) in [
                (default_loan_terms(reference, "FIXED")?, 0.06, false),
                (floating_mortgage_terms(reference)?, 0.05, true),
            ] {
                let trade = LoanTrade::new(builder.clone().build()?, reference, Side::LongReceive);
                let fair = pricer
                    .evaluate(&trade, &[Request::FairRate(0)], &context)?
                    .fair_rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("Fair rate".into()))?;
                let amortization_rate = if rebuild {
                    if floating {
                        0.04 + fair
                    } else {
                        fair
                    }
                } else {
                    original_rate
                };
                let pv = reprice_fair_loan(
                    builder,
                    &pricer,
                    &context,
                    fair,
                    amortization_rate,
                    floating,
                )?;
                assert!(
                    (pv - 250_000.0).abs() < 1e-5,
                    "floating={floating}, rebuild={rebuild}, fair={fair:.14}, PV={pv:.12}"
                );
            }
        }
        Ok(())
    }

    /// Compares the largest reported mortgage sensitivity with a complete market revaluation.
    #[test]
    fn mortgage_quote_sensitivity_matches_central_revaluation() -> ExampleResult<()> {
        let reference = Date::new(2026, 2, 24);
        let context = mortgage_market(reference)?;
        let trade = LoanTrade::new(
            floating_mortgage_terms(reference)?.build()?,
            reference,
            Side::LongReceive,
        );
        let pricer = estr_loan_pricer();
        let result = pricer.evaluate(&trade, &[Request::Sensitivities], &context)?;
        let risk = result
            .sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("Risk".into()))?;
        let (pillar, derivative) = risk
            .instrument_keys()
            .iter()
            .zip(risk.exposure())
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .ok_or_else(|| QSError::ValueNotSetErr("Risk pillar".into()))?;
        let bump = 1e-5;
        let up_scenario = Scenario::new(vec![Shock::new(pillar, bump, ScenarioType::Absolute)]);
        let down_scenario = Scenario::new(vec![Shock::new(pillar, -bump, ScenarioType::Absolute)]);
        let up = up_scenario.apply(&context)?;
        let down = down_scenario.apply(&context)?;
        let finite_difference = (evaluate_loan_receipt_present_value(&pricer, &trade, &up)?
            - evaluate_loan_receipt_present_value(&pricer, &trade, &down)?)
            / (2.0 * bump);
        assert!(
            (finite_difference - derivative).abs() < 0.1 + derivative.abs() * 1e-3,
            "{pillar}: AD {derivative}, central difference {finite_difference}"
        );
        Ok(())
    }
}
