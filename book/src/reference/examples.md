# Executable reading route

Executable examples complement the book by supplying complete construction
inputs and actual output interpretation. They are not evidence that every
illustrative fragment is a standalone program. Run Rust packages from the
repository root so their existing data paths resolve.

## From one valuation to a constructed market

Begin with `cargo run -p valuation-swaps`. It isolates one flat-curve regular
swap and gives a numerical reference for compounding, sign and cashflow-table
interpretation. `cargo run -p bootstrap` then replaces the hand-built market
with calibrated SOFR, Term SOFR, ICP, collateralized CLP and bond-anchored
curves. `cargo run -p sensitivity` follows those dependencies into quote-level
trade risk.

Product examples add genuinely different problems rather than just different
function names. `valuation-caps` connects optionlet quotes to volatility risk.
`valuation-bonds` compares tree and backward-Fourier callable valuation.
`valuation-loans` separates contractual principal from market observations,
prepayment, credit and fair-rate schedule choices.

## From market targets to future states

`cargo run -p volatilitysurface` inspects the implied-volatility grid.
`cargo run -p hullwhite` calibrates model volatility, reports instrument-level
quality and generates numerical paths. `cargo run -p pfe` develops
claim-based exposure, while `cargo run -p cva` adds netting-set credit/funding
inputs and the high-level XVA engine.

The scripting package has explicit binaries:

```sh
cargo run -p scripting-examples --bin valuation
cargo run -p scripting-examples --bin xva
cargo run -p scripting-examples --bin stepped-coupon-bond
```

These compare selected native and scripted representations under their stated
model/payoff setups. Read their assertions as scoped regressions, not proofs
of exact general stochastic equivalence or of optimal exercise for arbitrary
scripts.

## Python and Excel

The Python notebook collection follows the same scopes under
`bindings/python/examples`: bootstrap; valuation/swaps, bonds, caps and loans;
volatility surfaces; Hull–White; sensitivities; scripting; and XVA. Each can
run in a fresh kernel. The repository's existing runner executes a selected
scope or the whole collection:

```sh
python bindings/python/examples/run_notebooks.py valuation/swaps
python bindings/python/examples/run_notebooks.py
```

The optional
[rendered studies](https://jmelo11.github.io/quantsupport/api/python/studies/index.html)
present the same canonical notebooks and saved outputs without requiring a
local kernel.

The notebook [directory](https://github.com/jmelo11/quantsupport/tree/main/bindings/python/examples)
contains local loaders and illustrative market/model files. Read the
problem-setting narrative and units alongside the code; do not copy helper
defaults into an unrelated market without reviewing them.

Excel supplies swap and historical-pricing workbooks. Load the released XLL
before opening them and reconstruct native handles with full recalculation.
The [workbook chapter](../excel/workbook.md) explains revisioned dependencies;
the [historical chapter](../excel/historical-pricing.md) explains snapshots.
Their formulas, not saved handle strings, are the reproducible workflow.

## Checking examples and prose

`cargo test -p quantsupport` runs the native tests and doctests. The book's
self-contained Rust fences can be checked after building the library:

```sh
cargo +stable build -p quantsupport
RUSTUP_TOOLCHAIN=stable mdbook test -L target/debug/deps
python -m unittest discover -s book/tests -v
```

The build and rustdoc harness must use the same compiler toolchain. Selecting
stable explicitly also retains the dependency-library layout assumed by `-L`.

Runnable book snippets include the hidden crate declaration required by
mdBook's rustdoc harness. Fences marked `rust,ignore` are explicitly described
fragments requiring surrounding inputs, not a substitute for testing complete
examples. Structural tests check chapter coverage, local links and this
snippet distinction.
