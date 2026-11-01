# Net interest income forecasting

[Forecasting net interest income](nii_forecast.ipynb) builds a book of 300 distinct
seasoned contracts, forecasts monthly earned interest, and sizes deposit hedges
from native sensitivity reports. It shows both an opening hedge and monthly
PV01 rebalancing as new business arrives. All implementation and technical
explanations are in the notebook.

The forecast has three steps:

1. Project native portfolio groups and identify their future principal receipts.
2. Replace eligible repayments in date order and add explicit planned production.
3. Allocate those resolved coupons to monthly accrual windows and build reports.

`ForecastPosition` retains each trade and its segment. Portfolio grouping keeps
individual coupon, repayment and prepayment terms while reducing Python calls
and pandas tables. `position_detail=True` selects original-position attribution
for an audit. Rust prices each member once per forecast and reuses the resolved
cashflows for all reporting months.

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
forecast scenarios. Typed records hold orders, cohorts and report rows.

`ForecastTables` creates the income, production, cashflow, balance and monthly
summary tables. `ExampleTables` builds display comparisons. The forecast uses
an ordered list of production orders, with no dictionary-based event state or
quote cache. The mappings in `MarketSetup.context` are required quote-store and
currency-policy inputs to the native binding.

## Earned interest and principal

`EvaluationResults.cashflow_accruals(periods)` returns signed, undiscounted coupon
interest for every reporting window in one native call. It reuses the cashflow
report's resolved floating observations and expected prepayment-adjusted notionals,
clips reporting windows to coupon periods, and computes differences in cumulative
contractual accrual. This avoids rebuilding the entire portfolio projection for
each month. The standalone `Request.AccruedAmount(start, end)` remains available
for individual windows and independent reconciliation. Accrual uses ordinary
numeric values without PV or AD.

Only coupons present in the report contribute. The forecast requests full-life
receipts, including coupons paid beyond the horizon. Report accrual requires a
single coupon currency and performs no FX conversion; this example uses EUR.

Opening inventory uses `Loan.contractual_principal(as_of)` to read the remaining
scheduled principal without pricing each trade or creating per-trade cashflow
tables. This assumes no historical prepayment, as in the generated book.

Principal remains separate from NII:

`closing = opening + production - scheduled_principal - prepayment`.

Balance movements belong to `(period_start, period_end]`; earned interest uses
`[period_start, period_end)`. Production on the forecast date enters the first
period. Production on the horizon changes closing principal and earns no
in-horizon interest. Coupons paid after the horizon still contribute interest
earned before it. Current opening balances follow the remaining contractual
principal; the example assumes no historical prepayments.

## Deposit hedging

`DepositHedgingEngine` receives the opening book's native sensitivity report.
We quote par deposits across ESTR, composite LOANFUNDING and EURIBOR3m at 1, 3, 6,
12, 24, 60 and 120 months. Each reference deposit contributes a PV01 matrix
column. The least-squares fit chooses all signed notionals jointly; positive
amounts create `qs.Side.LongReceive` deposits and negative amounts create
`qs.Side.PayShort` deposits. Rounded trades are repriced to show residual risk.

With `cash_neutral=True`, initial lending and borrowing amounts are equal.
This funding choice is independent of keeping forecast principal constant.
The opening comparison keeps hedge coupons fixed and lets those positions run
off. Their PV01 hedge does not automatically offset floating NII changes.
The notebook explains the sizing equations and shows both the residual risk
and the hedge's income and principal-cash effects.

### Rebalance as new loans originate

`MonthlyHedgingEngine` advances the business book through each monthly date.
It carries expected prepayments into loan notionals, preserves contractual
amortization and coupon terms, and keeps past floating resets in a fixing store.
At each rebalance it includes originated lending and funding plus surviving
hedges, drops matured positions, and sizes additional cash-neutral deposit trades
against their combined current PV01. Existing hedge trades retain their original
coupons and notionals. Rounded new hedges are repriced; their native risk is added
to the existing report, avoiding a second valuation of the large business book.

Date-boundary payments and originations precede the solve. Mid-month originations
join the next monthly solve. `monthly`, `rebalances`, `tickets`, `risk` and
`productions` report the income, dated residual risk, executed trades and new
business. This targets PV01; it does not target sensitivity of forecast NII.

```python
def market_at(when, fixings):
    return MarketSetup.context(as_of=when, fixings=fixings)

monthly_hedge = MonthlyHedgingEngine(hedger).forecast(
    portfolio, AS_OF, HORIZON, market_at=market_at, fixings=context.fixings,
    rollovers=constant_balance_rules, planned=growth,
)
monthly_hedge.rebalances
```

The worked monthly market path holds quoted zero rates unchanged by tenor and
advances the reference date. Missing realized EURIBOR3m resets use the current
month's forwards; supplied observations take precedence. Supply another
`market_at(date, fixings)` factory for observed or simulated dated markets.
Expected prepayments stand in for realized principal in this credit-free example.
The unhedged and hedged NII columns use the same monthly market path, which can
differ from the earlier full-horizon forecasts using fixed-date forward curves.

Monthly risk rebalancing requires fresh valuations on each dated book. Loan
sensitivities and income receipts share a native projection, and income reuses
those resolved coupons for its reporting windows.

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
