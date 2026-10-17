# A first valuation

A five-year receive-fixed USD swap provides a controlled first example. The
contract receives a 3% fixed coupon, pays a single-fixing SOFR-indexed floating
coupon, and uses USD 10 million notional. Its market is a flat 3% continuously
compounded ACT/360 curve. This is deliberately a regular `Swap`, not an
overnight-index swap: choosing SOFR as the index does not by itself install
daily overnight compounding.

The complete example is
[`examples/valuation/swaps/src/main.rs`](https://github.com/jmelo11/quantsupport/blob/main/examples/valuation/swaps/src/main.rs).
Run it from the repository root:

```sh
cargo run -p valuation-swaps
```

## From contract to market

The following self-contained program constructs the same valuation. The
instrument builder fixes schedules and contractual amounts; the trade records
the position. Directly inserting a curve avoids calibration so the example can
isolate pricing conventions.

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

The nonzero value is informative. A continuously compounded 3% curve implies a
simple forward over an accrual interval \\(\tau\\) of

\\[
L=\frac{e^{0.03\tau}-1}{\tau},
\\]

which is not exactly 3%. Equal numbers under different compounding conventions
do not make a par swap. Receiving the fixed coupon therefore has a small
negative value in this setup.

`SOFR_flat` names the one independent rate in this hand-built market. A
calibrated market instead names the original quote pillars. The derivative's
unit is determined by that input; it is not automatically a DV01.

The example's cashflow table contains 24 raw rows, including the generated
principal exchanges on both legs. The default table reports underlying
unsigned amounts, with leg identity needed to reconstruct direction. It has
no discounted-PV column. Do not sum those amounts as signed receipts or assume
every row has already been filtered by evaluation date.

The remaining chapters explain what this controlled example deliberately
omits: calibrated market construction, explicit collateral policies, daily
overnight conventions, and the distinction between local derivatives and full
scenario revaluation.
