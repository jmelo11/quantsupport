# Scripted Products in XVA

`ScriptedProduct` implements `IntoContingentClaims`, so a script drops into the XVA engine exactly like a `SwapTrade` or a cross-currency swap. Nothing in `XvaEngine`, `NettingSet` or the aggregators knows about scripts; they only see `ContingentClaim`s whose `evaluation_strategy` is `ClaimEvaluationStrategy::Scripted { payoff }`.

## From script to claims

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

Each `pays` statement becomes one claim (`notional = 1.0`, `side = LongReceive`, `leg_id = payment id`). The claims share a single `Arc<ScriptEngine>`, so the script is parsed and indexed once regardless of how many coupons it generates.

## How the exposure evaluator prices a scripted claim

During `XvaEngine::run`:

1. `PreprocessorExecutor` collects `SimulationRequest`s from every claim. For scripted claims these are `ScriptedPayoff::simulation_requests()`, i.e. `ScriptEngine::model_requests()` — the discount factors, forward rates, FX rates and spots the script observes.
2. The LGM market model simulates those observables on every path and evaluation date.
3. At each valuation date \(t_k\) the exposure evaluator asks every live claim for its value. For a scripted claim it calls `ScriptedPayoff::evaluate(valuation_date, responses)`, which replays the compiled script on that path's responses and returns the numeraire-deflated value of **that payment only**. Payments already settled before \(t_k\) are excluded automatically, so the exposure profile rolls off correctly.
4. The per-path NPVs are aggregated into `NpvCube`s and then into CVA/DVA/FVA by the configured aggregators.

Because the payoff is evaluated in `DualFwd`, the engine's AAD pass produces XVA sensitivities to curve pillars and model parameters for scripted claims with no extra work.

## Worked comparison

`examples/scripting/src/bin/xva.rs` runs the native and scripted swap through the same engine and asserts identical results:

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
        sigma: Some(0.01),
        volatility: None,
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

The binary prints the EPE vector for both routes and a table of combined CVA/FVA sensitivities per risk factor (`SOFR.3M`, `SOFR.6M`, … plus model parameters), then checks that the maximum EPE difference and the maximum sensitivity difference are below `1e-8`:

```bash
cargo run -p scripting-examples --bin xva
```

## Mixing scripted and native trades

A netting set is just `Vec<ContingentClaim>`, so you can concatenate claims from different sources:

```rust,ignore
let mut claims = irs_trade.into_contingent_claims()?;
claims.extend(xccy_trade.into_contingent_claims()?);
claims.extend(structured_note.contingent_claims()?);   // ScriptedProduct
netting_sets.insert("counterparty_A".into(), NettingSet::with_csa_terms(claims, csa));
```

Trade ids are preserved in the resulting `NpvCube`s, so `result.cubes` still lets you separate the exposure contribution of the scripted note from the vanilla swaps while CVA/DVA/FVA are computed on the netted total.

## Limitations

- Scripted claims are always `Side::LongReceive` with unit notional; express direction and notional inside the script.
- The market model must be able to serve every request the script makes: `RateIndex("X", …)` needs a curve model for `MarketIndex::X`, `Spot("USD","CLP")` needs an FX model for CLP, `Spot("AAPL")` an equity model.
- Scripting is currently Rust-only; the Python bindings expose the XVA engine for native trades but not `ScriptedProduct`.
