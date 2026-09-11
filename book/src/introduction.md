# Introduction

QuantSupport is a Rust library for building market data, pricing derivatives, measuring risk with automatic differentiation, simulating exposure, and computing XVA. Python bindings built with PyO3 expose the configuration-driven parts of the same API.

Everything is organised around one flow:

1. Load observable **quotes**, **fixings**, and **FX rates** into `QuoteStore`, `FixingStore`, and `FxStore`.
2. Describe **curves**, **credit curves**, **volatility surfaces/cubes**, and **simulations** with serialisable configuration structs (`CurveConfiguration`, `CreditCurveConfiguration`, `VolatilitySurfaceConfiguration`, `VolatilityCubeConfiguration`, `SimulationConfiguration`).
3. Put them into a [`PricingContext`](concepts/pricing-context.md) and call `initialize()`, which bootstraps and builds every object in dependency order.
4. Build an **instrument** with a `Make*` builder and wrap it in a **trade** (`SwapTrade`, `FxForwardTrade`, …) that carries notional and side.
5. Ask a **pricer** (`DiscountedCashflowPricer`, `ClosedFormBlackCapPricer`, `FxOptionPricer`, …) for `Request::Value`, `FairRate`, `Cashflows`, or `Sensitivities`.
6. Reuse the same market for **scenarios** (`Scenario`), **scripted payoffs** (`ScriptEngine`), **simulation** (`LgmMarketModel`, `HullWhite`), and **XVA** (`XvaEngine`).

The single generic scalar parameter `T: Scalar` runs through curves, instruments and pricers. With `T = f64` you get plain numbers; with `T = DualFwd` (reverse-mode tape over a second-order forward type) every price is differentiable with respect to the quotes that built the market. This is why sensitivities never need a separate bump-and-reprice implementation.

## Crate layout

| Module                  | Contents                                                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ad`                    | `Tape`, `Dual<T>`, `Fwd<T>`, `DualFwd` and the `Scalar` trait                                                                                    |
| `core`                  | `PricingContext`, `ConstructedElementStore`, `Request`, `EvaluationResults`, `Trade`, `Side`, `Pricer`, `Evaluator`, discount policies           |
| `currencies`, `indices` | `Currency` and `MarketIndex` enums                                                                                                               |
| `quotes`                | `Quote`, `QuoteDetails`, `QuoteInstrument`, `QuoteStore`, `Scenario`, `FixingStore`, `FxStore`                                                   |
| `rates`                 | `DiscountTermStructure`, `FlatForwardTermStructure`, `RateDefinition`, `MultiCurveBootstrapper`, `CreditCurveBootstrapper`, curve configurations |
| `volatility`            | Surfaces, cubes, `VolatilityType`, `SmileType`, `Strike`, volatility sources                                                                     |
| `instruments`           | Instruments and `Make*` builders                                                                                                                 |
| `pricers`               | Concrete pricers                                                                                                                                 |
| `models`                | `HullWhite`, LGM components and `LgmMarketModel`, Brownian motion, Monte Carlo engine                                                            |
| `simulations`           | `SimulationConfiguration`, `SimulationBuilder`, `GeneratedMonteCarloSimulation`                                                                  |
| `scripting`             | Payoff language, `ScriptEngine`, `ScriptedProduct`                                                                                               |
| `xva`                   | Contingent claims, netting sets, CSA, `XvaEngine`, aggregators                                                                                   |
| `time`, `math`, `utils` | `Date`, `Period`, `Calendar`, schedules, interpolation, solvers, errors                                                                          |

`quantsupport::prelude::*` re-exports the types used in this book.

## What the book covers

- **Getting Started** installs the crate and prices a first swap in Rust and Python.
- **Core Concepts** explains the market-data model, `PricingContext::initialize`, and how instruments, trades, pricers and results relate.
- **Curves and Market Data** covers term structures, multi-curve bootstrapping with dependencies, FX-implied collateral curves, and volatility objects.
- **Pricing** documents each pricer: which `Request`s it supports, the formulas it implements, and the builder fields it needs.
- **Risk** describes the AD machinery, quote-pillar sensitivities, and scenario shocks.
- **Scripting** documents the payoff language, event streams, `ScriptEngine`, and scripted products in XVA.
- **Models and Simulation** covers Hull-White and LGM calibration and Monte Carlo generation.
- **XVA** covers contingent-claim decomposition, netting sets and CSA terms, CVA/DVA/FVA, and AAD sensitivities of XVA measures.
- **Reference** lists the JSON schemas, the runnable examples, and a glossary.

Rust snippets marked `rust,ignore` are extracted from the library and the `examples/` packages but are not compiled as doctests; the full programs are listed in [Examples](reference/examples.md).

Continue with [Installation](getting-started/installation.md).
