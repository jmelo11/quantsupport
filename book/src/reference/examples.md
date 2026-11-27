# Example programs

The examples include complete market data and valuations for the workflows in
this Book. Run them from the repository root so their data paths resolve.

## From one valuation to a constructed market

1. `cargo run -p valuation-swaps` prices one regular swap on a flat curve. It
   is a reference for compounding, signs, and reading the cashflow table.
2. `cargo run -p bootstrap` replaces the flat curve with calibrated SOFR, Term
   SOFR, ICP, collateralized CLP, and bond curves.
3. `cargo run -p sensitivity` shows trade risk to each quote through those
   curves.

The product examples cover valuation and quoting for each product family:

- `valuation-caps` links optionlet quotes to volatility risk.
- `valuation-bonds` compares tree and backward-Fourier valuation of a callable
  bond.
- `valuation-loans` shows contractual principal, fixings, prepayment, credit,
  and fair-rate schedule options.

## From market targets to future states

- `cargo run -p volatilitysurface` builds and queries an implied-volatility
  grid.
- `cargo run -p hullwhite` calibrates model volatility, reports fit quality for
  each instrument, and generates paths.
- `cargo run -p pfe` computes claim-based exposure.
- `cargo run -p cva` adds netting sets, credit and funding inputs, and the
  high-level XVA engine.

The scripting package has separate binaries:

```sh
cargo run -p scripting-examples --bin valuation
cargo run -p scripting-examples --bin xva
cargo run -p scripting-examples --bin stepped-coupon-bond
```

They compare native and scripted versions of the same products under the given
models and payoff rules. Their assertions check those setups. Stochastic
comparisons also need a sampling tolerance and the same exercise assumptions.

## Checking examples and prose

`cargo test -p quantsupport` runs the library tests and doctests. Build and
check the Book's code blocks with:

```sh
RUSTUP_TOOLCHAIN=stable python tools/book/check_rust.py
python -m unittest discover -s tools/tests -p 'test_book.py' -v
```

The checker builds the library and passes the artifacts reported by Cargo to
mdBook in a temporary library directory. Cached builds with different features
therefore cannot cause duplicate crate errors. The build and the doctests use
the same toolchain, set explicitly above.

Runnable code blocks in the Book include the hidden crate declaration that
mdBook's test harness needs. Blocks marked `rust,ignore` are fragments that
need surrounding inputs, and the text before them says so. They do not replace
complete tested examples. Structural tests check chapter coverage, local links,
and this distinction.
