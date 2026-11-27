# Installation

This repository contains the QuantSupport library, examples, benchmarks, and
this Book.

## Adding QuantSupport to an application

QuantSupport uses the Rust 2021 edition. Install a stable toolchain with
[rustup](https://rustup.rs), then add the crate to your application:

```sh
cargo add quantsupport
```

During development you can use a local checkout as a path dependency instead:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

Optional dependencies:

- The `plot` feature adds plotting helpers. Pricing does not need it.
- To load JSON files, add `serde_json` to your own application. The library
  types can be serialized, but your application reads and writes the files.

Build the library with the stable toolchain. Update the toolchain first so you
use the same compiler version as CI:

```sh
rustup update stable
cargo +stable build -p quantsupport
```

Automatic differentiation is part of the library and needs no extra setup. See
the [differentiation guide](../risk/aad.md) for how sensitivities are computed.

To build from source:

```sh
git clone https://github.com/jmelo11/quantsupport
cd quantsupport
cargo build -p quantsupport
cargo run -p valuation-swaps
```

The workspace contains the library, the examples, and the benchmarks. Run
`cargo test -p quantsupport` to test the library and its doctests. Contributors
should also run Clippy and build the documentation.

Continue with the [first valuation](first-swap.md) for a complete example.
