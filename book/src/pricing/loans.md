# Loans, prepayment and credit

A loan promises interest and principal payments according to a contractual
repayment schedule. Its value also depends on expected prepayment and default.
QuantSupport keeps the contractual schedule fixed while the pricer projects
these expected receipts under the chosen market assumptions.

`Loan<T>` stores the contract as a `Leg<T>` of coupons, redemptions and the
initial advance. Coupon notionals and principal redemptions are contractual
`f64` amounts. Supply a leg directly or use `MakeLoan<T>` to generate it from
repayment and schedule terms.

The builder defaults to monthly payments, French
amortization, Actual/365 accrual, a null calendar, unadjusted payments, zero
fixing lag, no prepayment or credit, and 40% recovery. Set the calendar, fixing
lag, and payment convention to the contract's actual terms.

Coupons use the same positional list as `SteppedCouponBond`: Rust accepts
`with_coupons(Vec<CashflowType<T>>)` and Python accepts the corresponding
`FixedRateCoupon`, `FloatingRateCoupon`, and `OptionEmbeddedCoupon` objects in
`coupons=[...]`. The final supplied coupon repeats through maturity. For monthly
payments, `[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.04))] * 12` followed
by `[qs.FloatingRateCoupon(index=qs.MarketIndex.EURIBOR12m, spread=0.01)]`
defines twelve fixed periods followed by floating periods. Lists can also step
fixed coupons, spreads or floating indices. Empty lists, non-coupon entries and
lists longer than the generated schedule are rejected. `OptionEmbeddedCoupon`
accepts a native `PayoffOps` expression, such as `(2 * qs.PayoffOps.Index +
0.005).max(0.02).min(0.09)`. The expression evaluates the observed index; the
coupon's separate spread is added afterward. Serialization preserves the
complete expression tree.

## Contractual principal

The builder computes principal before considering any prepayment function.
For initial principal \\(B\\) and annual amortization rate \\(r\\), let
\\(\tau_k\\) be the contractual day-count fraction and \\(B_f\\) the
final balloon. With \\(B_{k-1}\\) the opening scheduled balance,

\\[
i_k=r\tau_k,\qquad
D_k=\prod_{j=1}^{k}(1+i_j)^{-1},\qquad
A=\frac{B-B_fD_n}{\sum_{k=1}^{n}D_k},
\\]
\\[
I_k=B_{k-1}i_k,\qquad
R_k=A-I_k.
\\]

Here \\(A\\) is the constant scheduled instalment, \\(I_k\\) interest and
\\(R_k\\) scheduled principal.

These annuity factors use contractual interest accrual; they are independent of
market discount curves. Unequal intervals and final stubs use their own
fractions. If every contractual fraction is exactly \\(1/12\\), a 100,000
principal, 6% coupon and 360 payments produce an instalment of 599.550525.
This numerical example assumes equal accrual fractions. Actual schedule
fractions depend on the payment dates and day counter. At zero interest and
with no balloon, the instalment is principal divided by the number of periods.

Rust makers built with `MakeInstrument::build_using` obtain the initial all-in rate from
the first coupon's fixing or forecast, including spread and payoff rules. An
explicit `with_initial_amortization_rate` overrides that observation. Direct
`build()` construction requires an explicit initial all-in `amortization_rate`
for floating-first French/balloon loans. Python makers use `build_using(context)` when that rate is omitted.
Excel constructs the loan using its explicit construction context. The resolved rate fixes
the principal schedule.
Subsequent market moves or reset observations change floating interest without
changing contractual coupon notionals or redemptions. Otherwise the first fixed
coupon supplies the initial amortization rate for every principal period;
later coupon steps do not automatically recast principal. Bullet loans repay
principal at maturity.

Accrual day counters belong to coupons. A floating coupon uses its explicit
day counter, falling back to its index convention when omitted. Fixed coupons
use their `InterestRate` day counter. The loan uses those same conventions for
principal amortization and preserves them when generating each cashflow.
`MakeLoan::with_day_counter` and Python `Loan(day_counter=...)` have been removed;
move that setting into the coupon. With an initial amortization rate matching
the first all-in floating fixing, French payments are level through the first
reset, including months with different accrual lengths. Later resets can change
payments because contractual principal remains fixed.

The following Rust fragment builds a floating mortgage; its market and trade
are supplied by the surrounding application.

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

## Prepayment functions

A `Loan<T>` carries a `PrepaymentModel<T>` function independently of its
contractual cashflows. `PrepaymentModelConfiguration::Cpr`, `CprCurve`, and
`Refinancing` implement that function for any supported scalar. Custom models
passed to `MakeLoan<T>::with_custom_prepayment_model` use the same `T` as the
coupon templates. For a `MakeLoan<DualFwd>` already containing the
contract terms, a custom function can be
attached as in this Rust fragment:

```rust,ignore
let loan = builder.with_custom_prepayment_model(
    |state: &PrepaymentModelState<DualFwd>| -> Result<DualFwd> {
        // Illustrative annual CPR converted to this interval's probability.
        Ok(DualFwd::scalar(1.0 - 0.88_f64.powf(state.interval_year_fraction())))
    },
).build()?;
```

Changing a loan's scalar type requires an explicit target-scalar prepayment
model. Use `loan.into_scalar::<DualFwd>(Arc::new(model))`, and the same method on
`LoanTrade<T>` to preserve its date and side. For portable configurations, pass
the same configuration again. For custom models, supply an equivalent
implementation for the target scalar; converting an arbitrary model
automatically could discard its AD dependencies. Implicit `From`/`Into`
conversions between loan scalar types are therefore unavailable.

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

## Valuation and cashflows

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
    payment_frequency=qs.Frequency.Monthly,
    calendar=qs.Calendar.TARGET, fixing_lag=2,
    prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(cpr=0.12),
)
trade = qs.Trade(instrument=loan, trade_date=reference_date, side=qs.Side.LongReceive)
discount_policy = qs.FixedIncomeDiscountPolicy(discount_curve_by_currency={qs.Currency.EUR: qs.MarketIndex.ESTR})
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

`context.evaluate(qs.Trade(loan, reference_date), requests)` dispatches directly
to the core Rust `LoanPricer`. Use `PricerConfiguration.loan(...)` with Python
arguments and a typed `MarketIndex` to select fair-rate and refinancing settings:

```python
context.pricer_configurations = [qs.PricerConfiguration.loan(
    refinancing_benchmark_index=qs.MarketIndex.EURIBOR12m,
)]
```

Omitting this configuration uses the native defaults: no refinancing benchmark
and preserved amortization during fair-rate trials. The context's
`fixed_income_policy` selects discounting. Loans carry their payment currency,
while the policy selects the discount curve. The same contract can therefore be
valued against ESTR, EURIBOR, or a funding curve by changing the policy. Native
Rust valuation requires a configured policy. In Python, the default context
policy maps its base currency to its base curve when no explicit fixed-income
policy is supplied.
The same native settings apply to individual loan trades, portfolio members and
scenarios. No Python pricer registration or trade subclass is needed.

Sensitivities are derivatives with respect to labelled market pillars. Multiply
a rate-quote derivative by 0.0001 for the first-order PV change from a one-basis-point
increase. Native refinancing and survival weights participate in AD; contractual
principal schedules stay fixed during market sensitivity calculations.

## Fair rate and schedule choice

`FairRate` bootstraps one fixed coupon or floating spread per contiguous
contractual segment at or before origination. A change in fixed rate, spread,
coupon type, or floating index starts a segment; index fixing resets do not.
First shorten the loan to the first segment's end and repay all remaining
capital there. Solve that segment's rate so receipt PV equals the original
advance PV, lock the result, extend through the next segment, and repeat.
Every solved prefix therefore lends at par with its outstanding principal
repaid at that prefix's end. Bounded coupons remain unsupported.

`Request::FairRate(index)` in Rust and `qs.Request.FairRate(index)` in Python
select one zero-based segment and return its scalar in `fair_rate`. Only the
earlier segments needed for that target are solved. Use separate evaluations
for several targets; invalid indices are errors. There is no second fair-rate
attribute or implicit array. In Excel, `QS.REQUEST.FAIR.RATE(index)` supplies
the request to `QS.EVALUATE`, then `QS.RESULT.FAIR.RATE` reads its scalar.

```python
with context:
    second_rate = context.evaluate(
        trade=trade, requests=[qs.Request.FairRate(1)]
    ).fair_rate
```

The pricer's `rebuild_amortization_for_fair_rate` parameter controls how solver
trials treat principal. The default `false` preserves the already created
amortization schedule. With `true`, each trial regenerates the builder's French,
bullet, or balloon schedule using the first trial coupon's all-in rate. Once
the first segment is solved, its rate stays fixed during later trials. A custom leg needs explicit repayment
metadata to use this mode. Neither mode changes the original instrument.

```python
original_configurations = context.pricer_configurations
try:
    for rebuild in [False, True]:
        context.pricer_configurations = [qs.PricerConfiguration.loan(
            rebuild_amortization_for_fair_rate=rebuild,
        )]
        with context:
            fair_rate = context.evaluate(trade, [qs.Request.FairRate(0)]).fair_rate
            print(rebuild, fair_rate)
finally:
    context.pricer_configurations = original_configurations
```

In Rust, use `LoanPricer::new().with_fair_rate_amortization_rebuild(true)` for
rebuilding. The solver searches within [-5%, 50%], reducing the upper bound when
a trial rate would produce invalid amortization. It reports unbracketed
solutions as errors. To reprice a preserving-mode solution through the builder,
retain the original initial amortization rate while changing the coupon/spread.
To reprice a rebuilding-mode solution, use the candidate all-in rate for
amortization.

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
Funding-spread sensitivities use the original zero-rate quote identifiers
`ZeroRate_EUR_FUNDINGSPREAD_<tenor>`. The notebook isolates those derivatives by funding tenor,
reports EUR changes for +1 bp, and compares the parallel funding exposure with
fully repriced funding stress.
