# A first valuation

Consider a five-year USD interest-rate swap with a notional of USD 10 million.
It receives a fixed coupon of 3%, using simple compounding and ACT/360 accrual,
and pays a floating coupon every six months. We use `MakeSwap` to define the
contract, then value it against a flat curve.

The complete example is
[`examples/valuation/swaps/src/main.rs`](https://github.com/jmelo11/quantsupport/blob/main/examples/valuation/swaps/src/main.rs).
Run it from the repository root:

```sh
cargo run -p valuation-swaps
```

## From contract to market

The program below builds the instrument and wraps it in a `SwapTrade` to
record the position. It then supplies a flat, continuously compounded 3%
curve for both projection and discounting. This keeps the market setup small;
the [curve chapters](../curves/overview.md) explain how to replace it with
curves calibrated to market quotes.

The floating leg uses one SOFR-indexed observation per coupon period.
Daily compounded overnight coupons are available through
`MakeOvernightIndexSwap`, introduced in [Swaps](../pricing/swaps.md).

```rust
# extern crate quantsupport;
use std::{cell::RefCell, rc::Rc};
use quantsupport::prelude::*;

fn main() -> Result<()> {
    let rd = Date::new(2024, 1, 15);
    let notional = 10_000_000.0;
    let instrument = MakeSwap::<DualFwd>::default()
        .with_identifier("USD_IRS_5Y".into())
        .with_start_date(rd)
        .with_maturity_date(Date::new(2029, 1, 15))
        .with_notional(notional)
        .with_fixed_rate(0.03)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360, Compounding::Simple, Frequency::Semiannual,
        ))
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Semiannual)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()?;
    let trade = SwapTrade::new(instrument, rd, notional, Side::LongReceive);
    let curve = FlatForwardTermStructure::new(
        rd, DualFwd::from(0.03),
        RateDefinition::new(
            DayCounter::Actual360, Compounding::Continuous, Frequency::Annual,
        ),
    ).with_pillar_label("SOFR_flat".into());
    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
    );
    let context = PricingContext::new()
        .with_quote_store(QuoteStore::new(rd))
        .with_constructed_elements(elements);
    let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
    let result = pricer.evaluate(
        &trade, &[Request::Value, Request::Cashflows, Request::Sensitivities],
        &context,
    )?;
    println!("NPV: {:?}", result.price());
    if let Some(risk) = result.sensitivities() {
        for (label, derivative) in risk.instrument_keys().iter().zip(risk.exposure()) {
            println!("{label}: {derivative}");
        }
    }
    Ok(())
}
```

No fixing is required here because the first observation occurs on the
evaluation date and can be forecast when no same-day fixing is supplied.
Seasoned coupons require the relevant historical observations.

## Interpreting the result

The native example produces an NPV of approximately **−10,723.59 USD**, with
flat-rate derivative **−47,405,484.05 USD per unit rate**, or approximately
**−4,740.55 USD for a +1 bp move** to first order.

To understand the value, consider the floating coupon. A continuously compounded
3% curve implies a simple forward over an accrual interval \\(\tau\\) of

\\[
L=\frac{e^{0.03\tau}-1}{\tau},
\\]

which is slightly above the fixed coupon of 3%. The receive-fixed position
therefore has a small negative value. The fixed coupon and the curve rate use
different compounding conventions even though both were entered as `0.03`.

The sensitivity is labelled `SOFR_flat`, the name assigned to the flat rate.
It measures the PV change per unit change in that decimal rate. Multiplying
by `0.0001` gives the first-order change for a one-basis-point increase. With
calibrated curves, the report uses the original quote pillars instead.

The cashflow table contains 24 rows, including principal exchanges generated
on both legs. It reports signed contractual amounts: receipts are positive
and payments are negative, including initial advances. The table includes all supplied
cashflows and has no discounted-PV column, so reconciling it to NPV requires
payment selection and discounting. [Swaps](../pricing/swaps.md)
describes these reporting conventions in more detail.
