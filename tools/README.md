# Repository tooling

Run maintenance commands from the repository root. Rust benchmark runners live
in `tools/benchmarks/`, Book checks in `tools/book/`, and core release automation
in `tools/release/`. Python and Excel tooling belongs to their own repositories.

```sh
python -m unittest discover -s tools/tests -v
RUSTUP_TOOLCHAIN=stable python tools/book/check_rust.py
```

## Benchmarks

Run the benchmark code from the repository root:

```sh
cargo +stable bench -p benchmarks --bench ad_core
cargo +stable bench -p benchmarks --bench swap_pfe
cargo +stable bench -p benchmarks --bench xva_pricing
```

The runners in `tools/benchmarks/` compare branches and numerical results.
Keep generated JSON and Criterion reports under `target/`. Separate cold
compilation, market setup and warm execution when measuring performance, and
check numerical agreement before interpreting timing differences.
