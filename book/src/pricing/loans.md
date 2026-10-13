# Loan Contracts and Pricing Requests

`Loan<T>` owns a contractual `Leg<T>` containing the existing `FixedRateCoupon`,
`FloatingRateCoupon`, `Redemption`, and `Disbursement` cashflow types. Coupon
notionals and principal redemptions are `f64` contractual amounts. The instrument
can accept an explicitly constructed leg; it does not select an amortization
structure or infer defaults.

`MakeLoan<T>` generates that leg. Its defaults are monthly payments, French
amortization, Actual/365 accrual, a null calendar, unadjusted payments, zero
fixing lag, no prepayment or credit, and 40% recovery. Set the calendar, fixing
lag, and payment convention to the contract's actual terms.

## Contractual Principal

The builder computes principal before considering any prepayment function.
For opening balance B, annual initial amortization rate r, and accrual interval k:

```text
tau[k] = day_counter.year_fraction(accrual_start[k], accrual_end[k])
i[k] = r * tau[k]
D[k] = product(1 / (1 + i[j]), j = 1..k)
A = (B - final_balloon * D[n]) / sum(D[k], k = 1..n)
interest[k] = opening_balance[k] * i[k]
scheduled_principal[k] = A - interest[k]
```

These annuity factors use contractual interest accrual; they are independent of
market discount curves. Unequal intervals and final stubs use their own
fractions. For equal monthly Thirty/360 intervals, a 100,000 principal, 6% coupon,
and 360 payments produce an instalment of 599.550525. At zero interest and no
balloon, the instalment is principal divided by the number of periods.

French and balloon loans with floating coupons require an explicit initial
all-in `amortization_rate`. The builder uses it to fix the principal schedule.
Subsequent market moves or reset observations change floating interest without
changing contractual coupon notionals or redemptions. A fixed loan defaults to
its fixed coupon for amortization; a mixed loan defaults to its initial fixed
coupon. Bullet loans repay principal at maturity.

```rust,ignore
use quantsupport::prelude::*;
use quantsupport::time::calendars::target::TARGET;

let loan = MakeLoan::<DualFwd>::default()
    .with_identifier("EUR-MORTGAGE".into())
    .with_start_date(Date::new(2026, 2, 24))
    .with_maturity_date(Date::new(2028, 2, 24))
    .with_notional(100_000.0)
    .with_currency(Currency::EUR)
    .with_discount_index(MarketIndex::ESTR)
    .with_rate(LoanRateDefinition::Floating {
        index: MarketIndex::EURIBOR12m,
        spread: 0.01,
    })
    .with_amortization_rate(0.05)
    .with_payment_frequency(Frequency::Monthly)
    .with_day_counter(DayCounter::Actual360)
    .with_calendar(Calendar::TARGET(TARGET::new()))
    .with_fixing_lag(2)
    .build()?;

let contractual_cashflows = loan.leg().cashflows();
```

## Coupon Observation Requests

Payment frequency, reset frequency, and index tenor are independent.
`RateIndexDetails::frequency()` defines the index observation frequency, separately
from its quote compounding frequency. The builder reads this metadata rather
than matching particular index names. EURIBOR12m, for example, supplies annual
resets and a 12-month observation tenor for monthly-payment loans. Explicit
builder frequencies and tenors override the metadata. Daily indices default to
point observations at the loan's payment boundaries; specify the loan's actual
reset conventions explicitly when these differ.

Each floating coupon carries a `ForwardRateRequest` for its applicable reset.
Monthly coupons can share the same annual request. The request contains the
fixing date and full index observation start/end dates. This separates the rate
observation from the coupon's interest accrual interval. Reset dates and mixed
coupon switches must coincide with payment accrual boundaries.

The pricer resolves required historical resets from exact stored fixings. Today's
fixing is used when present and forecast otherwise. Future observations use the
index curve and its quote convention. It installs fixings on a cloned leg,
leaving the instrument unchanged.

## Prepayment Functions

A loan carries a `PrepaymentModel<DualFwd>` function independently of its
contractual cashflows. `PrepaymentModelConfiguration::Cpr`, `CprCurve`, and
`Refinancing` implement that function. The builder can also attach a custom
function:

```rust,ignore
let loan = builder.with_prepayment_function(
    |state: &PrepaymentModelState<DualFwd>| -> Result<DualFwd> {
        // Illustrative annual CPR converted to this interval's probability.
        Ok(DualFwd::scalar(1.0 - 0.88_f64.powf(state.year_fraction())))
    },
).build()?;
```

The pricer evaluates the function after scheduled principal. A regular monthly
interval converts annual CPR to `1 - (1 - CPR)^(1/12)`; partial or irregular
intervals use actual days divided by 365. Prepayment is a principal receipt.
The projected remaining share of the contractual balance reduces future expected
interest and scheduled principal proportionally. No contractual leg is modified.
For seasoned loans the projection starts from contractual outstanding principal
at the evaluation date; prior actual prepayments are not inferred.

Refinancing functions receive an explicitly configured market refinancing rate.
Its observation tenor comes from that index's metadata. Native prepayment
functions can retain AD dependencies on market rates without changing the
cashflows' `f64` coupon notionals.

## Valuation and Cashflows

`LoanPricer` implements the standard request handlers. `Request::Cashflows`
uses `HandleCashflows` over projected legs containing existing cashflow variants:
leg 0 contains scheduled coupons/redemptions, leg 1 prepayment redemptions, and
leg 2 recovery amounts. The table reports expected signed receipts strictly after
the evaluation date. Their discounted sum reproduces `Value`. The initial
advance remains part of the contractual leg but is excluded from receipt PV.

With a borrower `credit_index`, receipts are weighted by survival conditional
on the evaluation date. Default between payments pays recovery on opening
principal at the period-end payment date. This is a discrete recovery timing
approximation. Credit and prepayment affect pricing state, not the contract.

```python
import quantsupport as qs

loan = qs.Loan(
    identifier="EUR-MORTGAGE", start_date="2026-02-24",
    maturity_date="2028-02-24", notional=100_000.0,
    rate=qs.LoanRateDefinition.floating("EURIBOR12m", spread=0.01),
    currency="EUR", discount_index="ESTR",
    repayment=qs.LoanRepaymentStructure.french(), amortization_rate=0.05,
    payment_frequency="Monthly", day_counter="Actual360",
    calendar="TARGET", fixing_lag=2,
    prepayment=qs.PrepaymentModelConfiguration.cpr(0.12),
)
trade = qs.Trade(loan, reference_date, qs.Side.LongReceive)
with qs.PricingContext(quotes, curves, fixings=fixings, csa_policy=csa_policy) as context:
    result = context.evaluate(trade, ["Value", "Cashflows", "Sensitivities"])
    value = result.price
    expected_receipts = result.cashflows
    risk = result.sensitivities
```

The Python constructor delegates to the Rust builder. `to_dict`, `from_dict`,
and `from_json` serialize construction parameters. Typed definitions and their
plain serialized dictionaries are both accepted. `loan.fixing_requests()` lists
unique coupon observations for loading fixings; valuation belongs to the pricer.

`qs.LoanPricer(refinancing_index="EURIBOR12m")` configures a refinancing
function's market input. An optional `discount_policy` overrides the context's
fixed-income policy; otherwise the instrument discount index supplies the curve.

Sensitivities are derivatives with respect to labelled market pillars. Multiply
a rate-quote derivative by 0.0001 for the first-order PV change from a one-basis-point
increase. Native refinancing and survival weights participate in AD; contractual
principal schedules stay fixed during market sensitivity calculations.

## Fair Rate and Schedule Choice

`FairRate` solves a fixed coupon or floating spread that makes receipt PV equal
the PV of the original advance. It is defined at or before origination; mixed
loans require a specified adjustable coupon segment and are currently rejected.

The pricer's `recompute_schedule` parameter controls how solver trials treat
principal. The default `false` preserves the already created amortization
schedule. With `true`, each trial regenerates the builder's French, bullet, or
balloon schedule using the candidate fixed coupon or the first observed index
rate plus candidate spread. A custom leg needs explicit repayment metadata to
use this mode. Neither mode changes the original instrument.

```python
preserve = qs.LoanPricer(recompute_schedule=False)
rebuild = qs.LoanPricer(recompute_schedule=True)
with context:
    fair_with_existing_principal = preserve.evaluate(trade, context, ["FairRate"]).fair_rate
    fair_with_new_amortization = rebuild.evaluate(trade, context, ["FairRate"]).fair_rate
```

In Rust, use `LoanPricer::new().with_recompute_schedule(true)` for rebuilding.
The solver uses the bracket [-5%, 50%] and reports unbracketed solutions as errors.
To reprice a preserving-mode solution through the builder, retain the original
initial amortization rate while changing the coupon/spread. To reprice a
rebuilding-mode solution, use the candidate all-in rate for amortization.

Runnable examples remain in their separate product directories:

- Rust: `cargo run -p valuation-loans`.
- Python: `python bindings/python/examples/run_notebooks.py valuation/loans`.

Both use the illustrative EUR and borrower-credit inputs in
`examples/valuation/loans/data/loan_*.json` and independently verify principal,
cashflow PV, both fair-rate modes, and quote sensitivity.
