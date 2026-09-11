# Installation

QuantSupport requires a stable Rust toolchain. Install Rust with [rustup](https://rustup.rs/), then add the crate to your project:

```toml
[dependencies]
quantsupport = "0.1.5"
```

To develop against a local checkout, use a path dependency instead:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

Enable plotting helpers only when needed:

```toml
quantsupport = { version = "0.1.5", features = ["plot"] }
```

Verify the repository checkout with:

```bash
cargo build -p quantsupport
cargo test -p quantsupport
```

Examples are independent workspace packages. For example:

```bash
cargo run -p valuation
```

## Python development install

The Python extension is built with PyO3 and maturin. Create and activate a virtual environment, then run from the repository root:

```bash
python -m pip install maturin
maturin develop -m bindings/python/Cargo.toml --release
python -c "import quantsupport; print(quantsupport)"
```

See [Python API](python-api.md) for the context-manager workflow.
