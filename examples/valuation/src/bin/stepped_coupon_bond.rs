//! Prices one callable stepped-coupon bond with both numerical engines.

use std::{cell::RefCell, rc::Rc};

use quantsupport::prelude::*;

const FACE: f64 = 1_000_000.0;
const OAS: f64 = 0.0075;

fn reference_date() -> Date {
    Date::new(2025, 1, 1)
}

fn trade() -> Result<SteppedCouponBondTrade<DualFwd>> {
    let instrument = MakeSteppedCouponBond::<DualFwd>::default()
        .with_identifier("USD-STEP-CALL-2031".into())
        .with_start_date(reference_date())
        .with_maturity_date(Date::new(2031, 1, 1))
        .with_notional(FACE)
        .with_units(100.0)
        .with_currency(Currency::USD)
        .with_payment_frequency(Frequency::Semiannual)
        // Entries are positional. The floating definition repeats from the
        // second generated coupon period through maturity.
        .with_coupons(vec![
            CashflowType::fixed(0.045),
            CashflowType::floating(MarketIndex::SOFR, 0.006, Some(0.0), Some(0.06)),
        ])
        // Original principal is outstanding in period one, then 90%, then
        // 75%; the final 75% repeats through maturity.
        .with_notional_percentages(vec![1.0, 0.90, 0.75])
        .with_call_schedule(vec![
            CallScheduleEntry::window(Date::new(2028, 1, 1), Date::new(2028, 6, 30), 101.0),
            CallScheduleEntry::window(Date::new(2029, 1, 1), Date::new(2030, 12, 31), 100.0),
        ])
        .build()?;
    Ok(SteppedCouponBondTrade::new(
        instrument,
        reference_date(),
        FACE,
        Side::LongReceive,
    ))
}

fn context(pricer: PricerImplementation) -> PricingContext {
    let rate_definition = RateDefinition::new(
        DayCounter::Actual365,
        Compounding::Continuous,
        Frequency::Annual,
    );
    let curve =
        FlatForwardTermStructure::new(reference_date(), DualFwd::from(0.03), rate_definition)
            .with_pillar_label("SOFR.flat".into());
    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
    );
    let configuration = PricerConfiguration::new(
        InstrumentType::SteppedCouponBond,
        pricer,
        vec![PricingModelConfiguration::new(
            MarketIndex::SOFR,
            ModelConfiguration::HullWhite {
                alpha: 0.05,
                parameter_source: ParameterSource::Fixed(GaussianRateModelParameters::new(0.01)),
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

fn main() -> Result<()> {
    let trade = trade()?;
    let time_grid = TimeGridConfiguration::new(96, 4096, DayCounter::Actual365);
    let tree_context = context(PricerImplementation::TreeBased { time_grid });
    let fourier_context = context(PricerImplementation::BackwardFourierEvolution {
        time_grid,
        fourier_grid: FourierGridConfiguration::new(512, 10.0),
    });
    let requests = [Request::Value, Request::OAS, Request::Sensitivities];
    let policy = || -> Box<dyn DiscountPolicy> {
        Box::new(
            FixedIncomeDiscountPolicy::new(false)
                .with_risk_free_index(Currency::USD, MarketIndex::SOFR),
        )
    };
    let mut tree_pricer = TreeBasedBondPricer::new(BondPricingAnchor::OAS(OAS));
    tree_pricer.set_discount_policy(policy());
    let mut fourier_pricer = BackwardFourierEvolutionBondPricer::new(BondPricingAnchor::OAS(OAS));
    fourier_pricer.set_discount_policy(policy());
    let tree = tree_pricer.evaluate(&trade, &requests, &tree_context)?;
    let fourier = fourier_pricer.evaluate(&trade, &requests, &fourier_context)?;
    let tree_value = tree
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("tree bond value".into()))?;
    let fourier_value = fourier
        .price()
        .ok_or_else(|| QSError::ValueNotSetErr("Fourier bond value".into()))?;
    let difference = (tree_value - fourier_value).abs();

    println!("Tree PV:             {tree_value:.8}");
    println!("Backward Fourier PV: {fourier_value:.8}");
    println!("Absolute difference: {difference:.3e}");
    println!("OAS:                 {:.4}%", OAS * 100.0);
    println!(
        "Tree sensitivities:  {:?}",
        tree.sensitivities()
            .ok_or_else(|| QSError::ValueNotSetErr("tree sensitivities".into()))?
    );

    const TOLERANCE: f64 = 1_500.0;
    if difference > TOLERANCE {
        return Err(QSError::InvalidValueErr(format!(
            "tree and backward-Fourier prices differ by {difference}, tolerance is {TOLERANCE}"
        )));
    }
    Ok(())
}
