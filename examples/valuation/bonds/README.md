# Bond valuation

Compare tree and backward-Fourier prices and OAS risk for a callable,
amortizing, mixed fixed/floating USD bond. The Rust example uses a flat SOFR
curve and fixed Hull–White parameters dated 2025-01-01. The Python walkthrough
also demonstrates calibration to the shared illustrative caplet surface.

Run from the repository root:

```bash
cargo run -p valuation-bonds
```

The [Python bonds walkthroughs](../../../bindings/python/examples/valuation/bonds/README.md)
are isolated in the matching product folder. Return to the
[valuation catalog](../README.md).
