# QuantSupport

QuantSupport (QS) is a quantitative finance library written in Rust. It supports instrument valuation across several
asset classes, market sensitivities using automatic differentiation,
interest-rate and credit curve construction, volatility markets, Monte Carlo
simulation, and valuation adjustments such as CVA and FVA.

The library separates observable market data, such as quotes and fixings,
from objects constructed from those observations, such as bootstrapped curves
and implied-volatility surfaces. Instruments describe contractual payments
and observation rules. Trades combine those instruments with position terms,
including direction, notional and trade date. A pricer then values the trade
using the market data and numerical method appropriate to the contract.

## Using the library

Rust provides direct access to the financial types and the interfaces used to
extend pricing, market-data and simulation components. The
[Rust guide](getting-started/rust-api.md) introduces these interfaces, and the
[first valuation](getting-started/first-swap.md) constructs and prices a
receive-fixed swap.

## Library implementation

[Inside automatic differentiation and the JIT](implementation/ad-jit.md)
explains how scalar formulas become operation graphs, how reverse derivatives
are generated, and how Cranelift compiles reusable numerical programs. It also
covers capture ownership, branching, calibration maps and parallel execution.
