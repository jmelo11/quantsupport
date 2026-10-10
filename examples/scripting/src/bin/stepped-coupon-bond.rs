//! Cross-checks a callable stepped-coupon bond against its equivalent script.
//!
//! The Hull-White/LGM volatility is zero so the exercise decision is
//! deterministic. This isolates contractual schedule and call-ordering logic
//! from Monte Carlo sampling error.

use std::{cell::RefCell, rc::Rc};

use quantsupport::prelude::*;

const FACE: f64 = 100.0;

fn reference_date() -> Date {
    Date::new(2025, 1, 1)
}

fn curve() -> FlatForwardTermStructure<DualFwd> {
    FlatForwardTermStructure::new(
        reference_date(),
        DualFwd::from(0.03),
        RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Continuous,
            Frequency::Annual,
        ),
    )
    .with_pillar_label("SOFR.flat".into())
}

fn native_trade() -> Result<SteppedCouponBondTrade<DualFwd>> {
    let instrument = MakeSteppedCouponBond::<DualFwd>::default()
        .with_identifier("SCRIPT-CHECK-STEP-CALL".into())
        .with_start_date(reference_date())
        .with_maturity_date(Date::new(2029, 1, 1))
        .with_notional(FACE)
        .with_units(100.0)
        .with_currency(Currency::USD)
        .with_payment_frequency(Frequency::Annual)
        .with_coupons(vec![
            CashflowType::fixed(0.04),
            CashflowType::floating(MarketIndex::SOFR, 0.005, Some(0.0), Some(0.06)),
        ])
        .with_notional_percentages(vec![1.0, 0.8])
        .with_call_schedule(vec![CallScheduleEntry::new(Date::new(2027, 1, 1), 70.0)])
        .build()?;
    Ok(SteppedCouponBondTrade::new(
        instrument,
        reference_date(),
        FACE,
        Side::LongReceive,
        BondPricingAnchor::OAS(0.0),
    ))
}

fn native_context() -> PricingContext {
    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve()))),
    );
    let configuration = PricerConfiguration::new(
        InstrumentType::SteppedCouponBond,
        PricerImplementation::TreeBased {
            time_grid: TimeGridConfiguration::new(96, 4096, DayCounter::Actual365),
        },
        vec![PricingModelConfiguration::new(
            MarketIndex::SOFR,
            ModelConfiguration::HullWhite {
                alpha: 0.05,
                parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.0)),
            },
        )],
    );
    PricingContext::new()
        .with_quote_store(QuoteStore::new(reference_date()))
        .with_fixing_store(FixingStore::default())
        .with_constructed_elements(elements)
        .with_pricer_configurations(vec![configuration])
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
}

fn scripted_events() -> std::result::Result<EventStream, ScriptingError> {
    EventStream::try_from(vec![
        CodedEvent::new(
            Date::new(2025, 1, 1),
            r#"
                bond = 0;
                alive = 1;
                bond pays 4.0 on "2026-01-01";
                bond pays 20.0 on "2026-01-01";
            "#
            .into(),
        ),
        CodedEvent::new(
            Date::new(2026, 1, 1),
            r#"
                a2 = cvg("2026-01-01", "2027-01-01", "Actual360");
                r2 = max(min(RateIndex("SOFR", "2026-01-01", "2027-01-01") + 0.005, 0.06), 0.0);
                bond pays 80.0 * r2 * a2 on "2027-01-01";
            "#
            .into(),
        ),
        CodedEvent::new(
            Date::new(2027, 1, 1),
            r#"
                a3 = cvg("2027-01-01", "2028-01-01", "Actual360");
                a4 = cvg("2028-01-01", "2029-01-01", "Actual360");
                r3 = max(min(RateIndex("SOFR", "2027-01-01", "2028-01-01") + 0.005, 0.06), 0.0);
                r4 = max(min(RateIndex("SOFR", "2028-01-01", "2029-01-01") + 0.005, 0.06), 0.0);
                c3 = 80.0 * r3 * a3;
                c4 = 80.0 * r4 * a4;
                continuation = c3 * Df("2028-01-01") + (c4 + 80.0) * Df("2029-01-01");
                if continuation > 56.0 {
                    bond pays 56.0;
                    alive = 0;
                } else {
                    bond pays c3 on "2028-01-01";
                }
            "#
            .into(),
        ),
        CodedEvent::new(
            Date::new(2028, 1, 1),
            r#"
                if alive > 0.5 {
                    a4_due = cvg("2028-01-01", "2029-01-01", "Actual360");
                    r4_due = max(min(RateIndex("SOFR", "2028-01-01", "2029-01-01") + 0.005, 0.06), 0.0);
                    bond pays 80.0 * r4_due * a4_due + 80.0 on "2029-01-01";
                }
            "#
            .into(),
        ),
    ])
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut pricer = TreeBasedBondPricer::new();
    pricer.set_discount_policy(Box::new(
        FixedIncomeDiscountPolicy::new(false)
            .with_risk_free_index(Currency::USD, MarketIndex::SOFR),
    ));
    let native = pricer
        .evaluate(&native_trade()?, &[Request::Value], &native_context())?
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("native bond value".into()))?;

    let mut scripted_curve = curve();
    Tape::start_recording_fwd();
    scripted_curve.put_pillars_on_tape();
    let rate_model = LgmRateModel::new(DualFwd::scalar(0.05), DualFwd::zero(), &scripted_curve);
    let mut model = LgmMarketModel::new(
        Currency::USD,
        MarketIndex::SOFR,
        reference_date(),
        DayCounter::Actual365,
    )
    .with_n_paths(1)
    .with_seed(42);
    model.add_curve_model(MarketIndex::SOFR, rate_model);
    let scripted = ScriptEngine::new(
        scripted_events()?,
        reference_date(),
        Currency::USD,
        MarketIndex::SOFR,
    )?
    .evaluate(&mut model, Some("bond"))?
    .get("bond")
    .copied()
    .ok_or_else(|| QSError::ValueNotSetErr("scripted bond value".into()))?;
    Tape::stop_recording_fwd();

    let difference = (native - scripted).abs();
    println!("Native tree PV:  {native:.10}");
    println!("Scripted PV:     {scripted:.10}");
    println!("Absolute error:  {difference:.3e}");
    const TOLERANCE: f64 = 5.0e-2;
    if difference > TOLERANCE {
        return Err(format!(
            "native and scripted stepped-bond PVs differ by {difference}, tolerance is {TOLERANCE}"
        )
        .into());
    }
    Ok(())
}
