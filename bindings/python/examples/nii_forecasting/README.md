# Net interest income forecasting

[Forecasting net interest income](nii_forecast.ipynb) builds a book of 300 distinct
seasoned contracts and forecasts monthly earned interest with prepayment,
replacement lending, planned growth and funding scenarios. All implementation
and technical explanations are in the notebook.

The forecast has three steps:

1. Project native portfolio groups and identify their future principal receipts.
2. Replace eligible repayments in date order and add explicit planned production.
3. Allocate projected coupon interest to months with pandas and build the reports.

`ForecastPosition` retains each trade and its segment. Portfolio grouping keeps
individual coupon, repayment and prepayment terms while reducing Python calls
and pandas tables. `position_detail=True` selects original-position attribution
for an audit. The first accrual window is requested alongside cashflows;
subsequent simple-interest windows use those projected coupons without repricing.

## Keep principal balances constant

For the notebook's opening book, use a full same-day replacement rule for every
segment. The replacement template must retain that segment's currency and
receive/pay side. With the production templates defined in the notebook:

```python
rules = (
    RolloverRule.constant_balance(Segment.FIXED_LOANS, fixed_production),
    RolloverRule.constant_balance(Segment.FLOATING_LOANS, floating_production),
    RolloverRule.constant_balance(Segment.TERM_FUNDING, funding_production),
)
forecaster = IncomeForecaster(date(2026, 3, 1), date(2028, 3, 1))
with context:
    result = forecaster.forecast(context, portfolio, rollovers=rules)

ForecastTables.constant_balance_table(result)
```

`constant_balance` replaces 100% of scheduled principal and 100% of prepayments,
with zero lag. Replacement contracts follow the same rules when they repay.
The resulting table compares opening principal with the minimum and maximum
monthly closing principal for every segment. Those amounts agree in this case.

We leave `planned` empty for this forecast. A segment with no rule runs off;
replacement fractions below one or a positive lag allow its principal to fall.
Planned loan/funding orders add growth. Coupon income affects NII separately
from the principal balance kept constant here.

## Typed inputs and separate reporting

All forecast and ledger dates are `datetime.date`. Calendar-month shifts use
`dateutil.relativedelta` directly. We normalize dates returned by the binding
when reading observations and cashflows.

Contracts use native `qs.Side`, `qs.Currency`, `qs.MarketIndex`, `qs.DayCounter`,
`qs.Frequency`, `qs.Compounding` and repayment structures. Local enums describe
segments, product and rate types, production origins, principal movements and
forecast scenarios. Orders and cohorts retain business state; inventory, income,
principal movements and production reports use pandas DataFrames directly.

`ForecastTables` creates the income, production, cashflow, balance and monthly
summary tables. `ExampleTables` builds display comparisons. The forecast uses
an ordered list of production orders. Principal movements are bucketed once by
reporting period, then grouped cumulative sums produce balances. No custom row
classes or repeated monthly scans of the movement ledger are needed. The mappings
in `MarketSetup.context` are required quote-store and
currency-policy inputs to the native binding.

## Earned interest and principal

`Request.AccruedAmount(start, end)` returns signed, undiscounted coupon interest
in `result.accrued_amount`. Evaluation results hold completed data; the selected
pricer performs the calculation. Coupon periods clip the reporting window and
principal is excluded. Accrual is the difference in cumulative contractual
accrual at the clipped boundaries.

The forecast requests its first window together with `Cashflows`. The loan
pricer reuses the existing projection's resolved fixings and expected
prepayment-adjusted notionals. Later simple-interest windows allocate the same
projected coupon amounts with NumPy arrays. Actual/360 and Actual/365 use
vectorized calendar-day differences; other conventions use native cumulative
day-count fractions. Non-simple compounding, overnight conventions and
foreign-currency coupons retain native accrual requests.
Amounts use the context's base currency; this example uses EUR throughout.
Coupons paid beyond the reporting horizon still contribute interest earned
within it. Regression tests reconcile the pandas allocation to independent native
monthly accrual requests, including 30/360 boundaries and rate scenarios.

Opening inventory supplies historical coupon resets and refinancing benchmark
fixings, then values each loan once. Its remaining native redemptions give the
expected current principal after amortization, resets and prepayment from
origination. Funding deposits match that expected opening principal.

Principal remains separate from NII:

`closing = opening + production - scheduled_principal - prepayment`.

Balance movements belong to `(period_start, period_end]`; earned interest uses
`[period_start, period_end)`. Production on the forecast date enters the first
period. Production on the horizon changes closing principal and earns no
in-horizon interest. Coupons paid after the horizon still contribute interest
earned before it. Current opening balances include expected historical
prepayments.

## Run and scope

Follow the [shared setup](../README.md#install-and-run). From the repository root:

```sh
python bindings/python/examples/run_notebooks.py nii_forecasting/nii_forecast.ipynb
```

The example uses one currency, fixed/floating loans without default, and
simple-interest term deposits. Reporting currency matches the context base
currency. New business uses native par rates plus explicit commercial margins.
Scenarios reprice future resets, prepayments and production in their own market.

Residual principal cash is reported and earns zero. Funding costs require
explicit funding positions. Credit/default, fees, non-maturity deposits,
early withdrawals, partial overnight accrual and FX require additional models.
