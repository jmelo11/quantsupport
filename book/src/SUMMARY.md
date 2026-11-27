# Summary

[QuantSupport](introduction.md)

# Using the library

- [Installation](getting-started/installation.md)
- [A first valuation](getting-started/first-swap.md)
- [Library workflow](getting-started/rust-api.md)

# Contracts and market state

- [Cashflows, contracts and positions](concepts/instruments.md)
- [Building instruments explicitly](concepts/instrument-resolution.md)
- [Market data and historical markets](concepts/market-data.md)
- [Constructing and querying a market](concepts/pricing-context.md)

# Market construction

- [Discount factors and rate conventions](curves/overview.md)
- [Calibration and dependent curves](curves/bootstrapping.md)
- [Volatility markets and model calibration](curves/volatility.md)

# Valuation

- [Swaps and discounted cashflows](pricing/swaps.md)
- [Cross-currency contracts](pricing/cross-currency.md)
- [Bonds and issuer optionality](pricing/bonds.md)
- [Loans, prepayment and credit](pricing/loans.md)
- [Interest-rate options](pricing/caps-floors.md)
- [Equity and FX options](pricing/options.md)

# Risk

- [Differentiation and market sensitivities](risk/aad.md)
- [Scenarios and full revaluation](risk/scenarios.md)

# Models and scripting

- [Stochastic models and simulation](simulation/monte-carlo.md)
- [Scripting language](scripting/language.md)
- [Script engine and exposure claims](scripting/engine.md)

# Exposure and valuation adjustments

- [Claims, exposure and the XVA engine](simulation/exposure.md)
- [Netting and collateral](xva/netting-csa.md)
- [Credit, funding and XVA risk](xva/cva-dva-fva.md)

# Library implementation

- [Inside automatic differentiation and the JIT](implementation/ad-jit.md)

# Supporting material

- [Serializable configuration](reference/configuration.md)
- [Example programs](reference/examples.md)
