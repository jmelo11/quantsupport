# Your First Swap

This chapter introduces the QuantSupport pricing workflow through a five-year USD SOFR swap. The values are specific to the example, but the workflow applies to other products:

1. Define the instrument's contractual economics.
2. Wrap the instrument in a trade.
3. Assemble the market state for an evaluation date.
4. Select a compatible pricer and request specific calculations.
5. Read the requested values from `EvaluationResults`.

These responsibilities are deliberately separate. Instruments do not look up curves, market contexts do not decide which outputs to calculate, and result objects contain only the outputs produced by the pricer.

The complete program is in [`examples/valuation/src/main.rs`](../../../examples/valuation/src/main.rs). Run it from the workspace root with `cargo run -p valuation`.

## Choosing a scalar type

Most numerical types in QuantSupport are generic over `T: Scalar`. The scalar determines whether a calculation carries only values or also automatic derivatives:

- `f64` is appropriate for value-only calculations where the relevant market data and pricer support it.
- `DualFwd` carries automatic-differentiation information used by the current pricing and sensitivity infrastructure.

The instrument, curves, and pricer must use compatible scalar types. This example requests curve sensitivities, so it uses `DualFwd` throughout.

## 1. Define the instrument

An **instrument** describes contractual economics: schedules, rates, indices, currencies, and payoff direction. QuantSupport constructs instruments with `Make*` builders. A builder collects inputs, applies documented defaults, and validates required fields in `build()`.

For a vanilla fixed-versus-floating swap, `MakeSwap<T>` creates a fixed leg and a floating leg. `RateDefinition` describes how the fixed rate accrues through its day-count, compounding, and frequency conventions. Leg payment frequency is configured separately because payment and rate conventions are distinct.

### In this example

The contract receives a 3% fixed rate and pays six-month SOFR on USD 10 million from 15 January 2024 to 15 January 2029:

```rust,ignore
use std::{cell::RefCell, rc::Rc};
use quantsupport::prelude::*;

let start_date = Date::new(2024, 1, 15);
let maturity_date = Date::new(2029, 1, 15);
let notional = 10_000_000.0;

let rate_definition = RateDefinition::new(
    DayCounter::Actual360,
    Compounding::Simple,
    Frequency::Semiannual,
);

let swap = MakeSwap::<DualFwd>::default()
    .with_identifier("USD_IRS_5Y".to_string())
    .with_start_date(start_date)
    .with_maturity_date(maturity_date)
    .with_fixed_rate(0.030)
    .with_notional(notional)
    .with_rate_definition(rate_definition)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_side(Side::LongReceive)
    .with_fixed_leg_frequency(Frequency::Semiannual)
    .with_floating_leg_frequency(Frequency::Semiannual)
    .build()?;
```

`build()` returns `QSError` when a required field is absent or invalid. For `MakeSwap`, the required fields are the identifier, dates, notional, fixed rate, rate definition, currency, and floating-rate index. The principal optional settings are:

| Builder method                                        | Default                                          |
| ----------------------------------------------------- | ------------------------------------------------ |
| `with_spread(f64)`                                    | `0.0` on the floating leg                        |
| `with_side(Side)`                                     | `Side::LongReceive`                              |
| `with_fixed_leg_frequency(Frequency)`                 | `Frequency::Semiannual`                          |
| `with_floating_leg_frequency(Frequency)`              | `Frequency::Quarterly`                           |
| `with_calendar(Calendar)`                             | `Calendar::NullCalendar` (no holiday adjustment) |
| `with_business_day_convention(BusinessDayConvention)` | `Unadjusted`                                     |
| `with_date_generation_rule(DateGenerationRule)`       | `Backward` for bullet legs                       |
| `with_end_of_month(bool)`                             | `false`                                          |

Internally, leg `0` is fixed and has the swap's side. Leg `1` is floating, references `MarketIndex::SOFR`, and has the opposite side. Both are bullet legs, so their notionals do not amortize. The example overrides the floating-leg frequency from its quarterly default to semiannual.

## 2. Add the trade layer

An instrument defines what pays; a **trade** adds position-level metadata such as trade date, notional, and side. This separation lets pricing and portfolio workflows operate on positions without putting lifecycle metadata into every product definition. Pricers generally accept trades rather than bare instruments.

### In this example

```rust,ignore
let trade = SwapTrade::new(swap, start_date, notional, Side::LongReceive);
```

`LongReceive` means receive the fixed leg and pay the floating leg; `PayShort` reverses those signs.

## 3. Assemble the market

Pricing needs a market state as of an evaluation date. QuantSupport separates that state into three layers:

- Raw stores contain observations such as quotes, historical fixings, and FX rates.
- `ConstructedElementStore` contains derived objects such as discount and credit curves, volatility objects, and simulations.
- `PricingContext` owns those stores and implements `MarketDataProvider`, the interface through which pricers request only the data they need.

In a configuration-driven workflow, populate quotes and configurations and call `PricingContext::initialize()`. For a small program or unit test, constructed elements can instead be inserted directly. Elements are keyed by `MarketIndex`, so a SOFR leg resolves against the SOFR curve registered in the context; a missing required element is an error.

### In this example

The example creates one flat SOFR curve directly. `FlatForwardTermStructure` represents a constant rate interpreted using its `RateDefinition`; here the input is 3% with continuous compounding. The pillar label names that market input for sensitivity reporting.

```rust,ignore
let evaluation_date = Date::new(2024, 1, 15);
let discount_curve = FlatForwardTermStructure::new(
    evaluation_date,
    DualFwd::from(0.03),
    RateDefinition::new(
        DayCounter::Actual360,
        Compounding::Continuous,
        Frequency::Annual,
    ),
)
.with_pillar_label("SOFR_flat".to_string());

let mut constructed_elements = ConstructedElementStore::default();
constructed_elements.discount_curves_mut().insert(
    MarketIndex::SOFR,
    DiscountCurveElement::new(
        MarketIndex::SOFR,
        Rc::new(RefCell::new(discount_curve)),
    ),
);

let context = PricingContext::new()
    .with_quote_store(QuoteStore::new(evaluation_date))
    .with_fixing_store(FixingStore::default())
    .with_base_currency(Currency::USD)
    .with_constructed_elements(constructed_elements);
```

The curve is wrapped in `Rc<RefCell<_>>`, allowing constructed elements to be shared and updated by calibration workflows. The fixing store is empty because the swap starts on the evaluation date. The example does not call `initialize()` because its required curve has already been constructed and inserted.

## 4. Select a pricer and requests

A **pricer** connects a trade to market data. Its `market_data_request()` declares the curves, fixings, FX rates, and volatility objects it needs, and the provider resolves that declaration. The caller separately chooses outputs with `Request`, avoiding calculations that are not needed.

| Request                  | Meaning                                             |
| ------------------------ | --------------------------------------------------- |
| `Request::Value`         | Present value or NPV                                |
| `Request::Cashflows`     | Coupon and payment details                          |
| `Request::Sensitivities` | Derivatives with respect to labelled market pillars |
| `Request::FairRate`      | Rate that makes the instrument NPV equal to zero    |

Request support is pricer-specific.

### In this example

```rust,ignore
let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
let requests = vec![Request::Value, Request::Cashflows, Request::Sensitivities];
let results = pricer.evaluate(&trade, &requests, &context)?;
```

The generic parameters of `DiscountedCashflowPricer<I, T>` identify its instrument and trade types. It supports value, fair rate, cashflows, and sensitivities for leg-based products; `YieldToMaturity` and `ModifiedDuration` are not populated by this pricer. Value, cashflows, and sensitivities share one prepared valuation state during this `evaluate()` call.

## 5. Interpret the results

`EvaluationResults` is an envelope of optional outputs. A getter returns `Some(...)` when the corresponding result was produced and `None` otherwise. Callers should read the fields associated with the requests they submitted rather than assume every field is present.

### In this example

```rust,ignore
if let Some(price) = results.price() {
    println!("Swap NPV = {price:.2}");
}

if let Some(sensitivities) = results.sensitivities() {
    for (key, exposure) in sensitivities
        .instrument_keys()
        .iter()
        .zip(sensitivities.exposure())
    {
        println!("  {key}: {exposure:.4}");
    }
}

if let Some(cashflows) = results.cashflows() {
    let dates = cashflows.payment_dates();
    let types = cashflows.cashflow_types();
    let amounts = cashflows.amounts();
    let currencies = cashflows.currencies();

    for i in 0..dates.len() {
        println!(
            "{:<12} {:<22} {:>14.2} {:>6}",
            dates[i], types[i], amounts[i], currencies[i]
        );
    }
}
```

`SensitivityMap` contains parallel `instrument_keys()` and `exposure()` vectors. Each exposure is the derivative of NPV with respect to the labelled market pillar. This flat-curve example has one pillar, `SOFR_flat`, so it reports one value for \(\partial\mathrm{NPV}/\partial r\). A bootstrapped curve instead reports sensitivities against its quote labels, such as `OIS_USD_SOFR_5Y`.

`CashflowsTable` is column-oriented. In addition to the columns printed above, it exposes `fixing()`, `accrual_periods()`, `leg_indices()`, and optional caplet/floorlet strikes. Leg index `0` identifies fixed-leg rows and index `1` identifies floating-leg rows.

## What to read next

- [Rust API](rust-api.md) summarizes the traits behind the objects used above.
- [Pricing Context](../concepts/pricing-context.md) explains configuration-driven market construction and `initialize()`.
- [Interest Rate Swaps](../pricing/swaps.md) covers fair rates, spreads, fixings, and basis swaps.
