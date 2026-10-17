# Pricing Context

`PricingContext` is the single object most users interact with. It collects raw market data and configuration, constructs derived objects eagerly in `initialize()` or lazily on request, and then acts as the `MarketDataProvider` that pricers, the scripting engine and the XVA engine query.

## Building a context

A context is assembled in two phases. Builder methods collect observations and
construction instructions, then `initialize()` turns that description into a
usable market. The following chain makes every input visible at the call site.

```rust,ignore
use quantsupport::prelude::*;

let mut ctx = PricingContext::new()
    .with_quote_store(quote_store)                          // required: prices + reference date
    .with_quote_level(Level::Mid)                           // optional: Mid (default), Bid, or Ask
    .with_fixing_store(fixings)                             // optional: historical fixings
    .with_fx_store(fx_store)                                // optional: spot FX (needed for Collateral curves)
    .with_base_currency(Currency::USD)                      // CSA currency, default USD
    .with_base_index(MarketIndex::SOFR)                     // CSA discount index, default SOFR
    .with_curve_configurations(curve_specs)                 // Vec<CurveConfiguration>
    .with_credit_curve_configurations(credit_specs)         // Vec<CreditCurveConfiguration>
    .with_volatility_surface_configurations(surface_specs)  // Vec<VolatilitySurfaceConfiguration>
    .with_volatility_cube_configurations(cube_specs)        // Vec<VolatilityCubeConfiguration>
    .with_pricer_configurations(pricer_specs)               // Vec<PricerConfiguration>
    .with_simulation_configurations(sim_specs);             // Vec<SimulationConfiguration>

ctx.initialize()?;
```

Before the final call, the context owns its inputs. `initialize()` explicitly
constructs every configured market object. Applications can also pass the context
directly to pricers: market-data requests then construct and cache only the
required objects and their dependencies. `validate()` checks configuration
compatibility and duplicates without calibrating curves or generating paths.

`PricerConfiguration` is keyed by `InstrumentType`, so the same mechanism can
select an implementation for bonds, swaps, options, or future instrument
families. A configuration carries a typed `PricerImplementation` and typed
`ModelConfiguration` assignments by `MarketIndex`; valuation code does not
interpret arbitrary model or pricer strings. Duplicate configurations for one
instrument type are rejected during initialization.

All `with_*` methods consume and return `Self`. `evaluation_date()` takes its value from `quote_store.reference_date()`, giving observations and constructed elements one shared date.

Set `with_quote_level(Level::Bid)` or `with_quote_level(Level::Ask)` during setup
to choose which quote levels build the market. The choice applies to discount
and credit curves, volatility surfaces and cubes, simulations, and subsequent
bond, scripting, and XVA model calibration. Mid remains the default. A missing
requested level produces an error; no other level is substituted. After
changing the level on an existing Rust context, call `initialize()` to rebuild
its configured objects.

### Read accessors

After construction, accessors expose the inputs and results without
transferring ownership. Quotes always describe the base inputs; scenario views
apply temporary transformations through the provider interface.

| Method                                                                                                                                                              | Returns                                                           |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `quote_store()`                                                                                                                                                     | the original quote store |
| `quote_level()` | selected `Level` for construction and calibration |
| `fixing_store()`, `fx_store()`                                                                                                                                      | the stores as given                                               |
| `base_currency()`, `base_index()`                                                                                                                                   | CSA currency / index                                              |
| `curve_configurations()`, `credit_curve_configurations()`, `volatility_surface_configurations()`, `volatility_cube_configurations()`, `pricer_configurations()`, `simulation_configurations()` | the configuration vectors                                         |
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

1. **Pricer configuration.** Validate instrument/pricer compatibility, numerical controls, and the one-configuration-per-instrument rule.
2. **Discount curves.** `MultiCurveBootstrapper::new(curve_configurations, BootstrapDiscountPolicy::new(base_index, base_currency)).with_fx_store(fx_store).bootstrap(quote_store, quote_level)`. Each resulting `DiscountCurveElement` is inserted under its `MarketIndex`.
3. **Credit curves.** `CreditCurveBootstrapper::bootstrap(quote_store, quote_level, discount_curves)` — CDS premium and protection legs are discounted on the curves from step 2.
4. **Volatility surfaces.** `VolatilitySurfaceBuilder::build(quote_store, quote_level)`.
5. **Volatility cubes.** `VolatilityCubeBuilder::build(quote_store, quote_level)`.
6. **Simulations.** `SimulationBuilder::build(constructed_elements, quote_store, fixing_store, quote_level)` — runs last so models can calibrate to the surfaces/cubes and diffuse the bootstrapped curves.

Steps 3–6 are skipped when the corresponding configuration vector is empty. Calling `initialize()` twice rebuilds everything from the original quotes.

This ordering guarantees that every calibrator sees a complete set of its
dependencies. A [scenario](../risk/scenarios.md) returns an independent, request-driven market
view, preserving the base context. Python enables lazy construction on context
entry and uses `with context, scenario:` for temporary shocks.

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
    pricer_configuration_request: Option<Vec<InstrumentType>>,
    calibration_request: Option<ModelCalibrationConfiguration>,
}
pub enum ConstructedElementRequest {
    DiscountCurve { market_index }, DividendCurve { market_index },
    OptionalDividendCurve { market_index }, CreditCurve { market_index },
    VolatilitySurface { market_index }, VolatilityCube { market_index }, Simulation { market_index },
}
```

When requested, the response also carries the configuration selected by the
typed `InstrumentType`. This lets a pricer obtain its implementation controls
and model assignments through the same narrow provider boundary as its curve
and fixing dependencies.

`OptionalDividendCurve` allows a pricer to treat an absent dividend curve as zero dividends. A calibration request selects source quote details for a model basket. The response exposes them through `MarketData::calibration_quotes()` and carries the context's selected `quote_level()`, together with the requested surface or cube and curves. A custom provider for calibrated pricing must supply these inputs too.

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

The binding exposes the same object with keyword arguments matching the builders: `PricingContext(quotes, curves, fixings=None, fx=None, volatility_surfaces=None, volatility_cubes=None, pricer_configurations=None, simulations=None, csa_policy=None, fixed_income_policy=None, scenarios=None, quote_level=None)`. Set `quote_level=qs.Level.Bid` or `quote_level="Ask"`; names are case-insensitive, and omitting the setting uses `qs.Level.Mid`. The read-only `context.quote_level` property returns the selected enum.

Enter `with context:` to initialize the market automatically and start the AD
tape. Exiting the block clears the constructed market and resets the tape,
including when an exception occurs. An explicit `initialize()` call is only
needed when using the context without a `with` block.

Python-defined pricers can be registered with `register_pricer(TradeType, pricer)` and then participate in the same `context.evaluate(...)` workflow; they receive the initialized context as their market-data provider. The standalone Python `Evaluator` provides the corresponding type-based dispatch for custom providers and heterogeneous portfolios.

## Context lifecycle in practice

A pricing context moves from collected inputs to an initialized market and
then serves valuation requests until the inputs change. Rebuilding after a
scenario or market update preserves that lifecycle and keeps derived objects
synchronized. The context therefore coordinates the workflow, and economic
logic remains in instruments and numerical logic remains in pricers.

The Python `csa_policy` input binds Rust's `SingleCurveCSADiscountPolicy`, with
explicit `collateral_currency` and `collateral_index` fields. Rust also offers
`with_csa_policy(policy)` and `csa_policy()`. Python installs that policy on
native derivative pricers, while `fixed_income_policy` controls bond, deposit
and FRN curve selection. `CsaTerms` belongs to netting/XVA and additionally
includes counterparty credit and funding assumptions. The legacy Rust base
currency/index accessors continue to describe the base collateral curve.
