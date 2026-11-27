# QuantSupport

QuantSupport (QS) is a quantitative finance library written in Rust. It provides:

- valuation of instruments across several asset classes;
- market sensitivities computed with automatic differentiation (AD);
- interest-rate and credit curve construction;
- volatility surfaces and cubes;
- Monte Carlo simulation;
- valuation adjustments such as CVA and FVA.

The library is built around a few objects:

- **Market data** is what you observe: quotes and fixings.
- **Market objects** are built from that data, for example bootstrapped curves
  and implied-volatility surfaces.
- **Instruments** describe the contract: its payments and the rates or prices
  it observes.
- **Trades** add the position to an instrument: direction, notional, and trade
  date.
- **Pricers** value a trade with the market data and numerical method that
  suit the contract.

## Using the library

The [library workflow guide](getting-started/rust-api.md) introduces the main types and
the interfaces you implement to add your own pricers, market data sources, or
simulation models. The [first valuation](getting-started/first-swap.md) builds
and prices a receive-fixed swap.

## Library implementation

[Inside automatic differentiation and the JIT](implementation/ad-jit.md)
explains how the library computes derivatives:

- how formulas are recorded as operation graphs;
- how reverse-mode derivatives are generated;
- how Cranelift compiles these graphs into reusable native code.

It also covers branching, curve calibration, and parallel execution.
