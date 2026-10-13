# Loan valuation

| Notebook | Workflow |
| --- | --- |
| [loans.ipynb](loans.ipynb) | Typed terms, contractual schedules, cashflow requests, resets, prepayment functions, both fair-rate modes, credit and quote risk |

The local [market_data.py](market_data.py) reads the same illustrative
2026-02-24 EUR discount, EURIBOR12m and borrower CDS inputs as the Rust loan
package. Those files live in
[examples/valuation/loans/data](../../../../../examples/valuation/loans/data).
The pricer resolves observations and reports expected receipts through `Request.Cashflows`; contractual principal remains fixed.

Follow the [shared setup](../../README.md#install-and-run), then run this product
folder from the repository root:

```bash
python bindings/python/examples/run_notebooks.py valuation/loans
```

The [Rust loans package](../../../../../examples/valuation/loans) runs with
`cargo run -p valuation-loans`. Return to the [valuation catalog](../README.md).
