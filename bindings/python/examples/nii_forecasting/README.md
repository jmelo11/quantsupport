# Net interest income forecasting

[Forecasting net interest income](nii_forecast.ipynb) projects a EUR 1 million
loan portfolio and its term-deposit funding over two years. It compares runoff,
recursive principal replacement, and replacement plus planned growth under
base, +100 bp and -100 bp rate scenarios. Editable illustrative curves and
contracts are defined inline; no external market data is required.

The notebook defines and explains the forecast classes alongside their use:

| Class | Responsibility |
| --- | --- |
| `ForecastPosition` / `ForecastPortfolio` | Retain native trades, product groups and an equivalent `qs.Portfolio` |
| `ProductionTemplate` | Build new fixed/floating loans or fixed term deposits with curve-plus-margin pricing |
| `RolloverRule` | Replace separate fractions of scheduled principal and prepayments, optionally after a lag |
| `NewProduction` | Schedule additional business independently of replacement |
| `IncomeForecaster` | Project cohorts once, process dated production recursively and allocate coupon accrual into months |
| `ForecastResult` | Expose monthly NII, detailed interest, segment balances, production and full-life receipt tables |

The forecast uses native loan expected receipts, including their prepayment
reductions. Deposit interest uses native `FixedRateCoupon.amount()` so seasoned
deposits do not require discounting their historical initial advance. It keeps
principal out of income, includes accrual on coupons paid beyond the horizon,
respects position signs and size overrides, and pools same-day production.
Replacement cohorts and planned business generate their own subsequent runoff.
No original instrument is mutated.

Follow the [shared setup](../README.md#install-and-run), then run from the
repository root with the environment containing the checkout-built binding:

```bash
python bindings/python/examples/run_notebooks.py nii_forecasting
python -m unittest discover -s tools/tests -p 'test_nii_forecast.py' -v
```

The focused tests execute the notebook's definition cells directly, then check
independent simple-interest calculations, CPR principal conservation,
recursive replacement, different replacement fractions, delayed renewal,
planned lending/funding, position overrides, native portfolio reconciliation,
partial periods and scenario isolation.

## Using an existing book

Retain your member trades when constructing `ForecastPortfolio`; its `.native`
property returns the equivalent native `qs.Portfolio`. The native portfolio
does not expose membership to Python, and its aggregated cashflows cannot
recover the trade-level groups needed for replacement rules. Assign one
asset/liability side per segment. Supply actual current contracts and remaining
repayment terms: the library does not infer historical prepayments from CPR.

Call `IncomeForecaster.forecast` inside a pricing context on the forecast date.
Pass no rollover rules for runoff, or a rule per segment for replacement.
Planned production is additional to replacement. A rule applies to all cohorts
in its segment, including planned business. Run the complete forecast again
inside each `qs.Scenario` scope to capture rate, prepayment and production-price
changes together.

Accrual periods are `[period_start, period_end)`. Principal movements belong to
`(period_start, period_end]`, with production on the forecast date included in
the first period. Production on the horizon date affects closing balances but
earns no in-horizon interest. Full-life receipt tables retain payments beyond
the reporting horizon; production advances are tracked in the balance ledger.

## Scope and assumptions

This is a deterministic, single-currency coupon-income example for
simple-interest loans without default and fixed term deposits. New contracts
use unadjusted dates and zero fixing lag. Fixed production locks the simple
term forward on the coupon's day count plus a margin; this policy is not a
par-rate solver for amortizing loans. Floating production resets at its index
plus the contractual spread. Native refinancing CPR uses an index benchmark
that excludes replacement lending margins, fees and transaction costs.

Residual principal cash or funding earns zero, and is shown explicitly in
`net_principal_cashflow` and `cumulative_principal_cash`. A negative cash balance
needs additional funding whose cost is excluded here. The notebook pairs new
lending with funding and replaces both sides under its replacement/growth
cases. Add treasury positions for a fully funded business forecast.

Credit/default, effective-interest fee/premium amortization, non-maturity
deposit beta and decay, early deposit withdrawal, overnight compounding and
FX require additional models. NII is undiscounted accrued interest; changing
the discount curve alone does not create funding expense.
