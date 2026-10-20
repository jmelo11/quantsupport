# Swaps and discounted cashflows

An interest-rate swap is valued by projecting its floating coupons and
discounting the payments on both legs. Historical fixings determine coupons
whose observations are already known; forward curves supply the remaining
rates. The discount policy selects the curve appropriate to each payment.

For the payments included in the valuation, we can write

\\[
V=\sum_{\ell}s_\ell\sum_i
C_{\ell i}\\,P_{d,\ell}(0,T_{\ell i})\\,X_{\ell i}.
\\]

Here \\(s_\ell\\) is leg direction and \\(X_{\ell i}\\) is the applicable
conversion factor when the pricing policy calls for conversion. The pricer's
payment-selection and currency-conversion conventions determine which terms
enter the sum. The generic implementation's conventions are described below.

## Regular fixed–floating swaps

`MakeSwap` builds fixed leg zero and floating leg one; `SwapTrade` supplies
position metadata. A receive-fixed contract uses `LongReceive` on its fixed
side. For simple fixed coupons, \\(C_i=NK\tau_i\\); other compounding
conventions use the coupon's compound factor instead.

A regular floating coupon has a single observation. The pricer uses required
historical fixings for past observations and otherwise projects from the
forward curve. The spread is then added under the coupon's conventions.
Missing required history is an error. Choosing `SOFR` as the index does not
change this into overnight daily accrual.

Projection and discounting are separate. A Term SOFR coupon can project from
`TermSOFR3m` while a USD CSA discounts it on `SOFR`. Without an explicit
discount policy, the generic pricer attempts to infer curves from leg
identities; ambiguous choices can fail. Production multi-curve setups should
make collateral assumptions explicit.

## Payment selection and reporting

The generic `DiscountedCashflowPricer` iterates the cashflows supplied by the
instrument; it does not apply a universal `payment_date > evaluation_date`
filter. Do not use it as an implicit settlement/seasoning filter for an
arbitrary existing leg. Specialized bond and loan pricers apply their own
payment-selection conventions.

The default `HandleCashflows` report shows signed amounts, including principal
and initial disbursements where present. It applies leg direction and reverses
disbursements: receipts are positive and payments are negative.
Use its payment dates, leg indices, currencies and observations to inspect
the contract. Reconciling the table to PV also requires payment selection,
discount factors and any currency conversion.

In the [first valuation](../getting-started/first-swap.md), the builder produces
principal exchanges on both sides, giving 24 rows in the cashflow table.

## Fair fixed rate

For a vanilla fixed-versus-floating coupon swap under a specified market,
the familiar expression is

\\[
K^\ast=\frac{PV_{\rm float}}{A},\qquad
A=\sum_iN_i\tau_iP_d(0,T_i).
\\]

`Request::FairRate(index)` selects a zero-based leg. Target 0 solves the fixed
coupon and target 1 the floating spread on a vanilla swap. The scalar result
makes total trade PV zero, retaining all other legs' quotes, principal exchanges,
projection indices, discount policy and leg signs. Selected fixed or floating
coupons are replaced by a common coupon/spread; mixed coupon types in one
selected leg, daily spread compounding and legs without rate exposure are rejected. Request one target per
evaluation. The same selection applies to linear basis and cross-currency legs
under a compatible native discount policy.

## Overnight index swaps

`MakeOvernightIndexSwap` constructs `OvernightIndexSwap` with coupon-only legs
and index-specific overnight conventions:

```rust
# extern crate quantsupport;
use quantsupport::prelude::*;

let rd = Date::new(2026, 1, 2);
let ois = MakeOvernightIndexSwap::<DualFwd>::new(
    MarketIndex::SOFR, rd, Period::new(5, TimeUnit::Years), 0.04,
)
.with_identifier("USD_SOFR_OIS_5Y".into())
.with_notional(10_000_000.0)
.with_side(Side::LongReceive)
.build()?;
let _trade = OvernightIndexSwapTrade::new(
    ois, rd, 10_000_000.0, Side::LongReceive,
);
# Ok::<(), QSError>(())
```

For standard future compounding the overnight accumulation telescopes to a
discount-factor ratio. Seasoned periods combine weighted historical daily
observations with projected remainder. Lookback, lockout, observation shift,
daily spread treatment and payment lag are separate inputs; the
[calibration chapter](../curves/bootstrapping.md) explains their market presets
and the legacy XVA conversion limit.

Basis swaps extend the same logic to two floating indices. Their value and
quote risk depend on both projection curves and the policy-selected discount
curve. They are also calibration instruments, so a basis curve's derivative
can carry parent OIS risk through the IFT.

## Related linear products

Deposits and floating-rate notes share leg-based mechanics. Straight bonds
require settlement, quote anchors and yield/spread analytics and therefore
have a specialized [bond pricer](bonds.md). Listed rate futures use
`RateFuturesPricer`: implied rate is the decimal
\\((100-Q)/100\\), and contract size is value per futures **price point**, not
per basis point. A one-basis-point rate change changes a `100-100r` quote by
0.01 price points. These unit distinctions are essential before comparing
their risk with swap sensitivities.
