# Mortgage valuation

[Understanding mortgage cashflows and value](loans.ipynb) follows a EUR 250,000 mortgage
with 360 monthly payments over 30 years, from its first interest accrual to its
market value. It explains how the library
handles day count, French/bullet/balloon repayment, floating resets, introductory
fixed coupons over five years, coupon bounds, par lending rates, funding through EURIBOR plus a
spread curve, market scenarios, prepayment, and borrower recovery.
Tables and charts show how those assumptions affect payments, expected principal
life, income, and exposure to market quotes.

The local [market_data.py](market_data.py) reads the illustrative 24 February 2026
EUR discount, EURIBOR12m, and borrower CDS inputs shared with the Rust example.
Deposits anchor the short end; ESTR OIS and EURIBOR swaps extend the rate curves
through 35 years. Funding spreads and credit inputs cover the same horizon.
The notebook adds an explicit treasury funding spread curve and reprices fixed
and floating loans under benchmark and funding spread shocks.
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
