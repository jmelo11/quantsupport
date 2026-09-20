# Pricing Context

`PricingContext` is the single object most users interact with. It collects raw market data and configuration, builds every derived market object once in `initialize()`, and then acts as the `MarketDataProvider` that pricers, the scripting engine and the XVA engine query.

## Building a context

A context is assembled in two phases. Builder methods collect observations and
construction instructions, then `initialize()` turns that description into a
usable market. The following chain makes every input visible at the call site.

```rust,ignore
use quantsupport::prelude::*;

let mut ctx = PricingContext::new()
    .with_quote_store(quote_store)                          // required: prices + reference date
    .with_fixing_store(fixings)                             // optional: historical fixings
    .with_fx_store(fx_store)                                // optional: spot FX (needed for Collateral curves)
    .with_base_currency(Currency::USD)                      // CSA currency, default USD
    .with_base_index(MarketIndex::SOFR)                     // CSA discount index, default SOFR
    .with_curve_configurations(curve_specs)                 // Vec<CurveConfiguration>
    .with_credit_curve_configurations(credit_specs)         // Vec<CreditCurveConfiguration>
    .with_volatility_surface_configurations(surface_specs)  // Vec<VolatilitySurfaceConfiguration>
    .with_volatility_cube_configurations(cube_specs)        // Vec<VolatilityCubeConfiguration>
    .with_simulation_configurations(sim_specs)              // Vec<SimulationConfiguration>
    .with_scenarios(vec![Scenario::new("SOFR", 0.0001, ScenarioType::Absolute)]);

ctx.initialize()?;
```

Before the final call, the context owns its inputs. Expensive market
construction begins in `initialize()`, giving an application a clear point at
which to validate or modify configuration.

All `with_*` methods consume and return `Self`. `evaluation_date()` takes its value from `quote_store.reference_date()`, giving observations and constructed elements one shared date.

### Read accessors

After construction, accessors expose the inputs and results without
transferring ownership. The table distinguishes the shocked quote view from
the original observations and shows where initialized elements are stored.

| Method                                                                                                                                                              | Returns                                                           |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `quote_store()`                                                                                                                                                     | shocked store if scenarios are attached, otherwise the base store |
| `base_quote_store()`                                                                                                                                                | the unshocked store                                               |
| `fixing_store()`, `fx_store()`                                                                                                                                      | the stores as given                                               |
| `scenarios()`                                                                                                                                                       | `&Vec<Scenario>`                                                  |
| `base_currency()`, `base_index()`                                                                                                                                   | CSA currency / index                                              |
| `curve_configurations()`, `credit_curve_configurations()`, `volatility_surface_configurations()`, `volatility_cube_configurations()`, `simulation_configurations()` | the configuration vectors                                         |
| `constructed_elements()` / `constructed_elements_mut()`                                                                                                             | the `ConstructedElementStore` populated by `initialize()`         |
| `evaluation_date()`                                                                                                                                                 | reference date of the quote store                                 |

Application pricing normally uses the provider interface described below.
Direct access is valuable for diagnostics, calibration reports, and tests that
inspect a particular constructed object.

## What `initialize()` does

Initialization follows dependency order because later objects consume earlier
ones. The method has a small public signature. The sequence below
explains the work represented by that call.

```rust,ignore
pub fn initialize(&mut self) -> Result<()>
```

1. **Scenarios.** If `scenarios` is non-empty, clone the quote store and apply each `Scenario` in order. A scenario that matches no quote is an error. Everything below reads the shocked copy.
2. **Discount curves.** `MultiCurveBootstrapper::new(curve_configurations, BootstrapDiscountPolicy::new(base_index, base_currency)).with_fx_store(fx_store).bootstrap(quote_store, Level::Mid)`. Each resulting `DiscountCurveElement` is inserted under its `MarketIndex`.
3. **Credit curves.** `CreditCurveBootstrapper::bootstrap(quote_store, Level::Mid, discount_curves)` — CDS premium and protection legs are discounted on the curves from step 2.
4. **Volatility surfaces.** `VolatilitySurfaceBuilder::build(quote_store, Level::Mid)`.
5. **Volatility cubes.** `VolatilityCubeBuilder::build(quote_store, Level::Mid)`.
6. **Simulations.** `SimulationBuilder::build(constructed_elements, quote_store, fixing_store, Level::Mid)` — runs last so models can calibrate to the surfaces/cubes and diffuse the bootstrapped curves.

Steps 3–6 are skipped when the corresponding configuration vector is empty. Calling `initialize()` twice rebuilds everything from the (possibly shocked) quotes.

This ordering guarantees that every calibrator sees a complete set of its
dependencies. It also lets a scenario propagate consistently through curves,
volatility markets, and simulations.

## Serving market data to pricers

`PricingContext` implements `MarketDataProvider`:

```rust,ignore
pub trait MarketDataProvider {
    fn evaluation_date(&self) -> Date;
    fn handle_request(&self, request: &MarketDataRequest) -> Result<MarketData>;
}
```

The provider boundary turns a broad context into a focused response. A pricer
declares its needs first, which makes missing data visible before its valuation
formula begins.

A pricer first calls `market_data_request(&trade)` to describe what it needs, then the context resolves it:

```rust,ignore
pub struct MarketDataRequest {                 // all fields optional
    constructed_elements_request: Option<Vec<ConstructedElementRequest>>,
    fixings_request: Option<Vec<FixingRequest>>,
    fx_request: Option<Vec<FxRequest>>,
}
pub enum ConstructedElementRequest {
    DiscountCurve { market_index }, DividendCurve { market_index }, CreditCurve { market_index },
    VolatilitySurface { market_index }, VolatilityCube { market_index }, Simulation { market_index },
}
```

`handle_request` copies the requested elements into a fresh `ConstructedElementStore`, gathers the fixings (`MarketData::fixings()` is a `HashMap<MarketIndex, BTreeMap<Date, f64>>`), and attaches the `FxStore`. A missing element produces `QSError::NotFoundErr("Discount curve not found for index …")` — the most common error when a trade references an index without a curve configuration.

Because pricers only see `MarketData`, you can bypass the context entirely in tests by constructing `MarketData::new(fixings, constructed_elements).with_fx_store(fx)` and implementing `MarketDataProvider` on a small struct, or by populating `constructed_elements_mut()` directly with hand-built curves as the scripting example does:

```rust,ignore
let mut store = ConstructedElementStore::default();
store.discount_curves_mut().insert(
    MarketIndex::SOFR,
    DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
);
let ctx = PricingContext::new()
    .with_quote_store(QuoteStore::new(ref_date))
    .with_fixing_store(FixingStore::default())
    .with_constructed_elements(store)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR);
// The directly inserted curve makes the context ready for pricing.
```

This direct-construction route is useful for unit tests and focused examples. Configuration-driven applications call `initialize()` so the context can build and link the same element types from observable inputs.

## Scenarios

Scenarios modify observable quotes and ask the context to rebuild the market.
This provides a full revaluation path for large or non-linear shocks.

`Scenario::new(target, shock, ScenarioType::{Absolute, Relative})` shocks quotes before bootstrapping:

- `Absolute` adds `shock` to the quote, where `0.0001` is one basis point. `Relative` multiplies by `1 + shock`.
- `target` is a full identifier (`"OIS_USD_SOFR_5Y"`, one key-rate bump) or a segment selector: every underscore-separated segment of the target must appear among the identifier's segments. `"SOFR"` shocks all SOFR quotes (parallel shift), `"OIS_USD_SOFR"` all USD SOFR OIS pillars, `"Swaption_USD"` the USD swaption vol cube, `"CapletFloorlet_USD_SOFR"` the caplet surface.
- `scenario.apply(&mut store)` returns the number of quotes shocked and errors when zero matched.

Since all curves, volatility objects, and simulations are rebuilt from shocked quotes, a scenario performs full repricing. Use AAD sensitivities (`Request::Sensitivities`) for first-order risk. Use scenarios for stress tests, bump-and-reprice validation, and nonlinear moves. See [Scenarios](../risk/scenarios.md).

## Evaluating trades

Once initialization succeeds, the context is ready to serve deterministic and
simulation-based consumers. The caller still chooses the pricer and requested
outputs. The context resolves their market dependencies.

The chapter [Rust API](../getting-started/rust-api.md) shows the pricer-based flow (`DiscountedCashflowPricer::new().evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)`), and [Pricing Overview](../pricing/overview.md) lists which pricer handles which trade type and the `Evaluator` for heterogeneous portfolios. The XVA engine takes the same context: `XvaEngine::new(&ctx, config)?.run(&mut netting_sets)`.

## Python

The Python wrapper preserves the same lifecycle: one object owns the market,
and initialization builds its derived state. Keyword arguments mirror the
Rust builder inputs so configuration can be shared across both interfaces.

The binding exposes the same object with keyword arguments matching the builders: `PricingContext(quotes, curves, fixings=None, fx=None, volatility_surfaces=None, volatility_cubes=None, simulations=None, discounting=None, scenarios=None)`. Initialization runs during construction, and the object acts as a context manager that clears the AD tape on exit. See the generated Python API documentation for the exported types.

## Context lifecycle in practice

A pricing context moves from collected inputs to an initialized market and
then serves valuation requests until the inputs change. Rebuilding after a
scenario or market update preserves that lifecycle and keeps derived objects
synchronized. The context therefore coordinates the workflow, and economic
logic remains in instruments and numerical logic remains in pricers.
