# Your First Swap

This example values a five-year receive-fixed USD swap. It uses a flat curve to isolate the pricing workflow; production contexts normally bootstrap the curve from market quotes.

```rust
use std::{cell::RefCell, rc::Rc};
use quantsupport::prelude::*;

fn main() -> Result<()> {
    let today = Date::new(2024, 1, 15);
    let notional = 10_000_000.0;
    let rate = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Simple,
        Frequency::Semiannual,
    );

    let swap = MakeSwap::<DualFwd>::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(today)
        .with_maturity_date(Date::new(2029, 1, 15))
        .with_fixed_rate(0.03)
        .with_notional(notional)
        .with_rate_definition(rate)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Semiannual)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()?;
    let trade = SwapTrade::new(swap, today, notional, Side::LongReceive);

    let curve = FlatForwardTermStructure::new(
        today,
        DualFwd::from(0.03),
        RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Continuous,
            Frequency::Annual,
        ),
    ).with_pillar_label("SOFR_flat".to_string());

    let mut market = ConstructedElementStore::default();
    market.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
    );
    let context = PricingContext::new()
        .with_quote_store(QuoteStore::new(today))
        .with_fixing_store(FixingStore::default())
        .with_base_currency(Currency::USD)
        .with_constructed_elements(market);

    let results = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()
        .evaluate(
            &trade,
            &[Request::Value, Request::FairRate, Request::Cashflows,
              Request::Sensitivities],
            &context,
        )?;
    println!("NPV: {:.2}", results.price().unwrap_or_default());
    println!("Par rate: {:.6}", results.fair_rate().unwrap_or_default());
    Ok(())
}
```

`DualFwd` keeps market variables differentiable. `Request::Sensitivities` therefore uses the same valuation as NPV rather than a separately maintained bump-and-reprice implementation. Run the expanded version with `cargo run -p valuation`.
