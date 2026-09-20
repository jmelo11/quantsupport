# Rust API

QuantSupport separates product definition, market construction, pricing, and results. Most applications follow the same path regardless of asset class:

```text
quotes and fixings -> PricingContext -> Pricer -> EvaluationResults
                           ^              ^
                           |              |
                   market requests   instrument + trade
```

The easiest entry point is `quantsupport::prelude::*`, which re-exports the types used by normal pricing workflows. Lower-level modules remain useful when implementing a new instrument, pricer, curve, or simulation model.

This chapter maps those responsibilities and their relationships. The generated Rust documentation supplies the complete method and trait-bound reference.

## The pricing pipeline

A typical valuation has five steps:

1. Build an **instrument** containing contractual economics.
2. Wrap it in a **trade** containing position metadata.
3. Prepare a **pricing context** containing market data as of one date.
4. Select a **pricer** and the outputs to calculate.
5. Read those outputs from **evaluation results**.

```rust,ignore
use quantsupport::prelude::*;

let instrument = MakeSwap::<DualFwd>::default().build()?; // contract fields
let trade = SwapTrade::new(instrument, trade_date, notional, side);
let context = PricingContext::new(); // quotes, fixings, curves, volatility, and configuration

let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
let results = pricer.evaluate(
    &trade,
    &[Request::Value, Request::Sensitivities],
    &context,
)?;
```

The [first swap](first-swap.md) chapter develops a complete example of this flow.

## Numerical scalar types

Curves, instruments, and models commonly use a generic `T: Scalar`. The scalar supplies arithmetic and mathematical operations and allows the same financial logic to run with different numeric representations.

```rust,ignore
pub trait Scalar: Copy + PartialOrd {
    fn scalar(value: f64) -> Self;
    fn value(&self) -> f64;
    fn zero() -> Self;
    fn one() -> Self;
    // arithmetic and elementary functions
}
```

The principal choices are:

| Scalar | Use |
| --- | --- |
| `f64` | Value-only calculations and simulation paths where the surrounding API supports plain values |
| `Fwd<T>` | Forward-mode automatic differentiation |
| `Dual<T>` | Reverse-mode automatic differentiation |
| `DualFwd` | The library's standard nested AD scalar for pricing and market sensitivities |

Scalar types must agree across connected objects. For example, a `Swap<DualFwd>` is valued against curves that produce `DualFwd`. The current constructed-market and standard pricing-context infrastructure is AD-oriented, so `DualFwd` is the normal choice for direct pricing. Some simulations and standalone numerical components use `f64`.

Use `.value()` when a scalar calculation reaches a reporting boundary. Keep intermediate values in their scalar form so they retain derivative information.

See [Automatic Differentiation](../risk/aad.md) for tape and sensitivity behavior.

## Instruments and trades

An instrument describes contractual economics. The base trait intentionally guarantees only an identifier:

```rust,ignore
pub trait Instrument: Send + Sync {
    fn identifier(&self) -> String;
}
```

Product-specific traits expose additional capabilities. Examples include leg access, currency, discounting index, strike, or maturity. Pricers combine the capability traits required by their valuation method, and the base `Instrument` trait remains focused on identity.

Instruments are normally created with `Make*` builders:

```rust,ignore
let swap = MakeSwap::<DualFwd>::default()
    .with_identifier("USD_IRS_5Y".to_string())
    .with_start_date(start_date)
    .with_maturity_date(maturity_date)
    .with_notional(10_000_000.0)
    .with_fixed_rate(0.03)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_rate_definition(rate_definition)
    .build()?;
```

Builders collect required and optional fields, apply defaults, construct schedules and legs, and return `QSError` for invalid or missing inputs. This is preferable to calling long positional constructors in application code.

A trade adds position-level information:

```rust,ignore
pub trait Trade<I: Instrument>: Send + Sync {
    fn instrument(&self) -> &I;
    fn trade_date(&self) -> Date;
    fn side(&self) -> Side;
}

pub enum Side {
    PayShort,    // sign = -1
    LongReceive, // sign = +1
}
```

The exact meaning of the side follows the product. For a vanilla swap, `LongReceive` receives the fixed leg and pays the floating leg. Pricers generally accept the trade type, which supplies both contract and position metadata.

## Market data and the pricing context

Market data is split between raw observations and constructed valuation objects.

| Layer | Main types | Responsibility |
| --- | --- | --- |
| Raw data | `QuoteStore`, `FixingStore`, `FxStore` | Quotes, historical fixings, and spot FX observations |
| Configuration | Curve, volatility, credit, and simulation configurations | Instructions for constructing market objects |
| Constructed data | `ConstructedElementStore` | Discount, dividend, and credit curves, volatility surfaces and cubes, and simulations |
| Orchestration | `PricingContext` | Owns the market state and serves pricer requests |

`MarketIndex` is the key connecting products to market objects. A SOFR floating leg requests data under `MarketIndex::SOFR`. The context must contain or construct the corresponding curve.

There are two common ways to prepare a context.

### Configuration-driven construction

Provide quotes and configurations, then initialize once:

```rust,ignore
let mut context = PricingContext::new()
    .with_quote_store(quotes)
    .with_fixing_store(fixings)
    .with_curve_configurations(curve_configurations)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR);

context.initialize()?;
```

`initialize()` applies scenarios and builds configured curves, credit curves, volatility objects, and simulations in dependency order.

### Direct construction

Small applications and tests can create market objects themselves and insert them into a `ConstructedElementStore`. In that case, `initialize()` is unnecessary because the objects already exist. The [first swap](first-swap.md) uses this route.

### Request and response boundary

Pricers declare a `MarketDataRequest`, and a `MarketDataProvider` returns the requested subset as `MarketData`. This boundary gives the pricer a focused view of `PricingContext`:

```rust,ignore
pub trait MarketDataProvider {
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData>;
    fn evaluation_date(&self) -> Date;
}
```

This boundary keeps pricing logic independent of how the market was assembled. It also makes focused tests possible with a small provider that returns hand-built data.

## Pricers and calculation requests

A pricer binds one trade type to one pricing methodology:

```rust,ignore
pub trait Pricer: Send + Sync {
    type Item;
    type Policy: ?Sized + Send + Sync;

    fn evaluate(
        &self,
        trade: &Self::Item,
        requests: &[Request],
        context: &impl MarketDataProvider,
    ) -> Result<EvaluationResults>;

    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest>;
    fn set_discount_policy(&mut self, policy: Box<Self::Policy>);
    fn discount_policy(&self) -> Option<&Self::Policy>;
}
```

There are two different request concepts:

- `MarketDataRequest` is produced by the pricer and describes required market inputs.
- `Request` is supplied by the caller and describes desired outputs.

```rust,ignore
pub enum Request {
    Value,
    YieldToMaturity,
    ModifiedDuration,
    Sensitivities,
    Cashflows,
    FairRate,
}
```

Support is pricer-specific. Request the outputs listed for that pricer in the [pricing overview](../pricing/overview.md). A pricer populates the fields it supports, and unsupported fields remain empty.

When several outputs share valuation work, pricers can calculate the common state once. For example, `DiscountedCashflowPricer` prepares value state once for value, cashflow, and sensitivity requests submitted in the same call.

## Results

`EvaluationResults` is a non-generic reporting envelope. Its fields are optional because the caller chooses which calculations to request:

```rust,ignore
let results = pricer.evaluate(
    &trade,
    &[Request::Value, Request::Cashflows],
    &context,
)?;

if let Some(npv) = results.price() {
    println!("NPV: {npv:.2}");
}

if let Some(cashflows) = results.cashflows() {
    for (date, amount) in cashflows
        .payment_dates()
        .iter()
        .zip(cashflows.amounts())
    {
        println!("{date}: {amount:.2}");
    }
}
```

The main result types are:

| Type | Contents |
| --- | --- |
| `EvaluationResults` | Optional price, fair rate, sensitivities, and cashflows exposed through public getters |
| `SensitivityMap` | Parallel market-pillar labels and NPV derivatives |
| `CashflowsTable` | Column-oriented payment dates, types, amounts, fixings, accrual periods, currencies, leg indices, and optional strikes |

Check the `Option` associated with each submitted request. A populated variant confirms that the corresponding calculation was performed.

## Discount policies

Discounting rules depend on collateral, currency, asset class, and sometimes an issuer curve. A `DiscountPolicy` maps a discountable object to the curve index selected by those rules:

```rust,ignore
pub trait DiscountPolicy: Send + Sync {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex>;
    fn discount_indices(&self) -> Vec<MarketIndex>;
}
```

Important implementations include:

- `SingleCurveCSADiscountPolicy`, for collateralized discounting under one remuneration index and currency.
- `FixedIncomeDiscountPolicy`, which can prefer an instrument's own index or use a configured risk-free index by currency.

Without an explicit policy, `DiscountedCashflowPricer` uses its default curve-resolution rules. `NettingSet` also owns a discount policy so exposure and XVA preprocessing can resolve discount curves consistently.

## Curves, volatility, and models

The common rate-curve abstraction is `InterestRatesTermStructure<T>`. It provides discount factors, forward rates, dates, nodes, and day-count information. Principal implementations are:

- `FlatForwardTermStructure<T>` for a constant rate.
- `DiscountTermStructure<T>` for an interpolated discount-factor curve.

Volatility surfaces resolve expiry and strike coordinates. Volatility cubes add tenor. Model and simulation APIs consume the constructed curve and volatility interfaces, whose builders retain the links to source quotes.

Use the dedicated chapters for domain behavior:

- [Yield curves](../curves/overview.md)
- [Volatility](../curves/volatility.md)
- [LGM simulation](../simulation/lgm.md)
- [XVA](../xva/overview.md)

## Static and dynamic dispatch

Direct use of a concrete pricer gives compile-time type checking and is the simplest option:

```rust,ignore
let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
let results = pricer.evaluate(&trade, &[Request::Value], &context)?;
```

Applications pricing heterogeneous portfolios can register pricers in `Evaluator`. It stores `ErasedPricer` implementations by the trade's `TypeId` and performs the downcast at runtime:

```rust,ignore
use std::{any::{Any, TypeId}, collections::HashMap};
use quantsupport::core::{evaluator::Evaluator, pricer::ErasedPricer};

let mut pricers: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
pricers.insert(
    TypeId::of::<SwapTrade<DualFwd>>(),
    Box::new(DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()),
);

let evaluator = Evaluator::new(pricers);
let results = evaluator.evaluate(
    &trade as &dyn Any,
    &[Request::Value],
    &context,
)?;
```

Use direct dispatch for isolated pricing and generic library code. Use `Evaluator` when the trade type is known only at runtime.

## Errors

Public fallible APIs return the crate alias:

```rust,ignore
pub type Result<T> = std::result::Result<T, QSError>;
```

Common error categories are:

| Category | Typical cause |
| --- | --- |
| `ValueNotSetErr` | A required builder input is absent |
| `NotFoundErr` | A required curve, fixing, quote, model, or pricer is unavailable |
| `InvalidValueErr` | Inputs are inconsistent or outside the accepted domain |
| `InterpolationErr` / `NodeError` | Curve or surface construction/evaluation failed |
| `SolverErr` | Calibration or root finding failed |
| `TapeError` / `DualFwdError` | Automatic-differentiation state is invalid |
| Parsing and serialization errors | External identifiers or data failed decoding |

Use `?` to propagate errors and add application context at system boundaries. Avoid treating a missing optional result as an error unless that result was required by the workflow.

## Time conventions

Time types are shared across instruments, curves, and models:

| Type | Role |
| --- | --- |
| `Date` | Calendar date and date arithmetic |
| `Period` / `TimeUnit` | Relative terms such as three months or five years |
| `DayCounter` | Converts date intervals to year fractions |
| `Frequency` | Coupon, compounding, or schedule frequency |
| `Calendar` | Holiday and business-day rules |
| `BusinessDayConvention` | Adjustment rule applied to a date outside the business calendar |
| `DateGenerationRule` | Forward, backward, IMM, CDS, and related schedule rules |
| `MakeSchedule` | Builds explicit date schedules |

These conventions are valuation inputs. A coupon's day count, its payment frequency, and a curve's compounding convention are separate explicit choices.

## Extending the library

The architecture supports extension through focused traits. Choosing the
narrowest boundary keeps a new component reusable and prevents product logic
from becoming coupled to a particular market-loading or reporting workflow.

Add functionality at the narrowest suitable boundary:

- New contract: implement `Instrument` and normally provide a `Make*` builder and trade wrapper.
- New pricing method: implement `Pricer` and declare market dependencies in `market_data_request()`.
- New curve: implement `InterestRatesTermStructure<T>` and the pillar traits required by its use case.
- New discounting convention: implement `DiscountPolicy`.
- New portfolio dispatch entry: register the pricer with `Evaluator` under the trade's `TypeId`.

Keep economic definitions in instruments, market lookup in providers, numerical valuation in pricers, and presentation outside `EvaluationResults`. Maintaining those boundaries is what lets the same products participate in direct pricing, calibration, simulation, and XVA workflows.

## Putting the API together

The Rust API is built around explicit ownership of responsibilities. Generic
scalar types carry values and derivatives, builders validate contracts,
providers resolve market data, pricers answer calculation requests, and result
types form the reporting boundary. Once these roles are clear, the larger
library becomes a composition of small interfaces with well-defined links.

The next conceptual chapters follow data through those interfaces. They begin
with the architecture as a whole, then examine market data, the pricing
context, and the instrument hierarchy individually.
