# XVA Overview

XVA adjusts a portfolio's clean value for counterparty credit, own credit, funding, and collateral effects. QuantSupport's high-level `XvaEngine` coordinates claim decomposition, preprocessing, LGM simulation, exposure aggregation, valuation adjustments, and sensitivities.

The main inputs are:

- an initialized `PricingContext`;
- `XvaEngineConfig` with rate and FX model configurations, paths, seed, and frequency;
- one or more `NettingSet` values;
- `CsaTerms` attached to each netting set.

The engine produces exposure profiles, an `NpvCube`, XVA values, and AD sensitivities. Factories such as `CreditCurveCvaFactory`, `FundingCurveFvaFactory`, and `PfeAggregatorFactory` separate simulation from measure-specific aggregation.

XVA is path- and portfolio-dependent. A clean-pricing curve setup is necessary but not sufficient: netting scope, collateral currency, credit spread, recovery, funding spread, simulation dates, and model calibration all affect results.

Run `cargo run -p cva --release` for the complete high-level workflow. The scripting comparison is available with `cargo run -p scripting_examples --bin xva`.
