//! Thirty-year EUR mortgage contracts and pricer requests. Run: cargo run -p valuation-loans
use quantsupport::prelude::*;
use quantsupport::time::calendars::target::TARGET;
use std::collections::HashSet;

type ExampleResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn market(fixings: FixingStore, scenarios: Vec<Scenario>) -> ExampleResult<PricingContext> {
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
        .with_scenarios(scenarios)
        .with_base_currency(Currency::EUR)
        .with_base_index(MarketIndex::ESTR);
    context.initialize()?;
    Ok(context)
}
fn terms(reference: Date, identifier: &str) -> MakeLoan<DualFwd> {
    MakeLoan::default()
        .with_identifier(identifier.into())
        .with_start_date(reference)
        .with_maturity_date(reference.advance(30, TimeUnit::Years))
        .with_notional(250_000.0)
        .with_currency(Currency::EUR)
        .with_rate(LoanRateDefinition::Fixed(0.06))
        .with_repayment_structure(LoanRepaymentStructure::French)
        .with_payment_frequency(Frequency::Monthly)
        .with_day_counter(DayCounter::Actual360)
        .with_calendar(Calendar::TARGET(TARGET::new()))
        .with_business_day_convention(BusinessDayConvention::Following)
        .with_fixing_lag(2)
}
/// Selects ESTR discounting for EUR loan valuations.
fn estr_loan_pricer() -> LoanPricer {
    let mut pricer = LoanPricer::new();
    pricer.set_discount_policy(Box::new(
        FixedIncomeDiscountPolicy::new(false)
            .with_risk_free_index(Currency::EUR, MarketIndex::ESTR),
    ));
    pricer
}
fn price(pricer: &LoanPricer, trade: &LoanTrade<DualFwd>, context: &PricingContext) -> Result<f64> {
    pricer
        .evaluate(trade, &[Request::Value], context)?
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("Loan value".into()))
}
fn reconcile(result: &EvaluationResults, context: &PricingContext) -> Result<()> {
    let table = result
        .cashflows()
        .ok_or_else(|| QSError::ValueNotSetErr("Loan cashflows".into()))?;
    let element = context
        .constructed_elements()
        .discount_curve(&MarketIndex::ESTR)
        .ok_or_else(|| QSError::NotFoundErr("ESTR curve".into()))?;
    let curve = element.curve();
    let mut pv = 0.0;
    for (date, amount) in table.payment_dates().iter().zip(table.amounts()) {
        pv += amount * curve.discount_factor(*date)?.value();
    }
    assert!((pv - result.price().unwrap_or_default()).abs() < 1e-6);
    Ok(())
}
fn main() -> ExampleResult<()> {
    let reference = Date::new(2026, 2, 24);
    let fixed_builder = terms(reference, "FIXED");
    let floating_builder = terms(reference, "FLOATING")
        .with_rate(LoanRateDefinition::Floating {
            index: MarketIndex::EURIBOR12m,
            spread: 0.01,
        })
        .with_amortization_rate(0.05); // Initial all-in rate sets a fixed principal schedule.
    let fixed = fixed_builder.clone().build()?;
    let floating = floating_builder.clone().build()?;
    let mixed = terms(reference, "MIXED")
        .with_rate(LoanRateDefinition::Mixed {
            fixed_rate: 0.04,
            switch_date: reference.advance(5, TimeUnit::Years),
            index: MarketIndex::EURIBOR12m,
            spread: 0.01,
        })
        .build()?;

    // Each monthly floating coupon carries its contractual annual reset request.
    // Reset frequency comes from index metadata, separately from compounding.
    let mut seen = HashSet::new();
    let mut fixings = FixingStore::default();
    for observation in floating.observations().iter().flatten() {
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
    assert_eq!(seen.len(), 30);
    let context = market(fixings.clone(), vec![])?;
    let pricer = estr_loan_pricer();
    for loan in [fixed.clone(), floating.clone(), mixed] {
        let trade = LoanTrade::new(loan, reference, Side::LongReceive);
        let result = pricer.evaluate(
            &trade,
            &[Request::Value, Request::Cashflows, Request::Sensitivities],
            &context,
        )?;
        reconcile(&result, &context)?;
        println!(
            "{} receipt PV: EUR {:.2}",
            trade.instrument().identifier(),
            result.price().unwrap_or_default()
        );
    }

    // The same contractual leg is retained with or without prepayment.
    let prepaying = fixed_builder
        .clone()
        .with_identifier("CPR".into())
        .with_prepayment(PrepaymentModelConfiguration::Cpr(0.25))
        .build()?;
    let fixed_redemptions = |loan: &Loan<DualFwd>| {
        loan.leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| {
                if let CashflowType::Redemption(principal) = cashflow {
                    principal.amount().ok()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(fixed_redemptions(&fixed), fixed_redemptions(&prepaying));
    let result = pricer.evaluate(
        &LoanTrade::new(prepaying, reference, Side::LongReceive),
        &[Request::Value, Request::Cashflows],
        &context,
    )?;
    reconcile(&result, &context)?;
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
    assert!(prepaid > 0.0);

    // Fair-rate calculation can preserve the original schedule or recompute it.
    for rebuild in [false, true] {
        let pricer = estr_loan_pricer().with_recompute_schedule(rebuild);
        for (builder, original_rate, floating) in [
            (fixed_builder.clone(), 0.06, false),
            (floating_builder.clone(), 0.05, true),
        ] {
            let trade = LoanTrade::new(builder.clone().build()?, reference, Side::LongReceive);
            let fair = pricer
                .evaluate(&trade, &[Request::FairRate], &context)?
                .fair_rate()
                .ok_or_else(|| QSError::ValueNotSetErr("Fair rate".into()))?;
            let rate = if floating {
                LoanRateDefinition::Floating {
                    index: MarketIndex::EURIBOR12m,
                    spread: fair,
                }
            } else {
                LoanRateDefinition::Fixed(fair)
            };
            let amortization_rate = if rebuild {
                if floating {
                    0.04 + fair
                } else {
                    fair
                }
            } else {
                original_rate
            };
            let par = LoanTrade::new(
                builder
                    .with_rate(rate)
                    .with_amortization_rate(amortization_rate)
                    .build()?,
                reference,
                Side::LongReceive,
            );
            let pv = price(&pricer, &par, &context)?;
            assert!((pv - 250_000.0).abs() < 1e-5);
            println!(
                "{} recompute_schedule={rebuild}: fair coupon/spread {:.6}%, repriced PV EUR {pv:.2}",
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
    let up = market(
        fixings.clone(),
        vec![Scenario::new(pillar, bump, ScenarioType::Absolute)],
    )?;
    let down = market(
        fixings.clone(),
        vec![Scenario::new(pillar, -bump, ScenarioType::Absolute)],
    )?;
    let fd = (price(&pricer, &trade, &up)? - price(&pricer, &trade, &down)?) / (2.0 * bump);
    assert!((fd - derivative).abs() < 0.1 + derivative.abs() * 1e-3);
    println!("{pillar}: AAD {derivative:.4}, central difference {fd:.4}");

    let credit = fixed_builder
        .clone()
        .with_identifier("CREDIT".into())
        .with_credit_index(MarketIndex::Credit("BORROWER".into()))
        .with_recovery(0.4)
        .build()?;
    let result = pricer.evaluate(
        &LoanTrade::new(credit, reference, Side::LongReceive),
        &[Request::Value, Request::Cashflows],
        &context,
    )?;
    reconcile(&result, &context)?;
    println!(
        "Credit receipt PV: EUR {:.2}",
        result.price().unwrap_or_default()
    );
    let refinancing = fixed_builder
        .with_identifier("REFINANCING".into())
        .with_prepayment(PrepaymentModelConfiguration::Refinancing {
            base_cpr: 0.02,
            incentive_multiplier: 4.0,
            maximum_cpr: 0.4,
        })
        .build()?;
    let result = estr_loan_pricer()
        .with_refinancing_index(MarketIndex::EURIBOR12m)
        .evaluate(
            &LoanTrade::new(refinancing, reference, Side::LongReceive),
            &[Request::Value, Request::Cashflows],
            &context,
        )?;
    reconcile(&result, &context)?;
    println!(
        "Refinancing-sensitive receipt PV: EUR {:.2}",
        result.price().unwrap_or_default()
    );
    println!("Contract, cashflow, fair-rate and sensitivity checks passed.");
    Ok(())
}
