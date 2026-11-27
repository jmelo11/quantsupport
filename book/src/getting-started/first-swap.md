# A first valuation

This chapter prices a five-year USD interest-rate swap with a notional of
USD 10 million. The swap:

- receives a 3% fixed coupon, with simple compounding and ACT/360 accrual;
- pays a floating coupon every six months.

We define the contract with `MakeSwap` and value it on a flat curve.

The complete example is
[`examples/valuation/swaps/src/main.rs`](https://github.com/jmelo11/quantsupport/blob/main/examples/valuation/swaps/src/main.rs).
Run it from the repository root:

```sh
cargo run -p valuation-swaps
```

## From contract to market

The program below:

1. builds the swap and wraps it in a `SwapTrade`, which records the position;
2. creates a flat 3% curve, continuously compounded, used for both projection
   and discounting;
3. asks the pricer for the value, cashflows, and sensitivities.

A flat curve keeps the example short. The [curve chapters](../curves/overview.md)
show how to build curves from market quotes instead.

The floating leg uses one SOFR observation per coupon period. For daily
compounded overnight coupons, use `MakeOvernightIndexSwap`, described in
[Swaps](../pricing/swaps.md).

```rust
# extern crate quantsupport;
use std::{cell::RefCell, rc::Rc};
use quantsupport::prelude::*;

fn main() -> Result<()> {
    let rd = Date::new(2024, 1, 15);
    let notional = 10_000_000.0;
    let instrument = MakeSwap::<Recorded>::default()
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
        rd, Recorded::from(0.03),
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
    let pricer = DiscountedCashflowPricer::<Swap<Recorded>, SwapTrade<Recorded>>::new();
    let result = pricer.evaluate(
        &trade, &[Request::Value, Request::Cashflows(true), Request::Sensitivities],
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

You do not need to supply a fixing here. The first SOFR observation falls on
the evaluation date. When no fixing is given for that date, the pricer projects
the rate from the curve. A swap that started in the past needs historical
fixings for its past observations.

## Interpreting the result

The example gives:

- an NPV of about **−10,723.59 USD**;
- a sensitivity to the flat rate of **−47,405,484.05 USD per unit rate**;
- about **−4,740.55 USD for a +1 bp move**, to first order.

Why is the value negative? A continuously compounded 3% curve gives a simple
forward rate over an accrual period \\(\tau\\) of

\\[
L=\frac{e^{0.03\tau}-1}{\tau},
\\]

which is slightly above the 3% fixed coupon. The receive-fixed position
therefore has a small negative value. Both rates were entered as `0.03`, but
they use different compounding conventions.

The sensitivity is labelled `SOFR_flat`, the label given to the flat rate. It
is the change in PV per unit change in that decimal rate. Multiply it by
`0.0001` to get the first-order change for a one-basis-point rise. With curves
built from market quotes, the report lists the quote pillars instead.

The cashflow table has 24 rows, because both legs exchange principal. Amounts
are signed and undiscounted: receipts are positive and payments are negative,
including the initial principal payment. The table lists every cashflow of the
instrument and has no present-value column. To reconcile it to the NPV, select
the payments that the pricer includes and discount them.
[Swaps](../pricing/swaps.md) explains these reporting conventions.
