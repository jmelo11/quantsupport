QuantSupport
=========

**QuantSupport** is a high-performance quantitative finance library written in Rust, designed for precision, speed in financial calculations and ease of deployment into production enviroments.

Roadmap to Cargo
------------

**QuantSupport** is currently in alpha.

- **Available Pricers**
  - Implement pricers for fundamental products such as swaps, caps, floors, and swaptions, which are essential for supporting more complex financial processes.
- **Curve Bootstrapping and Volatility Stripping**
  - Develop robust procedures for generating market-data-derived components required by downstream analytics.
- **Comprehensive Testing**
  - Rigorous testing is critical for financial software to ensure reliability and correctness.
- **Usage Examples**
  - Provide high-quality examples to help users learn the library and reduce the learning curve.

Example – Pricing a Vanilla Interest-Rate Swap
------------

```rust
use std::{cell::RefCell, rc::Rc};

use quantsupport::prelude::*;

/// Build a 5-year receive-fixed / pay-floating vanilla USD swap.
fn create_swap() -> SwapTrade {
    let start_date = Date::new(2024, 1, 15);
    let maturity_date = Date::new(2029, 1, 15);
    let notional = 10_000_000.0;
    let fixed_rate = 0.030;

    let rate_definition = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Simple,
        Frequency::Semiannual,
    );

    let swap = MakeSwap::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(start_date)
        .with_maturity_date(maturity_date)
        .with_fixed_rate(fixed_rate)
        .with_notional(notional)
        .with_rate_definition(rate_definition)
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongRecieve) // receive fixed, pay floating
        .with_fixed_leg_frequency(Frequency::Semiannual)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()
        .expect("Failed to build swap");

    SwapTrade::new(swap, start_date, notional, Side::LongRecieve)
}

/// Build a pricing context backed by a flat SOFR discount curve at 3.0%.
fn create_pricing_context() -> ContextManager {
    let evaluation_date = Date::new(2024, 1, 15);
    let discount_rate = 0.03; // 3.0% flat curve

    let curve_definition = RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Continuous,
        Frequency::Annual,
    );
    let discount_curve = FlatForwardTermStructure::new(
        evaluation_date,
        ADReal::from(discount_rate),
        curve_definition,
    )
    .with_pillar_label("SOFR_flat".to_string());

    let mut constructed_elements = ConstructedElementStore::default();
    constructed_elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(
            MarketIndex::SOFR,
            Currency::USD,
            Rc::new(RefCell::new(discount_curve)),
        ),
    );

    let quote_store = QuoteStore::new(evaluation_date);
    let fixing_store = FixingStore::default();

    ContextManager::new(quote_store, fixing_store)
        .with_base_currency(Currency::USD)
        .with_constructed_elements(constructed_elements)
}

fn main() -> Result<()> {
    let trade = create_swap();
    let context = create_pricing_context();

    let pricer = CashflowDiscountPricer::<Swap, SwapTrade>::new();
    let requests = vec![Request::Value, Request::Cashflows, Request::Sensitivities];
    let results = pricer.evaluate(&trade, &requests, &context)?;

    if let Some(price) = results.price() {
        println!("Swap NPV = {price:.2}");
    }

    if let Some(sensitivities) = results.sensitivities() {
        println!("\nSensitivities:");
        let keys = sensitivities.instrument_keys();
        let exposures = sensitivities.exposure();
        for (key, exposure) in keys.iter().zip(exposures.iter()) {
            println!("  {key}: {exposure:.4}");
        }
    }

    Ok(())
}
```

Contributing
------------

Contributions to QuantSupport are welcome.
For small fixes, feel free to open a pull request directly.
For larger changes or design discussions, please open an issue first.

License
-------

QuantSupport is released under the MIT License. Details can be found in the [LICENSE](LICENSE) file.

Contact
-------

For more details or business inquiries, please contact <jmelo@live.cl>.
