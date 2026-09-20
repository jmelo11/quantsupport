# Installation

This chapter prepares a Rust project for QuantSupport and explains the choices
behind each installation route. A released dependency is appropriate for
applications that want a stable version. A workspace checkout is useful
for running the examples, reading the source, or contributing changes.

## Requirements

QuantSupport targets the stable Rust toolchain and uses the 2021 edition.
Installing Rust through [rustup](https://rustup.rs) provides `cargo`, the
compiler, and a straightforward upgrade path. Verify the installation with
`rustc --version` and `cargo --version` before adding the crate.

## Rust crate

For a normal application, add the latest published release. Cargo records the
selected compatible version in the project manifest and resolves its
dependencies automatically.

```bash
cargo add quantsupport
```

During library development, a path dependency points an application at a
local checkout. This lets the application compile against source changes
without publishing an intermediate release.

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

Plotting is an optional feature for applications and examples that render
charts. Core pricing services can use the default feature set.

```bash
cargo add quantsupport --features plot
```

The crate uses `chrono` for dates, `rayon` for parallel XVA and script evaluation, `rand` and `sobol_burley` for random sequences, `nalgebra` for solvers and correlation matrices, `num-complex` for FFT operations, `serde` for configuration, and `thiserror` for errors. Applications that load the JSON files described in [Configuration](../reference/configuration.md) should add `serde_json` to their own dependencies.

The distinction between library and application dependencies matters when
loading external data. QuantSupport supplies serializable types. The
application chooses the format reader and owns file-system concerns.

## Building the repository

The repository is a Cargo workspace whose members are the library,
`benchmarks`, `bindings/python`, and one package per example. Building the
library alone gives a quick development cycle. Building the workspace checks
that examples and bindings still agree with the public API.

```bash
git clone https://github.com/jmelo11/quantsupport
cd quantsupport
cargo build -p quantsupport              # library only
cargo test -p quantsupport               # unit tests + doctests
cargo build --workspace                  # everything, including examples and bindings
```

The crate is compiled with `missing_docs = "forbid"` and Clippy `pedantic`, `nursery` and `cargo` lint groups at `deny`, with `unwrap_used` and `expect_used` denied. If you contribute, run `cargo clippy --all-targets` before opening a pull request.

After the workspace builds, run an example to confirm that market data can be
loaded and a complete workflow can execute. These three commands exercise
direct pricing, curve construction, and scripting respectively.

```bash
cargo run -p valuation
cargo run -p bootstrap
cargo run -p scripting-examples --bin valuation
```

See [Examples](../reference/examples.md) for the full list.

## Documentation

The API reference answers method-level questions. This book explains
how the types collaborate and why the workflows are designed as they are.
During development, a local mdBook server rebuilds the prose as files change.

- API docs: <https://docs.rs/quantsupport>
- This book: <https://jmelo11.github.io/quantsupport/> (published from `main` by `.github/workflows/pages.yml`)
- Local build: `cargo install mdbook --locked && mdbook serve --open`

## Installation outcome

A successful workspace build establishes that the Rust toolchain, crate
features, examples, and bindings are compatible. The next chapter uses that
environment to construct and price a swap, turning the installation into a
working valuation path.
