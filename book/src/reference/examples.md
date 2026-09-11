# Examples

Each example is a workspace member under `examples/` with its own `data/` folder. Run from the repository root.

| Command                                           | What it shows                                                                                                                                                                                           |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo run -p bootstrap`                          | Loads `quotes.json` and `curve_specs.json`, bootstraps SOFR, TermSOFR3m, ICP and the CLP-under-USD collateral curve with `MultiCurveBootstrapper`, prints pillar dates, discount factors and zero rates |
| `cargo run -p valuation`                          | Builds swaps with `MakeSwap`, evaluates `Value`, `FairRate` and `Cashflows` through `PricingContext`, prints the cashflow table                                                                         |
| `cargo run -p sensitivity`                        | Prices SOFR, Term SOFR, ICP and USD/CLP cross-currency swaps with `DualFwd` and prints per-quote sensitivity ladders                                                                                    |
| `cargo run -p evaluator`                          | Registers several pricers in an `Evaluator` keyed by `TypeId` and prices a heterogeneous portfolio via `&dyn Any`                                                                                       |
| `cargo run -p volatilitysurface`                  | Builds a SOFR caplet Black surface and prints interpolated vols on an expiry × strike grid                                                                                                              |
| `cargo run -p hullwhite`                          | Calibrates Hull-White to caplets (`hw_calibration.json`), prints the calibration quality table and ATM cap prices, simulates paths from `simulation.json`                                               |
| `cargo run -p pfe`                                | Builds claims for a swap and an FX forward, simulates with `LgmMarketModel`, prints EE/EPE/PFE profiles                                                                                                 |
| `cargo run -p cva`                                | Runs `XvaEngine` on a netting set (5Y SOFR swap + 5Y USD/CLP XCCY) with `csa_terms.json` and `xva_config.json`, prints CVA/FVA and sensitivities                                                        |
| `cargo run -p scripting-examples --bin valuation` | Parses a scripted payoff, builds a `ScriptEngine` and prints value and expected cashflows                                                                                                               |
| `cargo run -p scripting-examples --bin xva`       | Wraps a `ScriptedProduct` as contingent claims and runs it through the exposure engine next to a vanilla swap                                                                                           |

## Common structure

```rust,ignore
let quotes: Vec<Quote> = serde_json::from_str(&fs::read_to_string("examples/<name>/data/quotes.json")?)?;
let curve_specs: Vec<CurveConfiguration> = serde_json::from_str(&fs::read_to_string(".../curve_specs.json")?)?;

let mut ctx = PricingContext::new()
    .with_reference_date(reference_date)
    .with_quote_store(QuoteStore::from_quotes(quotes))
    .with_curve_configurations(curve_specs)
    .with_fixing_store(fixings);
ctx.initialize()?;

let results = ctx.evaluate(&trade, &[Request::Value, Request::Sensitivities])?;
```

## Python

`bindings/python` mirrors the pricing and XVA examples (`PricingContext(...)`, `ctx.evaluate(trade, requests)`, `ctx.run_xva(config, netting_sets)`), returning pandas DataFrames; see [Python API](../getting-started/python-api.md). Scripting is Rust-only.

## Tests and benchmarks

- `cargo test` runs unit tests, integration tests and doctests (`cargo test --doc -p quantsupport`).
- `cargo bench -p benchmarks` runs Criterion benchmarks for bootstrapping and pricing; reports land in `target/criterion`.
