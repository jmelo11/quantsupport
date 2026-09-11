# Installation

## Requirements

- Rust stable toolchain (edition 2021). Install with [rustup](https://rustup.rs).
- For the Python bindings: Python 3.9+ and [maturin](https://www.maturin.rs).
- Optional: the `plot` Cargo feature pulls in `plotters` with the bitmap and SVG backends for the plotting helpers used by `examples/hullwhite`.

## Rust crate

Add the dependency to `Cargo.toml`:

```toml
[dependencies]
quantsupport = "0.1"
```

Or, from a checkout of the repository:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

With plotting helpers:

```toml
quantsupport = { version = "0.1", features = ["plot"] }
```

Runtime dependencies pulled in by the crate: `chrono` (dates), `rayon` (parallel XVA and script evaluation), `rand` and `sobol_burley` (random numbers and Owen-scrambled Sobol sequences), `nalgebra` (linear algebra for the Newton solver and correlation matrices), `num-complex` (FFT), `serde` (configuration), `thiserror` (errors). `serde_json` is a dev-dependency only; add it to your own project to load the JSON files described in [Configuration](../reference/configuration.md).

## Building the repository

The repository is a Cargo workspace whose members are the library, `benchmarks`, `bindings/python`, and one package per example:

```bash
git clone https://github.com/jmelo11/quantsupport
cd quantsupport
cargo build -p quantsupport              # library only
cargo test -p quantsupport               # unit tests + doctests
cargo build --workspace                  # everything, including examples and bindings
```

The crate is compiled with `missing_docs = "forbid"` and Clippy `pedantic`, `nursery` and `cargo` lint groups at `deny`, with `unwrap_used` and `expect_used` denied. If you contribute, run `cargo clippy --all-targets` before opening a pull request.

Run an example:

```bash
cargo run -p valuation
cargo run -p bootstrap
cargo run -p scripting-examples --bin valuation
```

See [Examples](../reference/examples.md) for the full list.

## Python bindings

```bash
python -m venv .venv && source .venv/bin/activate
python -m pip install maturin pandas
maturin develop -m bindings/python/Cargo.toml --release
python -c "import quantsupport as qs; print(qs.Date(2025, 1, 1) + '6M')"
```

`maturin develop` compiles the PyO3 extension in release mode and installs it into the active environment. Result tables are returned as pandas `DataFrame`s, so pandas must be installed.

## Documentation

- API docs: <https://docs.rs/quantsupport>
- This book: <https://jmelo11.github.io/quantsupport/> (published from `main` by `.github/workflows/pages.yml`)
- Local build: `cargo install mdbook --locked && mdbook serve --open`
