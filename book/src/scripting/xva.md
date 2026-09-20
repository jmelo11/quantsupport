# Scripted Products in XVA

`ScriptedProduct` implements `IntoContingentClaims`, which gives bespoke payoffs the same XVA entry point as swaps and cross-currency products. `XvaEngine`, `NettingSet`, and the aggregators operate on `ContingentClaim` values. A scripted claim identifies its valuation behavior through `ClaimEvaluationStrategy::Scripted { payoff }`.

This chapter follows that conversion, explains how the exposure evaluator replays a scripted payment, and compares a scripted swap with its native equivalent. It then shows how both forms can participate in one netting set.

## From script to claims

Claim conversion compiles the event stream, discovers every `pays` expression, and creates one claim per payment identity. A caller can perform the full conversion in one chain:

```rust,ignore
let scripted_claims = ScriptedProduct::new(
    "scripted_swap",
    scripted_swap_events()?,
    reference_date(),
    Currency::USD,
    MarketIndex::SOFR,
)?
.contingent_claims()?;
```

Each `pays` statement becomes one claim with unit notional, `LongReceive` side, and its payment id as `leg_id`. Direction and scale already live in the script expression. All claims share one `Arc<ScriptEngine>`, so parsing and indexing happen once for the whole product.

## How the exposure evaluator prices a scripted claim

During `XvaEngine::run`, the claim participates in the standard simulation and aggregation sequence:

1. `PreprocessorExecutor` collects `SimulationRequest`s from every claim. For scripted claims these are `ScriptedPayoff::simulation_requests()`, i.e. `ScriptEngine::model_requests()` — the discount factors, forward rates, FX rates and spots the script observes.
2. The LGM market model simulates those observables on every path and evaluation date.
3. At each valuation date \\(t_k\\), the exposure evaluator asks every live claim for its value. For a scripted claim, it calls `ScriptedPayoff::evaluate(valuation_date, responses)`. This method replays the compiled program and returns the numeraire-deflated value associated with that payment id. The live-claim schedule excludes payments settled before \\(t_k\\), which makes the exposure profile roll off over time.
4. The per-path NPVs are aggregated into `NpvCube`s and then into CVA/DVA/FVA by the configured aggregators.

The payoff is evaluated in `DualFwd`, so the engine's AAD pass carries scripted-claim XVA back to curve pillars and model parameters. Native and scripted claims therefore contribute to the same labelled sensitivity report.

## Worked comparison

The XVA example runs a native swap and its scripted representation through equivalent engine configurations. This controlled comparison checks the claim bridge, path valuation, aggregation, and sensitivities together. The core setup is:

```rust,ignore
fn csa_terms() -> CsaTerms {
    CsaTerms {
        collateral_index: MarketIndex::SOFR,
        collateral_currency: Currency::USD,
        credit_spread: 0.01,
        recovery: 0.4,
        funding_spread: 0.005,
        funding_spread_curve: None,
        funding_index: None,
        credit_index: None,
    }
}

let config = XvaEngineConfig {
    model_configs: vec![LgmModelConfig {
        market_index: MarketIndex::SOFR,
        lambda: Some(0.05),
        parameter_source: Some(ParameterSource::Fixed(
            GaussianRateModelParameters::new(0.01),
        )),
        driver: None,
    }],
    fx_configs: Vec::new(),
    n_paths: 1_000,
    seed: 42,
    frequency: Frequency::Quarterly,
};

let native_claims = native_swap()?.into_contingent_claims()?;
let scripted_claims = ScriptedProduct::new("scripted_swap", scripted_swap_events()?, ref_date, Currency::USD, MarketIndex::SOFR)?
    .contingent_claims()?;

let mut netting_sets = HashMap::from([("swap".to_string(), NettingSet::with_csa_terms(claims, csa_terms()))]);
let result: ExposureResult = XvaEngine::new(&context, config)?.run(&mut netting_sets)?;

let epe = result.cubes.iter().find(|c| c.trade_id == "swap").map(NpvCube::epe);
let sensitivities: BTreeMap<String, f64> = result.sensitivities.unwrap().into_iter().collect();
```

The full program creates one netting set for each representation and runs them independently. It prints both EPE vectors and the combined CVA/FVA sensitivities under curve and model labels. It then checks that maximum EPE and sensitivity differences are below `1e-8`. Run it with:

```bash
cargo run -p scripting-examples --bin xva
```

The command reports native and scripted profiles side by side, making any difference in claim conversion or path evaluation immediately visible.

## Mixing scripted and native trades

A netting set owns a `Vec<ContingentClaim>`, so claims produced by several product representations can be combined before collateral and exposure aggregation. This example groups an interest-rate swap, a cross-currency swap, and a scripted structured note:

```rust,ignore
let mut claims = irs_trade.into_contingent_claims()?;
claims.extend(xccy_trade.into_contingent_claims()?);
claims.extend(structured_note.contingent_claims()?);   // ScriptedProduct
netting_sets.insert("counterparty_A".into(), NettingSet::with_csa_terms(claims, csa));
```

Trade ids are preserved in the resulting `NpvCube` values. `result.cubes` can therefore display each trade's exposure contribution, and the aggregators use the total netted exposure for CVA, DVA, and FVA.

## Limitations

The current bridge has three operational boundaries that product authors should account for in their scripts and model setup:

- Scripted claims use `Side::LongReceive` with unit notional. Direction and notional belong in the script expression.
- The market model must be able to serve every request the script makes: `RateIndex("X", …)` needs a curve model for `MarketIndex::X`, `Spot("USD","CLP")` needs an FX model for CLP, `Spot("AAPL")` an equity model.
- Scripted-product construction is currently available through Rust. The Python bindings expose XVA for native trades.

## What to remember

The XVA bridge treats every scripted payment as a scheduled contingent claim backed by one shared compiled engine. Preprocessing derives simulation requests from the script, exposure evaluation replays each live payment on path responses, and aggregation combines the resulting values with native claims. Stable trade and payment ids retain product-level explainability throughout the calculation.
