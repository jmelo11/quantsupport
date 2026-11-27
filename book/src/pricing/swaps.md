# Swaps and discounted cashflows

A swap is valued by projecting its floating coupons and discounting the
payments on both legs.

- Coupons whose fixing is already known use historical fixings.
- The remaining rates come from forward curves.
- The discount policy chooses the discount curve for each payment.

For the payments included in the valuation,

\\[
V=\sum_{\ell}s_\ell\sum_i
C_{\ell i}\\,P_{d,\ell}(0,T_{\ell i})\\,X_{\ell i}.
\\]

Here \\(s_\ell\\) is the leg direction and \\(X_{\ell i}\\) is the FX
conversion factor, when the pricer converts currencies. Which payments are
included and how currencies are converted depend on the pricer. The rules for
the generic pricer are below.

## Regular fixed–floating swaps

`MakeSwap` builds the fixed leg as leg zero and the floating leg as leg one.
`SwapTrade` adds the position details. A receive-fixed swap uses `LongReceive`
on its fixed side. A simple fixed coupon is \\(C_i=NK\tau_i\\). Other
compounding conventions use the coupon's compound factor instead.

A regular floating coupon has one fixing.

- For a past fixing, the pricer uses the stored historical fixing. If it is
  missing, pricing fails.
- Otherwise, it projects the rate from the forward curve.
- The spread is then added using the coupon's conventions.

Choosing `SOFR` as the index does not turn the coupon into a daily compounded
overnight coupon; use `MakeOvernightIndexSwap` for that.

Projection and discounting are separate. For example, a Term SOFR coupon can be
projected on `TermSOFR3m` and discounted on `SOFR` under a USD CSA. If you do
not set a discount policy, the generic pricer tries to infer the curves from
the legs, and ambiguous cases fail. For production multi-curve setups, set the
collateral assumptions explicitly.

## Payment selection and reporting

The generic `DiscountedCashflowPricer` values every cashflow the instrument
provides. It does not drop payments dated on or before the evaluation date.
If a leg contains past payments, remove them yourself. The bond and loan pricers
have their own payment-selection rules.

The default `HandleCashflows` report shows signed amounts, including principal
and initial disbursements. It applies the leg direction and flips the sign of
disbursements: receipts are positive and payments are negative. Use its payment
dates, leg indices, currencies, and fixings to inspect the contract. To
reconcile the table to PV, you also need payment selection, discount factors,
and any currency conversion.

In the [first valuation](../getting-started/first-swap.md), the builder
creates principal exchanges on both legs, so the cashflow table has 24 rows.

## Fair fixed rate

For a vanilla fixed-versus-floating swap, the usual formula is

\\[
K^\ast=\frac{PV_{\rm float}}{A},\qquad
A=\sum_iN_i\tau_iP_d(0,T_i).
\\]

`Request::FairRate(index)` selects a leg by zero-based index:

- On a vanilla swap, target 0 solves for the fixed coupon and target 1 for the
  floating spread.
- The result is the single rate or spread that makes the total trade PV zero.
  All other legs keep their quotes, principal exchanges, projection indices,
  discount policy, and signs.
- All coupons of the selected leg are replaced with that common coupon or
  spread.
- Request one target per evaluation.

These cases are errors: a selected leg that mixes coupon types, daily spread
compounding, and a leg with no rate exposure.

The same rule applies to linear basis and cross-currency legs with a compatible
discount policy.

## Overnight index swaps

`MakeOvernightIndexSwap` builds an `OvernightIndexSwap` with coupon-only legs
and the overnight conventions of the chosen index:

```rust
# extern crate quantsupport;
use quantsupport::prelude::*;

let rd = Date::new(2026, 1, 2);
let ois = MakeOvernightIndexSwap::<Recorded>::new(
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

- A standard coupon that starts in the future is valued as a ratio of discount
  factors.
- A coupon that has already started combines weighted past daily fixings with
  a projection for the rest of the period.
- Lookback, lockout, observation shift, daily spread treatment, and payment lag
  are separate inputs.

The [calibration chapter](../curves/bootstrapping.md) lists the market presets
and the limits of the legacy XVA claim converter.

Basis swaps apply the same logic to two floating indices. Their value and risk
depend on both projection curves and on the discount curve chosen by the
policy. Basis swaps are also used to calibrate curves, so a basis curve's
derivative can carry risk to the parent OIS quotes through the IFT.

## Related linear products

- Deposits and floating-rate notes are valued from their legs in the same way.
- Straight bonds need settlement, quote anchors, and yield and spread
  analytics, so they have their own [bond pricer](bonds.md).
- Listed rate futures use `RateFuturesPricer`. The implied rate is the decimal
  \\((100-Q)/100\\).

The futures contract size is the value per **price point**, not per basis
point. A one-basis-point change in the rate moves a `100-100r` quote by 0.01
price points. Keep these units in mind before comparing futures risk with swap
sensitivities.
