# Loans, prepayment and credit

A loan pays interest and principal according to a repayment schedule in the
contract. Its value also depends on expected prepayment and default.
QuantSupport keeps the contractual schedule fixed. The pricer projects the
expected receipts under your market assumptions.

`Loan<T>` stores the contract as a `Leg<T>` of coupons, redemptions, and the
initial advance. Coupon notionals and principal redemptions are fixed `f64`
amounts. You can supply a leg directly, or use `MakeLoan<T>` to generate it from
repayment and schedule terms.

Builder defaults:

- monthly payments;
- French amortization;
- Actual/365 accrual;
- a null calendar and unadjusted payment dates;
- zero fixing lag;
- no prepayment and no credit risk;
- 40% recovery.

Set the calendar, fixing lag, and payment convention to match the real
contract.

Coupons are given with `with_coupons(Vec<CashflowType<T>>)`, as a positional
list like the one used by `SteppedCouponBond`.

- The last coupon repeats until maturity.
- The list can step fixed coupons, spreads, or floating indices.
- Empty lists, entries that are not coupons, and lists longer than the schedule
  are errors.

For a coupon with an embedded payoff, the `PayoffOps` expression is applied to
the observed index, and then the spread is added. Serialization keeps the full
expression.

## Contractual principal

The builder computes principal before any prepayment. Let \\(B\\) be the
initial principal, \\(r\\) the annual amortization rate, \\(\tau_k\\) the
day-count fraction of period \\(k\\), \\(B_f\\) the final balloon, and
\\(B_{k-1}\\) the scheduled balance at the start of period \\(k\\). Then

\\[
i_k=r\tau_k,\qquad
D_k=\prod_{j=1}^{k}(1+i_j)^{-1},\qquad
A=\frac{B-B_fD_n}{\sum_{k=1}^{n}D_k},
\\]
\\[
I_k=B_{k-1}i_k,\qquad
R_k=A-I_k.
\\]

\\(A\\) is the constant scheduled instalment, \\(I_k\\) is interest, and
\\(R_k\\) is scheduled principal.

These factors use the contract's interest accrual, not market discount curves.
Uneven periods and final stubs use their own fractions.

Example: if every period's fraction is exactly \\(1/12\\), a principal of
100,000 at 6% over 360 payments gives an instalment of 599.550525. Real
fractions depend on the payment dates and the day counter. With zero interest
and no balloon, the instalment is the principal divided by the number of
periods.

### Initial amortization rate

- `MakeInstrument::build_using` takes the initial all-in rate from the first
  coupon's fixing or forecast, including the spread and payoff rules.
- `with_initial_amortization_rate` sets the rate explicitly and takes priority.
- `build()` needs an explicit initial `amortization_rate` for French and
  balloon loans whose first coupon is floating.

The builder stores this baseline schedule and recalculates it at each known
fixed-coupon step.

During valuation, French and balloon loans recalculate the instalment each time
the coupon or reset changes, from the balance at that point and the remaining
maturity. The instalment stays the same within each rate period, even when
periods have different accrual lengths. Bullet loans repay all principal at
maturity.

### Day counts

Day counters belong to the coupons, not to the loan:

- A floating coupon uses its own day counter, or its index convention if none
  is set.
- A fixed coupon uses the day counter of its `InterestRate`.

The loan uses the same conventions to amortize principal and keeps them on each
generated cashflow. Set the day counter on the coupon, not on the loan builder.

If the initial amortization rate equals the first all-in floating fixing,
French payments stay level until the first reset, even in months with
different lengths. Each later reset sets a new level instalment for its rate
period.

The following fragment builds a floating-rate mortgage. The market and
trade come from the surrounding application.

```rust,ignore
use quantsupport::prelude::*;
use quantsupport::time::calendars::target::TARGET;

let loan = MakeLoan::<Recorded>::default()
    .with_identifier("EUR-MORTGAGE".into())
    .with_start_date(Date::new(2026, 2, 24))
    .with_maturity_date(Date::new(2056, 2, 24))
    .with_notional(250_000.0)
    .with_currency(Currency::EUR)
    .with_coupons(vec![CashflowType::floating(MarketIndex::EURIBOR12m, 0.01, None, None)])
    .with_initial_amortization_rate(0.05)
    .with_payment_frequency(Frequency::Monthly)
    .with_calendar(Calendar::TARGET(TARGET::new()))
    .with_fixing_lag_business_days(2)
    .build()?;

let contractual_cashflows = loan.contractual_leg().cashflows();
let mut pricer = LoanPricer::new();
pricer.set_discount_policy(Box::new(
    FixedIncomeDiscountPolicy::new(false)
        .with_risk_free_index(Currency::EUR, MarketIndex::ESTR),
));
```

## Coupon observation requests

Payment frequency, reset frequency, and index tenor are separate settings.

- `RateIndexDetails::frequency()` gives how often the index is observed. This
  is separate from the compounding frequency of its quotes.
- The builder reads this from the index details, not from the index name. For
  example, EURIBOR12m gives annual resets and a 12-month observation period,
  even for a loan with monthly payments.
- Frequencies and tenors you set on the builder take priority.
- Daily indices default to a single observation at each payment date. Set the
  loan's real reset conventions if they differ.

Each floating coupon has a `ForwardRateRequest` for its reset. Monthly coupons
can share one annual request. The request holds the fixing date and the start
and end dates of the index period, which can differ from the coupon's accrual
period.

Coupon terms change only at accrual boundaries. Each continuous run of one
floating index starts a new reset grid. Changing only the spread or the
floor/cap keeps the existing grid.

The pricer resolves fixings as follows:

- Past resets use exact stored fixings.
- Today's reset uses the stored fixing if present, and a forecast otherwise.
- Future resets use the index curve and its quote convention.

Fixings are applied to a copy of the leg. The instrument itself is not changed.

## Prepayment functions

A `Loan<T>` holds a `PrepaymentModel<T>` separately from its contractual
cashflows.

- `PrepaymentModelConfiguration::Cpr`, `CprCurve`, and `Refinancing` provide
  built-in models for any supported scalar.
- A custom model passed to `MakeLoan<T>::with_custom_prepayment_model` must use
  the same `T` as the coupon templates.

For a `MakeLoan<Recorded>` that already has the contract terms, this
fragment adds a custom function:

```rust,ignore
let loan = builder.with_custom_prepayment_model(
    |state: &PrepaymentModelState<Recorded>| -> Result<Recorded> {
        // Illustrative annual CPR converted to this interval's probability.
        Ok(Recorded::scalar(1.0 - 0.88_f64.powf(state.interval_year_fraction())))
    },
).build()?;
```

### Changing the scalar type

To change a loan's scalar type, pass a prepayment model for the new scalar:
`loan.into_scalar::<Recorded>(Arc::new(model))`. `LoanTrade<T>` has the same
method, which keeps the trade date and side.

- For built-in models, pass the same configuration again.
- For a custom model, write an equivalent version for the new scalar.

There is no `From`/`Into` conversion between loan scalar types.

### How prepayment is applied

- The pricer applies prepayment after scheduled principal.
- For a regular monthly period, annual CPR becomes `1 - (1 - CPR)^(1/12)`.
  Partial or irregular periods use actual days divided by 365.
- Prepayment is received as principal.
- The remaining share of the balance reduces expected future interest and
  scheduled principal in proportion.
- The contractual leg is not changed.
- For a loan that started in the past, projection starts from the contractual
  balance at the evaluation date. Actual past prepayments are not inferred.

### Refinancing models

Refinancing models compare the loan's rate with a benchmark index that you
configure. The benchmark's observation tenor comes from the index details. It
does not include lending spreads, fees, or transaction costs of a new loan.

The model state provides:

- `refinancing_benchmark_rate()`;
- `contractual_coupon_rate()`;
- `outstanding_principal()`.

Models return the probability of prepaying in the period through
`conditional_prepayment_probability()`. Built-in models can depend on market
rates through AD while the cashflow notionals stay `f64`.

## Valuation and cashflows

`LoanPricer` supports the standard requests. Cashflow reports use
`HandleCashflows` over three projected legs:

- leg 0: scheduled coupons and redemptions;
- leg 1: prepayment redemptions;
- leg 2: recovery redemptions.

Prepayment is projected from the loan's start, including periods before the
evaluation date. This needs the past coupon and refinancing fixings; a missing
fixing is an error.

Report window:

- `Request::Cashflows(true)` reports payments strictly after the evaluation
  date.
- `Request::Cashflows(false)` reports the whole schedule, including the initial
  advance.

The window does not change the PV. PV includes future receipts and excludes the
initial advance.

Floating rows show their `fixing_date`. Monthly coupons that share an annual
reset show the same date. Fixed and principal rows have no fixing date.

### Schedules and stubs

Payment schedules support forward or backward generation, month-end alignment,
`first_coupon_date`, and `next_to_last_coupon_date`.

- The first-coupon date ends an initial stub. The next-to-last date starts a
  final stub.
- `Backward` generation puts an automatic stub at the start. `Forward` puts it
  at the end.
- By default, French amortization keeps the same instalment for a stub. Interest
  uses the stub's actual accrual fraction, and the rest goes to principal.

### Floors and caps

Floating templates with a floor or cap become `OptionEmbeddedCoupon`. The
floor/cap is applied to the resolved index rate on the loan's accrual basis.
This is a cashflow projection only; it adds no option time value.

### Credit

With a borrower `credit_index`, receipts are weighted by the probability of
survival from the evaluation date. On default between payments, recovery on
the balance at the start of the period is paid on the period's payment date.
This is an approximation of recovery timing. Credit and prepayment affect only
the valuation, not the contract.

### Pricer settings

Set the refinancing benchmark and the fair-rate schedule option on the pricer.
By default there is no refinancing benchmark, and fair-rate trials keep the
existing amortization.

Configure a `FixedIncomeDiscountPolicy` explicitly. The loan has a payment
currency, and the policy chooses the discount curve. You can value the same
contract on ESTR, EURIBOR, or a funding curve by changing the policy.

### Sensitivities

Sensitivities are derivatives with respect to labelled market pillars.
Multiply a rate-quote derivative by 0.0001 to get the first-order PV change for
a one-basis-point rise. Refinancing and survival weights are included in AD.
Principal schedules that depend on rates are included in market
sensitivities.

## Fair rate and schedule choice

`FairRate` solves one fixed coupon or floating spread per segment of the
contract. A new segment starts when the fixed rate, spread, coupon type, or
floating index changes. Index resets do not start a new segment. The
evaluation date must be on or before the loan's start date.

The solver works segment by segment:

1. Shorten the loan to the end of the first segment and repay all remaining
   principal there.
2. Solve that segment's rate so the PV of receipts equals the PV of the
   original advance.
3. Fix that rate, extend the loan to the end of the next segment, and repeat.

Each solved part of the loan therefore lends at par, with the remaining
principal repaid at its end. Coupons with a floor or cap are not supported.

`Request::FairRate(index)` selects one zero-based segment and returns its rate
in `fair_rate`. Only the earlier segments needed for that target are solved.
Use separate evaluations for several targets. An invalid index is an error.

### Amortization during the solve

The pricer setting `rebuild_amortization_for_fair_rate` controls how trial
rates affect principal:

- `false` (default): keep the existing amortization schedule.
- `true`: on each trial, regenerate the French, bullet, or balloon schedule
  using the first trial coupon's all-in rate. Once the first segment is solved,
  its rate stays fixed for later trials. A custom leg needs explicit repayment
  details to use this mode.

Neither mode changes the original instrument. Both apply the French reset rule
when projecting receipts.

To rebuild, use `LoanPricer::new().with_fair_rate_amortization_rebuild(true)`.

The solver searches between -5% and 50%. It lowers the upper bound if a trial
rate would give an invalid amortization schedule. If it cannot bracket a
solution, it returns an error.

### Repricing with the solved rate

- In keep-amortization mode, keep the original initial amortization rate and
  change only the coupon or spread.
- In rebuild mode, use the solved all-in rate for amortization too.
- In both modes, keep the original coupon's day count, compounding, and
  frequency.

For example, the mortgage example uses an Actual/360 fixed coupon, while
`CashflowType::fixed(rate)` creates an Actual/365 coupon. Repricing the
mortgage with that shortcut would change the contract and would not reproduce
par.

## Example and scenarios

Run `cargo run -p valuation-loans` for the complete example. It uses sample
EUR market and borrower-credit data in
`examples/valuation/loans/data/loan_*.json`.

`Scenario::new(vec![Shock::new(target, value, ScenarioType::Absolute)])`
creates a quote shock. `scenario.apply(&context)?` returns a
`MarketDataProvider` that pricers and `Evaluator` accept. The original context
is unchanged.

Funding-spread sensitivities keep the original quote identifiers
`ZeroRate_EUR_OTHER:FUNDINGSPREAD_<tenor>`. Multiply decimal-rate derivatives by
`0.0001` to get the approximate EUR change for a one-basis-point move.
