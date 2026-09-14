# Installation

## Requirements

- Rust stable toolchain (edition 2021). Install with [rustup](https://rustup.rs).

## Rust crate

Add the latest release to your project:

```bash
cargo add quantsupport
```

Or, from a checkout of the repository:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

With plotting helpers:

```bash
cargo add quantsupport --features plot
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

## Documentation

- API docs: <https://docs.rs/quantsupport>
- This book: <https://jmelo11.github.io/quantsupport/> (published from `main` by `.github/workflows/pages.yml`)
- Local build: `cargo install mdbook --locked && mdbook serve --open`
