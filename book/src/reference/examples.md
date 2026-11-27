# Executable reading route

The example collection supplies complete market inputs and valuations for the
workflows developed in this book. Run the Rust packages from the repository
root so their data paths resolve.

## From one valuation to a constructed market

Begin with `cargo run -p valuation-swaps`. It isolates one flat-curve regular
swap and gives a numerical reference for compounding, sign and cashflow-table
interpretation. `cargo run -p bootstrap` then replaces the hand-built market
with calibrated SOFR, Term SOFR, ICP, collateralized CLP and bond-anchored
curves. `cargo run -p sensitivity` follows those dependencies into quote-level
trade risk.

The product examples develop valuation and quotation choices for each family.
`valuation-caps` connects optionlet quotes to volatility risk. `valuation-bonds`
compares tree and backward-Fourier callable valuation. `valuation-loans`
separates contractual principal from market observations, prepayment, credit and
fair-rate schedule choices.

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

These compare native and scripted representations under specified models and
payoff rules. Their assertions test those setups; stochastic comparisons also
require a sampling tolerance and matching exercise assumptions.

## Checking examples and prose

`cargo test -p quantsupport` runs the native tests and doctests. The book's
self-contained Rust fences can be built and checked with:

```sh
RUSTUP_TOOLCHAIN=stable python tools/book/check_rust.py
python -m unittest discover -s tools/tests -p 'test_book.py' -v
```

The runner builds the library and selects the artifacts reported by Cargo,
then gives mdBook a temporary library search directory. Cached builds with
different features cannot introduce duplicate crate candidates. The build and
rustdoc harness use the same compiler toolchain, selected explicitly above.

Runnable book snippets include the hidden crate declaration required by
mdBook's rustdoc harness. Fences marked `rust,ignore` are explicitly described
fragments requiring surrounding inputs, not a substitute for testing complete
examples. Structural tests check chapter coverage, local links and this
snippet distinction.
