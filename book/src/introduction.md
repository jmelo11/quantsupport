# Introduction

QuantSupport is a Rust library for building market data, pricing derivatives, measuring risk with automatic differentiation, simulating exposure, and computing XVA.

Everything is organised around one flow:

1. Load observable **quotes**, **fixings**, and **FX rates** into `QuoteStore`, `FixingStore`, and `FxStore`.
2. Describe **curves**, **credit curves**, **volatility surfaces/cubes**, and **simulations** with serialisable configuration structs (`CurveConfiguration`, `CreditCurveConfiguration`, `VolatilitySurfaceConfiguration`, `VolatilityCubeConfiguration`, `SimulationConfiguration`).
3. Put them into a [`PricingContext`](concepts/pricing-context.md) and call `initialize()`, which bootstraps and builds every object in dependency order.
4. Build an **instrument** with a `Make*` builder and wrap it in a **trade** (`SwapTrade`, `FxForwardTrade`, …) that carries notional and side.
5. Ask a **pricer** (`DiscountedCashflowPricer`, `ClosedFormBlackCapPricer`, `FxOptionPricer`, …) for `Request::Value`, `FairRate`, `Cashflows`, or `Sensitivities`.
6. Reuse the same market for **scenarios** (`Scenario`), **scripted payoffs** (`ScriptEngine`), **simulation** (`LgmMarketModel`, `HullWhite`), and **XVA** (`XvaEngine`).

Curves, instruments, and pricers use the generic scalar parameter `T: Scalar`. `T = f64` carries plain numerical values. `T = DualFwd`, a reverse-mode tape over a second-order forward type, makes each price differentiable with respect to the quotes that built the market.

## Crate layout

The library is divided by responsibility so that users can locate a concept
before they need to understand its implementation. The following map starts
with numerical foundations, moves through market construction and pricing,
and ends with the portfolio workflows that reuse those components.

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

This division is also an architectural guide. Data stores own observations,
constructed elements own calibrated market objects, instruments own contract
terms, and pricers connect contracts to markets. Keeping those responsibilities
visible makes larger workflows easier to reason about.

## What the book covers

The chapters follow the same order in which a valuation system is normally
built. A new reader can proceed from installation to a first price, then add
market construction, risk, simulation, and XVA as those concerns become
relevant.

- **Getting Started** installs the crate and prices a first swap in Rust and Python.
- **Core Concepts** explains the market-data model, `PricingContext::initialize`, and how instruments, trades, pricers and results relate.
- **Curves and Market Data** covers term structures, multi-curve bootstrapping with dependencies, FX-implied collateral curves, and volatility objects.
- **Pricing** documents each pricer: which `Request`s it supports, the formulas it implements, and the builder fields it needs.
- **Risk** describes the AD machinery, quote-pillar sensitivities, and scenario shocks.
- **Scripting** documents the payoff language, event streams, `ScriptEngine`, and scripted products in XVA.
- **Models and Simulation** covers Hull-White and LGM calibration and Monte Carlo generation.
- **XVA** covers contingent-claim decomposition, netting sets and CSA terms, CVA/DVA/FVA, and AAD sensitivities of XVA measures.
- **Reference** lists the JSON schemas, the runnable examples, and a glossary.

Rust snippets marked `rust,ignore` illustrate focused parts of the library and example packages. The complete compiled programs are listed in [Examples](reference/examples.md).

## How to use this book

Readers building an application should begin with [Installation](getting-started/installation.md)
and [Your First Swap](getting-started/first-swap.md). Readers extending the
library can use the architecture and Rust API chapters to find the appropriate
trait boundary before moving to the domain-specific chapters. The reference
section is most useful after the concepts behind each configuration file are
understood.

By the end of the book, the opening six-step flow should read as one connected
system: observable data becomes a constructed market, the market supports
pricing and risk, and the same objects feed simulation and XVA.
