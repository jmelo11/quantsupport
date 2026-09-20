# Your First Swap

This chapter introduces the QuantSupport pricing workflow through a five-year USD SOFR swap. The example uses specific values, and the workflow applies to other products:

1. Define the instrument's contractual economics.
2. Wrap the instrument in a trade.
3. Assemble the market state for an evaluation date.
4. Select a compatible pricer and request specific calculations.
5. Read the requested values from `EvaluationResults`.

The complete program is in [`examples/valuation/src/main.rs`](../../../examples/valuation/src/main.rs). Run it from the workspace root with `cargo run -p valuation`.

## Choosing a scalar type

Most numerical types in QuantSupport are generic over `T: Scalar`. The scalar determines whether a calculation carries only values or also automatic derivatives:

- `f64` is appropriate for value-only calculations where the relevant market data and pricer support it.
- `DualFwd` carries automatic-differentiation information used by the current pricing and sensitivity infrastructure.

The instrument, curves, and pricer must use compatible scalar types. This example requests curve sensitivities, so it uses `DualFwd` throughout.

## 1. Define the instrument

An **instrument** describes a financial product contractual economics: schedules, rates, indices, currencies, and payoff direction. QuantSupport constructs instruments with `Make*` builders (builder pattern). A builder collects inputs, applies documented defaults, and validates required fields in `build()`. For a vanilla fixed-versus-floating swap, `MakeSwap<T>` creates a fixed leg and a floating leg, with the given parameters.

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

`build()` returns `QSError` when a required field is absent or invalid. For `MakeSwap`, the required fields are the identifier (a string to identify this particular swap), dates, notional, fixed rate, rate definition, currency, and floating-rate index. The principal optional settings are:

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

Internally, legs are stored in a vector. Leg `0` is fixed and has the swap's side. Leg `1` is floating, references `MarketIndex::SOFR`, and has the opposite side. Both are bullet legs with constant notionals. This example overrides the floating-leg frequency from its quarterly default to semiannual.

## 2. Add the trade layer

An instrument defines what pays. A **trade** adds position-level metadata such as trade date, notional, and side. This separation lets pricing and portfolio workflows operate on positions, and product definitions remain focused on contractual terms. Pricers generally accept trades.

### In this example

The trade uses the instrument dates and notional established above. `LongReceive` records that the position receives the swap's fixed leg:

```rust,ignore
let trade = SwapTrade::new(swap, start_date, notional, Side::LongReceive);
```

`LongReceive` means receive the fixed leg and pay the floating leg. `PayShort` reverses those signs.

## 3. Assemble the market

Pricing needs a market state (a set of market variables) as of an evaluation date. QuantSupport separates that state into three layers:

- Raw stores contain observations such as quotes, historical fixings, and FX rates.
- `ConstructedElementStore` contains derived objects such as discount and credit curves, volatility objects, and simulations.
- `PricingContext` owns those stores and implements `MarketDataProvider`, the interface through which pricers request only the data they need.

In a configuration-driven workflow, populate quotes and configurations and call `PricingContext::initialize()`. This constructs the discount curves, volatility objects, and other elements required for pricing. Every element is keyed by `MarketIndex`, so a SOFR leg resolves the curve registered under `MarketIndex::SOFR`. An unavailable index produces a descriptive lookup error.

### In this example

This example creates a flat SOFR curve. `FlatForwardTermStructure` represents a constant rate interpreted using its `RateDefinition`. Here the input is 3 percent with continuous compounding. The pillar label gives the sensitivity report a stable market name:

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

The curve is wrapped in `Rc<RefCell<_>>`, allowing constructed elements to be shared and updated by calibration workflows. The fixing store is empty because the swap starts on the evaluation date. The context is ready for pricing because the required curve was constructed and inserted directly.

## 4. Select a pricer and requests

A **pricer** connects a trade to market data and produces selected `Request` outputs. Its `market_data_request()` declares the required curves, fixings, FX rates, and volatility objects, and the market data provider resolves that declaration. The caller chooses the output set for each evaluation, keeping the calculation focused.

| Request                  | Meaning                                             |
| ------------------------ | --------------------------------------------------- |
| `Request::Value`         | Present value or NPV                                |
| `Request::Cashflows`     | Coupon and payment details                          |
| `Request::Sensitivities` | Derivatives with respect to labeled market pillars |
| `Request::FairRate`      | Rate that makes the instrument NPV equal to zero    |

Request support is pricer-specific because each product and valuation method exposes its own measures.

### In this example

The swap uses the general discounted-cashflow pricer. The request list asks one evaluation to return NPV, the generated payment schedule, and market sensitivities:

```rust,ignore
let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
let requests = vec![Request::Value, Request::Cashflows, Request::Sensitivities];
let results = pricer.evaluate(&trade, &requests, &context)?;
```

The generic parameters of `DiscountedCashflowPricer<I, T>` identify its instrument and trade types. It supports value, fair rate, cashflows, and sensitivities for leg-based products. Yield-to-maturity and modified-duration requests belong to specialized fixed-income pricers. Value, cashflows, and sensitivities share one prepared valuation state during this `evaluate()` call.

## 5. Interpret the results

`EvaluationResults` is an envelope of optional outputs. A getter returns `Some(...)` when the corresponding result was produced. Callers read the fields associated with the requests they submitted, and the remaining fields stay empty.

### In this example

Each getter corresponds to one requested output. This final block prints the NPV and risk labels, then iterates through the cashflow columns in row order:

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

`SensitivityMap` contains parallel `instrument_keys()` and `exposure()` vectors. Each exposure is the derivative of NPV with respect to the labeled market pillar. This flat-curve example has one pillar, `SOFR_flat`, so it reports one value for \\(\partial\mathrm{NPV}/\partial r\\). A bootstrapped curve reports sensitivities against its quote labels, such as `OIS_USD_SOFR_5Y`.

`CashflowsTable` is column-oriented. In addition to the columns printed above, it exposes `fixing()`, `accrual_periods()`, `leg_indices()`, and optional caplet/floorlet strikes. Leg index `0` identifies fixed-leg rows and index `1` identifies floating-leg rows.

## What to read next

The example has now crossed every boundary in the basic pricing path. The
instrument described the contract, the trade described the position, the
context supplied the market, the pricer performed the calculation, and the
results object exposed only the requested outputs. The following chapters
develop each of those responsibilities in more depth.

- [Rust API](rust-api.md) summarizes the traits behind the objects used above.
- [Pricing Context](../concepts/pricing-context.md) explains configuration-driven market construction and `initialize()`.
- [Interest Rate Swaps](../pricing/swaps.md) covers fair rates, spreads, fixings, and basis swaps.

Use the same sequence for other products. Each product supplies its own builder
fields and pricer. The separation between contract, position, market,
calculation, and result remains the organizing principle.
