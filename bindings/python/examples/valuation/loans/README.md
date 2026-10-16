# Loan valuation

[Understanding loan cashflows and value](loans.ipynb) follows a EUR 100,000 loan
from its first interest accrual to its market value. It explains how the library
handles day count, French/bullet/balloon repayment, floating resets, introductory
fixed coupons, coupon bounds, par lending rates, prepayment, and borrower recovery.
Tables and charts show how those assumptions affect payments, expected principal
life, income, and exposure to market quotes.

The local [market_data.py](market_data.py) reads the illustrative 24 February 2026
EUR discount, EURIBOR12m, and borrower CDS inputs shared with the Rust example.
The input files live in
[examples/valuation/loans/data](../../../../../examples/valuation/loans/data).

Follow the [shared setup](../../README.md#install-and-run), then execute the
walkthrough from the repository root:

```bash
python bindings/python/examples/run_notebooks.py valuation/loans
```

The [Rust loans example](../../../../../examples/valuation/loans) runs with
`cargo run -p valuation-loans`. See the [valuation catalog](../README.md) for
other products.
