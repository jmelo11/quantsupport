# Loan Contracts and Pricing Requests

`Loan<T>` owns a contractual `Leg<T>` containing the existing `FixedRateCoupon`,
`FloatingRateCoupon`, `OptionEmbeddedCoupon`, `Redemption`, and `Disbursement` cashflow types. Coupon
notionals and principal redemptions are `f64` contractual amounts. The instrument
can accept an explicitly constructed leg; it does not select an amortization
structure or infer defaults.

`MakeLoan<T>` generates that leg. Its defaults are monthly payments, French
amortization, Actual/365 accrual, a null calendar, unadjusted payments, zero
fixing lag, no prepayment or credit, and 40% recovery. Set the calendar, fixing
lag, and payment convention to the contract's actual terms.

Coupons use the same positional list as `SteppedCouponBond`: Rust accepts
`with_coupons(Vec<CashflowType<T>>)` and Python accepts the corresponding `FixedRateCoupon`, `FloatingRateCoupon`, and
`OptionEmbeddedCoupon` objects in `coupons=[...]`.
The final supplied coupon repeats through maturity. For monthly payments,
`[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.04))] * 12` followed by
`[qs.FloatingRateCoupon(index=qs.MarketIndex.EURIBOR12m, spread=0.01)]` defines
twelve fixed periods followed by floating periods. Lists can also step fixed coupons,
spreads, or floating indices. Empty lists, non-coupon entries, and lists longer
than the generated payment schedule are rejected. Python uses the same native coupon classes as stepped-coupon bonds.
`OptionEmbeddedCoupon` accepts a native `PayoffOps` expression, such as
`(2 * qs.PayoffOps.Index + 0.005).max(0.02).min(0.09)`. The expression evaluates
the observed index; the coupon's separate spread is added afterward.
Serialization preserves the complete expression tree.

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

French and balloon loans whose first coupon is floating require an explicit initial
all-in `amortization_rate`. The builder uses it to fix the principal schedule.
Subsequent market moves or reset observations change floating interest without
changing contractual coupon notionals or redemptions. Otherwise the first fixed
coupon supplies the initial amortization rate for every principal period;
later coupon steps do not automatically recast principal. Bullet loans repay
principal at maturity.

```rust,ignore
use quantsupport::prelude::*;
use quantsupport::time::calendars::target::TARGET;

let loan = MakeLoan::<DualFwd>::default()
    .with_identifier("EUR-MORTGAGE".into())
    .with_start_date(Date::new(2026, 2, 24))
    .with_maturity_date(Date::new(2056, 2, 24))
    .with_notional(250_000.0)
    .with_currency(Currency::EUR)
    .with_coupons(vec![CashflowType::floating(MarketIndex::EURIBOR12m, 0.01, None, None)])
    .with_initial_amortization_rate(0.05)
    .with_payment_frequency(Frequency::Monthly)
    .with_day_counter(DayCounter::Actual360)
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
observation from the coupon's interest accrual interval. Coupon changes occur at
payment accrual boundaries. Each contiguous run of a floating index starts a
new reset grid; changing only its spread or bounds preserves the existing grid.

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
let loan = builder.with_custom_prepayment_model(
    |state: &PrepaymentModelState<DualFwd>| -> Result<DualFwd> {
        // Illustrative annual CPR converted to this interval's probability.
        Ok(DualFwd::scalar(1.0 - 0.88_f64.powf(state.interval_year_fraction())))
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

Refinancing-incentive models receive an explicitly configured index benchmark.
Its observation tenor comes from the index's metadata. This benchmark excludes
replacement-loan lending spreads, fees, and transaction costs. The state exposes
it as `refinancing_benchmark_rate()`, alongside `contractual_coupon_rate()` and
`outstanding_principal()`. Models return an interval probability through
`conditional_prepayment_probability()`. Native prepayment functions can retain
AD dependencies on market rates without changing the cashflows' `f64` notionals.

## Valuation and Cashflows

`LoanPricer` implements the standard request handlers. `Request::Cashflows`
uses `HandleCashflows` over projected legs containing existing cashflow variants:
leg 0 contains scheduled coupons/redemptions, leg 1 prepayment redemptions, and
leg 2 recovery amounts. The table reports expected signed receipts strictly after
the evaluation date. Their discounted sum reproduces `Value`. The initial
advance remains part of the contractual leg but is excluded from receipt PV.

Bounded floating templates retain `OptionEmbeddedCoupon`. Their floor/cap payoff
is applied to the resolved index observation with the loan's accrual basis.
This is a deterministic cashflow projection; it does not add option time value.

With a borrower `credit_index`, receipts are weighted by survival conditional
on the evaluation date. Default between payments pays recovery on opening
principal at the period-end payment date. This is a discrete recovery timing
approximation. Credit and prepayment affect pricing state, not the contract.

```python
import quantsupport as qs

loan = qs.Loan(
    identifier="EUR-MORTGAGE", start_date=qs.Date(year=2026, month=2, day=24),
    maturity_date=qs.Date(year=2056, month=2, day=24), notional=250_000.0,
    coupons=[qs.FloatingRateCoupon(index=qs.MarketIndex.EURIBOR12m, spread=0.01)],
    currency=qs.Currency.EUR,
    repayment=qs.LoanRepaymentStructure.french(), amortization_rate=0.05,
    payment_frequency=qs.Frequency.Monthly, day_counter=qs.DayCounter.Actual360,
    calendar=qs.Calendar.TARGET, fixing_lag=2,
    prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(cpr=0.12),
)
trade = qs.Trade(instrument=loan, trade_date=reference_date, side=qs.Side.LongReceive)
discount_policy = qs.FixedIncomeDiscountPolicy(risk_free_indices={qs.Currency.EUR: qs.MarketIndex.ESTR})
with qs.PricingContext(quotes=quotes, curves=curves, fixings=fixings, csa_policy=csa_policy,
                      fixed_income_policy=discount_policy) as context:
    result = context.evaluate(trade=trade, requests=[qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities])
    value = result.price
    expected_receipts = result.cashflows
    risk = result.sensitivities
```

The Python constructor delegates to the Rust builder. `to_dict`, `from_dict`,
and `from_json` serialize construction parameters. Typed definitions and their
plain serialized dictionaries are both accepted. `loan.coupon_fixing_requests()` lists
unique coupon observations for loading fixings; valuation belongs to the pricer.

`qs.LoanPricer(refinancing_benchmark_index=qs.MarketIndex.EURIBOR12m)` selects the benchmark
for a refinancing-incentive model. An optional `discount_policy` overrides the context's
fixed-income policy. Loans carry their payment currency, while the policy selects
the discount curve. The same contract can therefore be valued against ESTR,
EURIBOR, or a funding curve by changing the policy. Native Rust valuation requires
a configured policy. In Python, the default context policy maps its base currency
to its base curve when no explicit fixed-income policy is supplied.

Sensitivities are derivatives with respect to labelled market pillars. Multiply
a rate-quote derivative by 0.0001 for the first-order PV change from a one-basis-point
increase. Native refinancing and survival weights participate in AD; contractual
principal schedules stay fixed during market sensitivity calculations.

## Fair Rate and Schedule Choice

`FairRate` solves a fixed coupon or floating spread that makes receipt PV equal
the PV of the original advance. It is defined at or before origination for
uniform fixed coupons or a uniform floating spread. Mixed, stepped, and bounded
coupons require a specified adjustable segment and are currently rejected by
`FairRate`; their Value, Cashflows, and Sensitivities requests remain supported.

The pricer's `rebuild_amortization_for_fair_rate` parameter controls how solver trials treat
principal. The default `false` preserves the already created amortization
schedule. With `true`, each trial regenerates the builder's French, bullet, or
balloon schedule using the candidate fixed coupon or the first observed index
rate plus candidate spread. A custom leg needs explicit repayment metadata to
use this mode. Neither mode changes the original instrument.

```python
preserve = qs.LoanPricer(rebuild_amortization_for_fair_rate=False)
rebuild = qs.LoanPricer(rebuild_amortization_for_fair_rate=True)
with context:
    fair_with_existing_principal = preserve.evaluate(trade=trade, context=context, requests=[qs.Request.FairRate]).fair_rate
    fair_with_new_amortization = rebuild.evaluate(trade=trade, context=context, requests=[qs.Request.FairRate]).fair_rate
```

In Rust, use `LoanPricer::new().with_fair_rate_amortization_rebuild(true)` for rebuilding.
The solver searches within [-5%, 50%], reducing the upper bound when a trial
rate would produce invalid amortization. It reports unbracketed solutions as errors.
To reprice a preserving-mode solution through the builder, retain the original
initial amortization rate while changing the coupon/spread. To reprice a
rebuilding-mode solution, use the candidate all-in rate for amortization.

Runnable examples remain in their separate product directories:

- Rust: `cargo run -p valuation-loans`.
- Python: `python bindings/python/examples/run_notebooks.py valuation/loans`.

Both use the illustrative EUR and borrower-credit inputs in
`examples/valuation/loans/data/loan_*.json`. The mortgage notebook develops
amortization, annual EURIBOR resets, composite cost-of-funds discounting,
market scenarios, prepayment, credit, and quote sensitivities over thirty years.
It reuses one context by assigning `context.curves` and `context.fixed_income_policy`.
Temporary benchmark and funding-spread shocks use `with context, scenario:` with
`qs.Scenario(shocks=[qs.Shock(target=..., value=..., shock_type=...)])`. Pricing
constructs requested market objects in an independent view. The base context
retains its inputs and curves, including when the scope raises an exception.

In Rust, `Scenario::new(vec![Shock::new(target, value, ScenarioType::Absolute)])`
creates the same combination. `scenario.apply(&context)?` returns a borrowed
`MarketDataProvider` accepted by pricers and `Evaluator`; the context stays unchanged.
Funding-spread sensitivities use the explicit zero-rate pillar labels
`FUNDINGSPREAD_<date>`. The notebook isolates those derivatives by funding tenor,
reports EUR changes for +1 bp, and compares the parallel funding exposure with
fully repriced funding stress.
