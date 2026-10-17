# Loan valuation

Fixed, floating and mixed EUR loan valuation, with fixed contractual principal,
annual reset requests, historical fixings, prepayment functions, both fair-rate
schedule modes, credit,
refinancing and AAD-versus-scenario checks. Local `data/loan_*.json` files hold
the illustrative 2026-02-24 market shared with the Python loan walkthrough.

Run from the repository root:

```bash
cargo run -p valuation-loans
```

The [Python loans walkthroughs](../../../bindings/python/examples/valuation/loans/README.md)
are isolated in the matching product folder. Return to the
[valuation catalog](../README.md).
