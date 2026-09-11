# Your First Swap

This chapter walks through `examples/valuation/src/main.rs` line by line. It values a five-year receive-fixed USD SOFR swap against a flat curve and asks for NPV, cashflows and curve sensitivities. A flat curve keeps the market setup short; [Curve Bootstrapping](../curves/bootstrapping.md) replaces it with quotes.

Run the finished program with `cargo run -p valuation`.

## 1. Build the instrument

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
    .with_side(Side::LongReceive)              // receive fixed, pay floating
    .with_fixed_leg_frequency(Frequency::Semiannual)
    .with_floating_leg_frequency(Frequency::Semiannual)
    .build()?;
```

`MakeSwap<T>` is a builder; `build()` fails with `QSError` if any of the required fields (`notional`, `start_date`, `maturity_date`, `fixed_rate`, `rate_definition`, `currency`, `market_index`, `identifier`) is missing. Optional fields and their defaults:

| Builder method | Default |
| --- | --- |
| `with_spread(f64)` | `0.0` on the floating leg |
| `with_side(Side)` | `Side::LongReceive` |
| `with_fixed_leg_frequency(Frequency)` | `Frequency::Semiannual` |
| `with_floating_leg_frequency(Frequency)` | `Frequency::Quarterly` |
| `with_calendar(Calendar)` | `Calendar::NullCalendar` (no holiday adjustment) |
| `with_business_day_convention(BusinessDayConvention)` | `Unadjusted` |
| `with_date_generation_rule(DateGenerationRule)` | `Backward` for bullet legs |
| `with_end_of_month(bool)` | `false` |

Internally the builder creates two `Leg`s with `MakeLeg`: leg `0` is the fixed leg on the swap's side, leg `1` is the floating leg on the opposite side, indexed by `market_index`, both bullet (no amortisation). The `RateDefinition` describes how the fixed rate accrues: day counter, compounding (`Simple`, `Compounded`, `Continuous`, `SimpleThenCompounded`, `CompoundedThenSimple`) and frequency. The scalar type `DualFwd` makes every coupon differentiable; use `MakeSwap::<f64>` when you only need a number.

## 2. Wrap it in a trade

```rust,ignore
let trade = SwapTrade::new(swap, start_date, notional, Side::LongReceive);
```

Instruments describe economics; trades add `trade_date`, `notional` and `side`. Pricers and the XVA engine take trades. `SwapTrade<f64>` converts into `SwapTrade<DualFwd>` with `.into()` when you want to reuse an `f64` definition on the tape.

## 3. Build a market

```rust,ignore
let evaluation_date = Date::new(2024, 1, 15);
let discount_curve = FlatForwardTermStructure::new(
    evaluation_date,
    DualFwd::from(0.03),
    RateDefinition::new(DayCounter::Actual360, Compounding::Continuous, Frequency::Annual),
)
.with_pillar_label("SOFR_flat".to_string());

let mut constructed_elements = ConstructedElementStore::default();
constructed_elements.discount_curves_mut().insert(
    MarketIndex::SOFR,
    DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(discount_curve))),
);

let context = PricingContext::new()
    .with_quote_store(QuoteStore::new(evaluation_date))
    .with_fixing_store(FixingStore::default())
    .with_base_currency(Currency::USD)
    .with_constructed_elements(constructed_elements);
```

`FlatForwardTermStructure::new(reference_date, rate, RateDefinition)` implements `InterestRatesTermStructure<T>` with a constant continuously-compounded rate; `with_pillar_label` gives its single rate a name that will appear in the sensitivity table. Curves are stored in a `ConstructedElementStore`, keyed by `MarketIndex`, and wrapped in `Rc<RefCell<_>>` so bootstrappers can update them in place. The `PricingContext` is the object every pricer reads from; because we built the curve ourselves, `initialize()` is not needed here.

## 4. Price

```rust,ignore
let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
let requests = vec![Request::Value, Request::Cashflows, Request::Sensitivities];
let results = pricer.evaluate(&trade, &requests, &context)?;
```

`DiscountedCashflowPricer<I, T>` is generic over the instrument and trade types and implements `Pricer`. It supports `Request::Value`, `FairRate`, `Cashflows`, `Sensitivities`, `YieldToMaturity` and `ModifiedDuration`. Passing several requests at once evaluates the cashflows once and derives every result from the same tape.

## 5. Read the results

```rust,ignore
if let Some(price) = results.price() {
    println!("Swap NPV = {price:.2}");
}

if let Some(sensitivities) = results.sensitivities() {
    for (key, exposure) in sensitivities.instrument_keys().iter().zip(sensitivities.exposure()) {
        println!("  {key}: {exposure:.4}");        // "SOFR_flat: -4,6xx,xxx.xxxx"
    }
}

if let Some(cashflows) = results.cashflows() {
    let dates = cashflows.payment_dates();
    let types = cashflows.cashflow_types();
    let amounts = cashflows.amounts();
    let currencies = cashflows.currencies();
    for i in 0..dates.len() {
        println!("{:<12} {:<22} {:>14.2} {:>6}", dates[i], types[i], amounts[i], currencies[i]);
    }
}
```

`EvaluationResults` exposes `price()`, `fair_rate()`, `sensitivities() -> Option<&SensitivityMap>` and `cashflows() -> Option<&CashflowsTable>`. `SensitivityMap` is a pair of parallel vectors: `instrument_keys()` (pillar labels) and `exposure()` (`dNPV/dPillar`). `CashflowsTable` is column-oriented: `payment_dates()`, `cashflow_types()`, `amounts()`, `fixing()`, `accrual_periods()`, `currencies()`, `leg_indices()`, plus `caplet_strikes()`/`floorlet_strikes()` for optional legs.

With the flat curve the sensitivity table has a single row, `SOFR_flat`, equal to \(\partial\text{NPV}/\partial r\). Once the curve is bootstrapped from quotes (next chapters) the same request returns one row per quote identifier, e.g. `OIS_USD_SOFR_5Y`.

## What to read next

- [Rust API](rust-api.md) summarises the traits behind the objects used above.
- [Pricing Context](../concepts/pricing-context.md) explains `initialize()` for configuration-driven markets.
- [Interest Rate Swaps](../pricing/swaps.md) covers fair rates, spreads, fixings and basis swaps.
