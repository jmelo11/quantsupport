# Examples

The example packages turn the concepts in this book into complete executable workflows. Each package is a Cargo workspace member under `examples/` and keeps its input files in a local `data/` directory. This chapter helps readers choose an example, recognize their shared setup, and use them as starting points for Rust or Python applications.

Run the commands from the repository root. The table is ordered broadly from market construction through pricing, risk, simulation, and XVA:

| Command                                           | What it shows                                                                                                                                                                                           |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo run -p bootstrap`                          | Loads `quotes.json` and `curve_specs.json`, bootstraps SOFR, TermSOFR3m, ICP, the CLP-under-USD collateral curve, and a price-anchored corporate bond curve, then prints pillar dates, discount factors and zero rates |
| `cargo run -p valuation`                          | Builds swaps with `MakeSwap`, evaluates `Value`, `FairRate` and `Cashflows` through `PricingContext`, prints the cashflow table                                                                         |
| `cargo run -p sensitivity`                        | Prices SOFR, Term SOFR, ICP and USD/CLP cross-currency swaps with `DualFwd` and prints per-quote sensitivity ladders                                                                                    |
| `cargo run -p evaluator`                          | Registers several pricers in an `Evaluator` keyed by `TypeId` and prices a heterogeneous portfolio via `&dyn Any`                                                                                       |
| `cargo run -p volatilitysurface`                  | Builds a SOFR caplet Black surface and prints interpolated vols on an expiry × strike grid                                                                                                              |
| `cargo run -p hullwhite`                          | Builds the caplet market from `vol_specs.json`, calibrates the Hull-White model configured once in `simulation.json`, prints calibration quality and simulates paths                                  |
| `cargo run -p pfe`                                | Builds claims for a swap and an FX forward, simulates with `LgmMarketModel`, prints EE/EPE/PFE profiles                                                                                                 |
| `cargo run -p cva`                                | Runs `XvaEngine` on a netting set (5Y SOFR swap + 5Y USD/CLP XCCY) with `csa_terms.json` and `xva_config.json`, prints CVA/FVA and sensitivities                                                        |
| `cargo run -p scripting-examples --bin valuation` | Parses a scripted payoff, builds a `ScriptEngine` and prints value and expected cashflows                                                                                                               |
| `cargo run -p scripting-examples --bin xva`       | Wraps a `ScriptedProduct` as contingent claims and runs it through the exposure engine next to a vanilla swap                                                                                           |

The examples deliberately overlap. For instance, `bootstrap` focuses on constructed curves, `sensitivity` follows those curves into trade risk, and `cva` extends them into simulated exposure. Reading that sequence shows how the same market identities and quote labels survive across workflows.

## Common structure

Configuration-driven examples all load observable data, attach construction specifications to a `PricingContext`, initialize the market, and pass the context to a pricer. The following condensed version uses the public quote-record type and the wrapper found in the example curve files:

```rust,ignore
#[derive(serde::Deserialize)]
struct CurveSpecFile {
    curve_specs: Vec<CurveConfiguration>,
}

let records: QuoteStoreRecords = serde_json::from_str(
    &fs::read_to_string("examples/<name>/data/quotes.json")?
)?;
let quote_store = QuoteStore::try_from(records)?;
let curve_file: CurveSpecFile = serde_json::from_str(
    &fs::read_to_string("examples/<name>/data/curve_specs.json")?
)?;

let mut ctx = PricingContext::new()
    .with_quote_store(quote_store)
    .with_curve_configurations(curve_file.curve_specs)
    .with_fixing_store(fixings);
ctx.initialize()?;

let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
```

Product-specific examples add volatility, simulation, or XVA configuration before initialization. The key lifecycle stays the same: deserialize typed inputs, initialize dependent market elements once, then evaluate one or more trades against that context.

## Python

`bindings/python` mirrors the pricing and XVA workflows through `PricingContext(...)`, `ctx.evaluate(trade, requests)`, and `ctx.run_xva(config, netting_sets)`. Results are returned as pandas DataFrames. `bindings/python/README.md` documents the supported constructors and conversion rules. Scripted-product construction currently lives in the Rust API.

## Tests and benchmarks

Examples demonstrate workflows, tests enforce behavior, and benchmarks measure performance-sensitive paths. Run them with:

- `cargo test` runs unit tests, integration tests and doctests (`cargo test --doc -p quantsupport`).
- `cargo bench -p benchmarks` runs Criterion benchmarks for bootstrapping and pricing. Reports are written to `target/criterion`.

## What to remember

Choose the smallest example that contains the workflow you need, then follow its typed data from loading through construction to results. The packages are executable documentation, and their assertions provide useful reference values when adapting the code to another currency, index, product, or model.
