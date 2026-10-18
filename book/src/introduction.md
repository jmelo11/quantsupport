# QuantSupport

QuantSupport (QS) is a quantitative finance library written in Rust, with
Python and Excel bindings. It supports instrument valuation across several
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

The Python package exposes native instruments and typed market configurations,
with results available as scalar properties and pandas tables. Its context
manager controls the lifetime of constructed markets and differentiation
state. [Working in Python](python/getting-started.md) provides a complete
valuation and introduces scenarios, historical markets and scripting.

The Excel add-in provides worksheet functions for market construction,
valuation and risk. Formulas return handles to native objects, allowing a
workbook to express the dependencies between inputs and results.
[Working in Excel](excel/workbook.md) explains this workflow and its
recalculation rules. The add-in requires 64-bit Windows desktop Excel.
