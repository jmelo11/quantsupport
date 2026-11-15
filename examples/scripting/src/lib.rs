//! Shared setup for the native and scripted swap examples.

use std::{cell::RefCell, rc::Rc};

use quantsupport::{prelude::*, time::calendars::nullcalendar::NullCalendar};

/// Principal shared by the native swap and the scripted payments.
pub const NOTIONAL: f64 = 10_000_000.0;
/// Annual fixed coupon shared by both representations.
pub const FIXED_RATE: f64 = 0.035;

/// Returns the common valuation and swap start date.
#[must_use]
pub fn reference_date() -> Date {
    Date::new(2025, 1, 1)
}

/// Returns the final payment date for both representations.
#[must_use]
pub fn maturity_date() -> Date {
    Date::new(2026, 1, 1)
}

/// Returns the four unadjusted quarterly accrual intervals used by the script.
#[must_use]
pub fn accrual_periods() -> [(Date, Date); 4] {
    [
        (Date::new(2025, 1, 1), Date::new(2025, 4, 1)),
        (Date::new(2025, 4, 1), Date::new(2025, 7, 1)),
        (Date::new(2025, 7, 1), Date::new(2025, 10, 1)),
        (Date::new(2025, 10, 1), Date::new(2026, 1, 1)),
    ]
}

/// Curve shared by both valuation routes.
pub fn discount_curve() -> Result<DiscountTermStructure<Recorded>> {
    DiscountTermStructure::<Recorded>::new(
        vec![
            reference_date(),
            Date::new(2025, 4, 1),
            Date::new(2025, 7, 1),
            Date::new(2025, 10, 1),
            maturity_date(),
        ],
        vec![
            Recorded::one(),
            Recorded::from(0.9900),
            Recorded::from(0.9795),
            Recorded::from(0.9685),
            Recorded::from(0.9570),
        ],
        DayCounter::Actual360,
        Interpolator::LogLinear,
        true,
    )?
    .with_pillar_labels(vec![
        "SOFR.0M".to_string(),
        "SOFR.3M".to_string(),
        "SOFR.6M".to_string(),
        "SOFR.9M".to_string(),
        "SOFR.12M".to_string(),
    ])
}

/// Builds the native receive-fixed, pay-SOFR swap on the script's schedule.
///
/// The comparison deliberately uses unadjusted dates in both representations.
/// Index-default holiday adjustments would change the native contract and its
/// exposure timeline relative to the explicit dates in the script.
pub fn native_swap() -> Result<SwapTrade<f64>> {
    let rate_definition = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Simple,
        Frequency::Annual,
    );
    let swap = MakeSwap::<f64>::default()
        .with_identifier("USD_SOFR_SWAP".to_string())
        .with_start_date(reference_date())
        .with_maturity_date(maturity_date())
        .with_fixed_rate(FIXED_RATE)
        .with_notional(NOTIONAL)
        .with_rate_definition(rate_definition)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Quarterly)
        .with_floating_leg_frequency(Frequency::Quarterly)
        .with_floating_day_counter(DayCounter::Actual360)
        .with_calendar(Calendar::NullCalendar(NullCalendar::new()))
        .with_business_day_convention(BusinessDayConvention::Unadjusted)
        .build()?;

    Ok(SwapTrade::new(
        swap,
        reference_date(),
        NOTIONAL,
        Side::LongReceive,
    ))
}

/// The same swap expressed as four scripted net coupon payments.
pub fn scripted_swap_events() -> std::result::Result<EventStream, QSError> {
    let events: Vec<CodedEvent> = accrual_periods()
        .into_iter()
        .enumerate()
        .map(|(period, (start, end))| {
            let initialization = if period == 0 {
                format!("swap = 0; fixed_rate = {FIXED_RATE};")
            } else {
                String::new()
            };
            let source = format!(
                r#"
                {initialization}
                accrual = cvg("{start}", "{end}", "Actual360");
                floating_rate = RateIndex("SOFR", "{start}", "{end}");
                swap pays {NOTIONAL} * (fixed_rate - floating_rate) * accrual on "{end}";
                "#
            );
            CodedEvent::new(start, source)
        })
        .collect();

    EventStream::try_from(events)
}

/// Supplies the shared discount curve and empty fixing store to the native pricer.
#[must_use]
pub fn pricing_context(curve: &DiscountTermStructure<Recorded>) -> PricingContext {
    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve.clone()))),
    );

    PricingContext::new()
        .with_quote_store(QuoteStore::new(reference_date()))
        .with_fixing_store(FixingStore::default())
        .with_constructed_elements(elements)
        .with_base_currency(Currency::USD)
        .with_base_index(MarketIndex::SOFR)
}
