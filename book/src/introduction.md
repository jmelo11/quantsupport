# QuantSupport

QuantSupport is a Rust library for building market data, pricing derivatives, measuring risk, simulating exposure, and calculating XVA. Python bindings expose the same configuration-driven workflows for research and integration.

The library is designed around one flow:

1. Load observable quotes, fixings, and FX rates.
2. Describe curves, volatility objects, and simulation models with serializable configuration.
3. Initialize a [`PricingContext`](concepts/pricing-context.md), which constructs market objects in dependency order.
4. Build an instrument and attach trade economics such as notional and side.
5. Ask a pricer for values, cashflows, fair rates, or quote-level sensitivities.
6. Apply scenarios or move the portfolio into simulation and XVA workflows.

The same market graph supports deterministic cashflow pricing, closed-form option models, Monte Carlo, automatic differentiation, and exposure simulation. This avoids a common source of inconsistency: maintaining separate market-data implementations for valuation and risk.

## What the book covers

The first chapters build and value a USD interest-rate swap. Later sections explain bootstrapping, multi-curve pricing, volatility, instrument-specific pricers, automatic differentiation, model calibration, simulation, and XVA.

The book favors complete workflows over exhaustive API listings. Public Rust items are documented on [docs.rs](https://docs.rs/quantsupport), while runnable programs live under the repository's `examples/` directory.

## Conventions

- Rates and volatilities are decimal values: `0.05` means 5%.
- Monetary values use the instrument or pricing-context currency.
- Dates are represented by `Date`; periods use values such as `"3M"` and `"5Y"`.
- Rust examples generally import `quantsupport::prelude::*`.
- Fallible Rust operations return `quantsupport::prelude::Result<T>`.

Start with [Installation](getting-started/installation.md), then work through [Your First Swap](getting-started/first-swap.md).
