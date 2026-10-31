# Net interest income forecasting

[Forecasting net interest income](nii_forecast.ipynb) generates 300 individual
seasoned positions: 120 fixed loans, 80 floating loans and 100 term deposits.
Contracts have different origination dates, maturities, notionals, rates, payment
frequencies and repayment schedules. Funding matches the loans' current
outstanding principal after contractual amortization.

The notebook compares runoff, recursive replacement and replacement plus planned
growth over two years. Scenarios shift interest rates or the funding premium.
Market inputs and historical observations are illustrative and reproducible.

All implementation and explanations are in the notebook:

| Definition | Responsibility |
| --- | --- |
| `ForecastPosition` / `ForecastPortfolio` | Retain individual native trades, segments and an equivalent `qs.Portfolio` |
| `generate_opening_portfolio` | Generate seasoned contracts, their historical fixings and matched funding |
| `ProductionTemplate` / `ProductionQuote` | Solve native par coupons or spreads, add a commercial margin and record the quote audit |
| `RolloverRule` / `NewProduction` | Separate scheduled/prepaid principal replacement from additional business |
| `IncomeForecaster` | Project each cohort, process recursive production events and calculate earned interest |
| `ForecastResult` | Return monthly NII, coupon-level accrual, segment balances, production quotes and receipts |

Follow the [shared setup](../README.md#install-and-run), then run from the
repository root in an environment containing the checkout-built binding:

```bash
python bindings/python/examples/run_notebooks.py nii_forecasting
python -m unittest discover -s tools/tests -p 'test_nii_forecast.py' -v
```

The focused tests execute the notebook's definition cells directly. They check
native par pricing, floating-index conventions, margins, accrual, principal
conservation, reproducible generation, recursive replacement, delayed renewal,
position overrides and independent scenario runs.

## Pricing new positions

Loan production uses `Request.FairRate(0)` with
`rebuild_amortization_for_fair_rate=True`. Fixed loans solve for a coupon;
floating loans solve for a spread over their index. The configured
`fixed_income_policy` selects the loan funding curve. Native construction with
`context` resolves the initial floating observation and builds amortization
from the initial all-in coupon.

The zero-margin contract is rebuilt and repriced before applying the commercial
margin. Loan receipt PV must match the discounted advance; deposit net PV must
be zero. `par_error_bp` records the residual. The final offer adds the margin
once to the solved coupon or spread. Quotes are cached only within one forecast,
for the same template and origination date; scenarios have separate caches.

## Income and balances

Interest is signed expected principal times the resolved annual coupon times the
coupon's Actual/360 or Actual/365 year fraction for its overlap with the reporting
month. Floating coupons use `fixing + spread`. Native loan receipt notionals
already include projected prepayments and position signs. Accrual detail retains
the inputs to each interest amount, including coupons paid beyond the horizon.

Principal never enters NII. Balance movements belong to `(period_start,
period_end]`; interest uses `[period_start, period_end)`. Production on the forecast
date is included in the first period. Production on the horizon date affects
closing balances but earns no in-horizon interest.

For an existing book, retain member trades and segments before native portfolio
aggregation. Supply contracts reflecting actual current terms and past principal
reductions. The generated portfolio assumes no historical prepayments and derives
current balances from remaining contractual principal; future CPR does not infer
past prepayments.

## Scope

The example uses one currency, fixed/floating loans without default, simple-interest
term deposits and Actual/360 or Actual/365 accrual. New contracts use unadjusted
dates and zero fixing lag. Native refinancing CPR compares coupons with its index
benchmark, excluding replacement commercial margins, fees and transaction costs.

Residual principal cash is explicit and earns zero. Negative cumulative cash
requires additional funding whose cost must be represented by treasury trades.
The replacement and growth cases match lending with funding. Credit/default,
effective-interest fee amortization, non-maturity deposits, early deposit
withdrawal, overnight compounding and FX require additional models.
