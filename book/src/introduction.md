# QuantSupport

QuantSupport is a quantitative-finance library implemented in Rust and used
directly, through Python, or through an Excel add-in. Its subject is the
construction of a market and the valuation of contracts against that market:
interest-rate and credit curves, volatility markets, deterministic pricing,
market sensitivities, stochastic payoffs, exposure and valuation adjustments.
These are not independent calculators. They share contract representations,
market identities and numerical conventions.

The central distinction is between an **observation** and a **model of the
market**. A swap quote is an observed par rate; a discount factor is a calibrated
quantity that lets the library price payments between quoted maturities.
Similarly, an implied-volatility surface represents option observations, whereas
a short-rate model specifies how future rates evolve. Keeping these roles
separate makes both calibration and risk interpretable.

For observations \\(q\\), a constructed market \\(M(q)\\), and a position \\(c\\),
valuation can be written schematically as

\\[
V(c;q)=\mathcal P(c,M(q)).
\\]

The construction map includes interpolation and numerical calibration.
Sensitivities must therefore account for its effect as well as the direct
pricing formula. Scenarios evaluate the composition again at changed inputs.
Simulation extends the same construction into future market states; claim
aggregation then adds netting, collateral, credit and funding assumptions.

This book develops that connected system rather than cataloging declarations.
The opening chapters explain the three interfaces and a complete valuation.
Contracts and market-state chapters establish the shared vocabulary. Curve and
volatility construction lead into product valuation and risk. The remaining
chapters connect stochastic models and dated payoff programs to exposure and
XVA. Configuration and executable examples support the discussion without
replacing it.

## One core, different working environments

Rust exposes ownership, numerical scalar types and trait boundaries directly.
It is the natural interface for extending a pricer, market provider or simulation
model. Python provides native financial objects, typed construction inputs,
context-managed market lifetimes and pandas reporting. Excel expresses
construction and valuation as a worksheet dependency graph whose cells carry
revisioned handles to native objects.

These interfaces share a core, but their capabilities are not identical. The
Python scripting service, for example, returns averaged variables rather than
every low-level Rust scripting output. Excel's process-local handles are not
serialized markets. Likewise, not every simulation stored in a `DualFwd`
container retains derivatives of the model that generated it. Those boundaries
are stated where they affect interpretation.

## Numerical interpretation

Rates and spreads normally use decimal units: `0.04` means 4%, and `0.0001`
means one basis point. A quoted rate is incomplete without its day count and
compounding convention. Payment dates, fixing dates, observation periods,
collateral currency and position direction are also valuation inputs, not
presentation choices.

The library's generic `Scalar` abstraction permits ordinary numerical values
and automatic-differentiation values in many mathematical components. Standard
market construction and deterministic pricing are predominantly
`DualFwd`-oriented. Automatic differentiation follows the implemented numerical
program; it does not supply omitted model dependencies, remove discretization
error or make discontinuous payoffs smooth.

The book distinguishes implemented behavior from idealized formulas. In
particular, it identifies fixed calibration assumptions, numerical-path
limitations, and approximations in exposure and funding calculations. This
discipline is necessary before a computed number can become a price or a useful
risk measure.
