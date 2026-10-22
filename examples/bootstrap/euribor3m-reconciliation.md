# Euribor 3M reconciliation through one year

Run the supplied 1 October 2026 snapshot and alternative conventions:

```sh
cargo run -p bootstrap --bin euribor3m-reconciliation
```

The executable compares the reported factors with curves built using 30/360
or ACT/360 fixed accrual, annual or semiannual fixed payments, alternative
settlement lags, and either ESTR discounting or a single curve. It checks both
annual day counts against a separate cashflow recursion with a `1e-10` DF
tolerance. It also verifies that changing
the reference date from 2 October to 1 October and normalizing to 2 October
preserves the reconstructed factors.

## Findings

Euribor `Swap` quotes now default to **annual Thirty360 (bond basis)** fixed
coupons and floating payments at the index tenor, with **ACT/360** floating
accrual. Standard settlement remains two TARGET business days. These defaults
replace the previous semiannual ACT/360 fixed and quarterly floating defaults.
Explicit frequency and settlement overrides still take precedence.

The optional fixed day counter follows settlement days:

```text
Swap_EUR_EURIBOR3m_1Y_Annual_Quarterly_1_Thirty360
Swap_EUR_EURIBOR3m_1Y___1_Thirty360
```

Both select the same Euribor 3M convention and a one-business-day start lag.
The day count is also available through `QuoteDetails::with_fixed_day_counter`
and the serialized `fixed_day_counter` field. It changes only fixed coupons.

The old annual **ACT/360** numerical comparison is retained as a diagnostic.
It gives the closest tested fit to this particular table, which does not
establish the source's contract conventions. Its assumptions are:

- Effective date **2 October 2026**, TARGET Modified Following schedules.
- **Annual fixed ACT/360**, quarterly Euribor 3M ACT/360, simple coupon rates.
- The supplied ESTR quotes as **OIS** instruments, annual coupons, zero payment
  lag, and the same effective date; ESTR discounts both swap legs.
- Log-linear discount-factor interpolation. All quarterly payment dates in
  this reconstruction coincide with supplied OIS and Euribor pillars.
- Projection factors normalized to **2 October**, so the factor at that date
  is one. These are Euribor projection factors, distinct from the ESTR factors
  used to discount swap cashflows.

| Maturity | Supplied DF | Annual 30/360 fixed | Annual ACT/360 fixed |
| --- | ---: | ---: | ---: |
| 2027-01-04 | 0.993260 | 0.993404457 | 0.993262041 |
| 2027-04-02 | 0.986120 | 0.986279527 | 0.986129093 |
| 2027-07-02 | 0.978322 | 0.978557687 | 0.978323326 |
| 2027-10-04 | 0.970002 | 0.970428597 | 0.970027516 |

These are tested reconstruction assumptions, not a claim about the source's
contract conventions. In particular, annual ACT/360 must not be inferred from
the curve name alone. A standard Euribor swap setup in
[QuantLib's own tests](https://github.com/lballabio/QuantLib/blob/master/test-suite/overnightindexedswap.cpp)
uses annual **30/360** fixed coupons. The corrected `Swap` quote builder now
separates that fixed convention from the floating index. Changing the curve's
`day_counter` only changes interpolation time, not contractual coupon accrual.

### Fixed payment frequency

Before the correction, `Swap_EUR_EURIBOR3m_1Y` defaulted to **semiannual**
ACT/360 fixed payments. That produces a 1Y factor of `0.969800211` in this
reproduction. The annual ACT/360 diagnostic now specifies its nonstandard
fixed day count explicitly:

```text
Swap_EUR_EURIBOR3m_3M_Annual_Quarterly_1_Actual360
Swap_EUR_EURIBOR3m_6M_Annual_Quarterly_1_Actual360
Swap_EUR_EURIBOR3m_9M_Annual_Quarterly_1_Actual360
Swap_EUR_EURIBOR3m_1Y_Annual_Quarterly_1_Actual360
OIS_EUR_ESTR_1W___1_0
...
OIS_EUR_ESTR_1Y___1_0
```

These identifiers assume a reference date of **1 October 2026**. The
regular-swap settlement field explicitly selects a **one-business-day** start lag to
match the supplied start date. The OIS fields similarly select one settlement
day and zero payment lag. Keep each supplied rate in decimal units, e.g.
`0.0302858` for `3.02858%`. For a reference date of **2 October**, select zero
settlement days instead.

### Date basis

With a reference date of 1 October, the default two-business-day lag starts
both swap families on **5 October**, not 2 October. An extraction date does
not necessarily identify the source curve's valuation date; the table does
not resolve why its effective date is 2 October.

`discount_factor(T)` is anchored at the library curve's reference date.
To compare a factor from start date `S` to maturity `T`, use:

```text
spot_df(T) = discount_factor(T) / discount_factor(S)
```

For the annual ACT/360 diagnostic anchored on 1 October, `P(2 October)` is
`0.9999280796`. Its absolute 1Y factor is `0.969957751`; normalizing to
2 October gives `0.970027516`. The first reported factor also supports a
2 October basis: `1 / (1 + 0.02598 * 94 / 360) = 0.993262041`.

### Independent checks and remaining difference

For a single annual fixed payment, let `A_i` be ACT/360 from the common start
to maturity, `D_i` the ESTR factor, `P_i` the Euribor projection factor, and
`K_i` the quoted par rate. The par condition is:

```text
K_i * A_i * D_i = sum(j=1..i, (P_(j-1) / P_j - 1) * D_j),  P_0 = 1
```

The executable independently solves this equation in maturity order. Using
30/360 instead for `A_i` also reproduces the corrected library's standard-swap
factors. Both checks agree within `1e-10`, establishing numerical accuracy for
each constructed contract. They do not establish which contract the source
data represents. Substituting the supplied target DFs instead implies the
following par rates under the annual ACT/360 diagnostic assumptions:

| Tenor | Supplied par (%) | Target-DF implied par (%) | Difference (bp) |
| --- | ---: | ---: | ---: |
| 3M | 2.598000 | 2.59879246 | +0.079246 |
| 6M | 2.781542 | 2.78338176 | +0.183976 |
| 9M | 2.920300 | 2.92049003 | +0.019003 |
| 1Y | 3.028580 | 3.03119087 | +0.261087 |

The annual ACT/360 residual is not removed by tightening the solver tolerance. At
1Y it also exceeds the displayed DF rounding. Matching it requires the
source's precise contract/date conventions and inputs; the table alone does
not identify the cause. With these dates and coupons, changing interpolation
cannot explain it because every payment is already at a quoted node.
Using single-curve discounting gives a 1Y factor of `0.970050006`, a worse fit.
Using the standard annual 30/360 fixed leg gives `0.970428597`; under those
conventions, the supplied target factors imply a 1Y par rate of `3.07305814%`,
about `4.4478` bp above the supplied quote.

In the ACT/360 diagnostic, the 2.598% fixing is represented by the matching
first-quarter par quote. A 30/360 fixed stub at that same quoted rate has a
different accrual amount, so it does not reproduce the ACT/360 money-market
anchor. The source's short-end `Par.Mid` values may therefore need different
calibration instruments or rate interpretations; the table's curve label
alone does not establish that every row is a standard swap quote.

Adding a fixing only to a `FixingStore` does not add a curve
pillar: regular-swap bootstrap coupons currently forecast from the trial
projection curve, while overnight coupons can use the bootstrap fixing store.
