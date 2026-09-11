# Netting and CSA

A `NettingSet` groups trades whose positive and negative future values can offset under one legal agreement. `CsaTerms` describes collateral and counterparty-specific credit and funding assumptions.

```rust,ignore
let csa = CsaTerms::new(
    MarketIndex::SOFR,
    Currency::USD,
    credit_spread,
    recovery,
    funding_spread,
);
let netting_set = NettingSet::new("counterparty-a", claims, csa);
```

Use the constructors and field types from the current API; the full call appears in `examples/cva`. Python provides the corresponding `qs.CsaTerms` and `qs.NettingSet` classes.

Netting must happen before applying positive and negative exposure functions. If \(V_i(t)\) are trade values, the net exposure is based on \(\sum_i V_i(t)\), not \(\sum_i\max(V_i(t),0)\).

Collateral currency and index determine remuneration and discounting assumptions. Separate counterparties or legal agreements into separate netting sets even when their trades share the same market simulation. Treat recovery, credit, and funding inputs as scenarioable market or policy assumptions, not product terms.
