# Exposure Simulation

Exposure measures future portfolio value by counterparty and netting agreement. Trades are decomposed into `ContingentClaim` values, preprocessed for fixings and repeated structure, then evaluated across model paths and dates.

At date \(t\), positive and negative exposure are commonly summarized as

\[
\operatorname{EPE}(t)=\mathbb{E}[\max(V_t,0)],\qquad
\operatorname{ENE}(t)=\mathbb{E}[\min(V_t,0)].
\]

QuantSupport's workflow uses `IntoContingentClaims`, preprocessing visitors, `ExposureEvaluator`, and an `NpvCube`. `ExposureResult` provides exposure profiles and values consumed by XVA aggregators.

Simulation dates must include economically relevant payment, fixing, exercise, and margin dates. Claim compression improves performance but must preserve net cashflows by date, currency, and state dependency.

Exposure is a portfolio calculation: aggregate trades under their `NettingSet` before applying positive/negative parts. Applying exposure separately per trade overstates risk by discarding netting.

Run `cargo run -p pfe --release` for a complete multi-currency example.
