# Bonds and issuer optionality

A bond is valued from its coupon and principal payments plus a market quote.

- For a straight fixed-rate bond, QuantSupport solves the spread implied by a
  quoted price. It also computes yield and duration.
- For a stepped-coupon bond with issuer calls, valuation also needs a
  short-rate model to value the call option.

| Instrument | Pricer | Supported analytics |
| --- | --- | --- |
| `FixedRateBond` | `BondPricer` | value, cashflows, yield to maturity, modified duration, Z-spread, sensitivities, accrued amount |
| `SteppedCouponBond` | `TreeBasedBondPricer` or `BackwardFourierEvolutionBondPricer` | value, cashflows, OAS, sensitivities, accrued amount |

Stepped-coupon bonds take a price or spread anchor. If you request yield to
maturity or modified duration for them, those result fields stay empty.

## Contractual accrued amount

`Request::AccruedAmount(start, end)` returns the signed coupon interest earned
between two dates. Read it from `EvaluationResults::accrued_amount()`.

- The amount is undiscounted. It is in the context base currency when one is
  set.
- Principal is not included.
- The dates are clipped to each coupon period, so adjacent windows add up to
  the full coupon.

Accrued interest is computed from the coupons alone. It does not need a spread,
a clean or dirty quote, a tree or Fourier model, or AD. For a callable bond,
accrued interest assumes the scheduled coupons are paid; the chance of a future
call is not taken into account.

## Pricing anchors

Every bond trade has exactly one `BondPricingAnchor`. Pricers take no quote
when you create them, so you can reuse one pricer for many trades.

- `CleanPrice(value)` is a price per bond units, excluding accrued interest.
- `DirtyPrice(value)` is a price per bond units, including accrued interest.
- `ZSpread(value)` is a continuously compounded spread for a straight bond.
- `OAS(value)` is an option-adjusted spread for a bond with issuer calls.

A clean or dirty price gives a Z-spread for a straight bond and an OAS for a
callable bond. The pricer rejects an OAS anchor on a straight bond and a
Z-spread anchor on a callable bond.

The spread, whether supplied or solved, is an AD input. The sensitivity table
therefore shows curve risk plus either `dPV/dZSpread` or `dPV/dOAS`.

## Straight fixed-rate bonds

Only coupons and principal paid strictly after settlement are valued. A clean
price is turned into a dirty price by adding accrued interest. The Z-spread
`z` solves

\\[
B_{\rm dirty}(S;z)=\sum_{T_i>S}CF_i
\frac{P(0,T_i)}{P(0,S)}
\exp\\!\left[-z\\,\tau(S,T_i)\right].
\\]

Here \\(S\\) is the settlement date and \\(\tau\\) uses the spread day
counter. The formula gives the settlement value; quoted prices are then scaled
to the bond's units.

- The Z-spread solver widens a bracket, then bisects inside it.
- Yield to maturity is solved separately, using the bond's day count,
  frequency, and compounding.
- Modified duration is `-(dP/dy)/P`, using the exact derivative of each yield
  discount factor.

`Request::Value` returns the signed PV of the position at the evaluation date.
The quoted price sets the spread; it is not the PV.

## Stepped-coupon contract

The builder uses `MakeSchedule` to generate dates. Besides
`payment_frequency`, it accepts:

- a calendar and business-day convention;
- a termination-date convention;
- a date-generation rule and end-of-month flag;
- a first-coupon date and next-to-last-coupon date.

By default, the final date uses the same adjustment as the other dates.

The side is set on the trade (`SteppedCouponBondTrade` or `FixedRateBondTrade`),
together with the pricing anchor and optional settlement and spread day counts.
Instrument builders do not take a side. Cashflows describe the contract; the
pricer applies the trade direction to PV and sensitivities, and claim
conversion applies it to XVA claims.

`SteppedCouponBond` is built from three schedules:

1. `coupons` is positional. Entry zero applies to the first coupon period,
   entry one to the second, and so on. If there are more periods than entries,
   the last entry repeats until maturity. A list longer than the schedule is an
   error.
2. `notional_percentages` is also positional, and its last entry repeats in the
   same way. Each value is the fraction of the original principal outstanding
   in that period. The first value must be `1.0`. Values must stay in `(0, 1]`
   and must not increase. A list longer than the schedule is an error.
3. `call_schedule` is not positional. Each entry has a `start_date`, an
   optional `end_date`, and a `strike`.
   - Without an end date, the entry is a single call date.
   - With an end date, it is an exercise window that includes both ends.
   - Windows are sorted. They must start after the bond starts, end before the
     final payment, and must not overlap, not even at a shared end date.
   - The serialized field `date` is accepted as another name for `start_date`.

Each coupon is one of the `CashflowType<T>` variants:

```text
FixedRateCoupon(FixedRateCoupon<T>)
FloatingRateCoupon(FloatingRateCoupon<T>)
OptionEmbeddedCoupon(OptionEmbeddedCoupon<T>)
```

For a floating fixing `L`, the all-in coupon rate is

\\[
R=\min\\!\left(\max(L+\text{spread},\text{floor}),\text{cap}\right),
\\]

where a missing floor means no floor and a missing cap means no cap. Fixed coupons
use the bond's coupon day counter. Floating coupons use the index's convention.

When the outstanding notional steps down, the reduction is redeemed at the end
of that coupon period. The rest of the principal is redeemed at maturity.

Calls give the issuer the right to redeem the bond. Holder puts are not
supported. Strikes are in bond units and do not include coupons paid on the
same day.

On each date, valuation works backward in this order:

1. discount the value of continuing to hold the bond;
2. apply the issuer call: take `min(V, K)`;
3. add the cashflows paid on that date.

So a coupon or amortization paid on a call date is paid before the rest of the
bond is called.

The following fragment shows the contract and position terms. The context
and market configuration are set up separately.

```rust,ignore
let bond = MakeSteppedCouponBond::<Recorded>::default()
    .with_identifier("USD-STEP-CALL-2031".into())
    .with_start_date(Date::new(2025, 1, 1))
    .with_maturity_date(Date::new(2031, 1, 1))
    .with_notional(1_000_000.0)
    .with_units(100.0)
    .with_currency(Currency::USD)
    .with_payment_frequency(Frequency::Semiannual)
    .with_coupons(vec![
        CashflowType::fixed(0.045),
        CashflowType::floating(
            MarketIndex::SOFR,
            0.006,
            Some(0.0),
            Some(0.06),
        ),
    ])
    .with_notional_percentages(vec![1.0, 0.90, 0.75])
    .with_call_schedule(vec![
        CallScheduleEntry::window(
          Date::new(2028, 1, 1), Date::new(2028, 6, 30), 101.0,
        ),
        CallScheduleEntry::new(Date::new(2029, 1, 1), 100.0),
    ])
    .build()?;

let trade = SteppedCouponBondTrade::new(
    bond, Date::new(2025, 1, 1), 1_000_000.0, Side::LongReceive,
    BondPricingAnchor::OAS(0.0075),
);
let mut pricer = TreeBasedBondPricer::new();
  pricer.set_discount_policy(Box::new(
    FixedIncomeDiscountPolicy::new(false)
      .with_risk_free_index(Currency::USD, MarketIndex::SOFR),
  ));
```

Neither the stepped bond nor its builder has a discount index. The pricer's
`DiscountPolicy` chooses the discount curve. Straight bonds also need a policy;
an index stored on the instrument is not used unless the policy says so.
`FixedIncomeDiscountPolicy::new(prefer_instrument_index)` controls whether the
instrument's index is preferred.

`CashflowType::fixed` and `CashflowType::floating` create coupon templates. The
builder fits each template to the generated dates and outstanding notional.
Rates, spreads, payoff operations, and fixed-rate conventions are kept, unless
you override the day counter.

## Floating-coupon projection convention

- A past coupon set in advance uses the fixing stored on its accrual start
  date.
- A future coupon uses the forward rate from today's curve for its index, then
  applies the spread, floor, and cap.

The coupon amount stays an AD expression, so its forward-curve risk appears in
the normal sensitivity table.

The callable engines use the Hull–White short rate to value the issuer call.
Future floating coupons are projected from today's forwards and treated as
known amounts inside the call calculation. Their forward-curve risk is
included, but the engines do not model random future fixings.

## Context-level pricer and model selection

Choose the callable engine with a `PricerConfiguration` for
`InstrumentType::SteppedCouponBond`. Other product families use the same kind
of configuration. Each model assignment links a `ModelConfiguration` to a
`MarketIndex`.

A tree configuration for the stepped-coupon bond:

```json
{
  "instrument_type": "SteppedCouponBond",
  "pricer": {
    "TreeBased": {
      "time_grid": {
        "steps_per_year": 96,
        "max_steps": 4096,
        "day_counter": "Actual365"
      }
    }
  },
  "models": [{
    "market_index": "SOFR",
    "model": {
      "HullWhite": {
        "alpha": 0.05,
        "parameter_source": { "Fixed": { "sigma": 0.01 } }
      }
    }
  }]
}
```

Backward Fourier evolution with the same model assignment:

```json
{
  "instrument_type": "SteppedCouponBond",
  "pricer": {
    "BackwardFourierEvolution": {
      "time_grid": {
        "steps_per_year": 96,
        "max_steps": 4096,
        "day_counter": "Actual365"
      },
      "fourier_grid": {
        "points": 512,
        "standard_deviations": 10.0
      }
    }
  },
  "models": [{
    "market_index": "SOFR",
    "model": {
      "HullWhite": {
        "alpha": 0.05,
        "parameter_source": { "Fixed": { "sigma": 0.01 } }
      }
    }
  }]
}
```

The model is looked up using the discount index chosen by the policy.

## Hull-White dynamics and curve fit

Both callable engines use the one-factor Hull-White model

\\[
dx_t=-\alpha x_t\\,dt+\sigma\\,dW_t,\qquad r_t=x_t+\phi(t).
\\]

Over one step `dt`, the exact transition is

\\[
E[x_{t+\Delta}\mid x_t=x]=e^{-\alpha\Delta}x,\qquad
\operatorname{Var}[x_{t+\Delta}\mid x_t]
=\sigma^2\frac{1-e^{-2\alpha\Delta}}{2\alpha},
\\]

which becomes `sigma^2 dt` when `alpha` is zero.

- The shift `phi(t)` fits the initial discount curve.
- OAS is added to the short rate after the fit, so changing the OAS does not
  change the fitted drift.

Time grid:

- Payments and single call dates are moved to the nearest grid point.
- In both engines, a call window allows exercise at every grid time inside the
  window, not only at its ends.
- A window that has already started can be exercised from settlement until its
  end date.
- The strike applies to the principal still outstanding, after amortization,
  at the exercise step.
- Raise `steps_per_year` for finer time steps. `max_steps` caps the total
  number of steps.

### Recombining tree

`TreeBasedBondPricer` spaces nodes by `sqrt(3 Var[dx])`. At each node:

1. it picks as middle successor the node closest to the conditional mean;
2. it sets three probabilities that match the conditional mean and variance.

State prices are rolled forward. At each step the shift is chosen so that the
discounted state prices match the input curve's discount factor. The bond is
then valued by backward induction, applying the call at each step.

### Backward Fourier evolution

`BackwardFourierEvolutionBondPricer` uses an evenly spaced state grid. Let
\\(V_{i+1}(x)\\) be the value at the next step and \\(\eta\\) the Gaussian
shock. First compute the convolution

\\[
g_i(y)=\mathcal F^{-1}\\!\left(
\mathcal F(V_{i+1})(k)
e^{-\tfrac12\operatorname{Var}(\eta)k^2}\right)(y).
\\]

The conditional expectation is then
\\(E[V_{i+1}(e^{-\alpha\Delta}x+\eta)]=g_i(e^{-\alpha\Delta}x)\\).

Each step:

1. runs an FFT;
2. multiplies by the Gaussian characteristic function;
3. runs an inverse FFT;
4. interpolates at the mean-reverted state;
5. discounts by `exp(-(x + phi_i + OAS)dt)`;
6. applies the same call rule as the tree.

Grid settings:

- `fourier_grid.points` must be a power of two. More points reduce
  interpolation and spectral error.
- `standard_deviations` sets the half-width of the state grid. A larger value
  reduces wrap-around error but spreads the points wider.

The Fourier engine fits the curve using the variance of the integrated state.
With `I_n = dt sum_(i<n) x_i`, the Gaussian property gives

\\[
E[e^{-I_n}]=e^{\tfrac12\operatorname{Var}(I_n)},\qquad
\sum_{i<n}\phi_i\Delta=-\log P(0,t_n)
+\tfrac12\operatorname{Var}(I_n).
\\]

This fits the same initial curve as the tree, computed in a different way.

## OAS inversion and risk

With a clean or dirty price anchor, the pricer solves for the OAS with a
bracketed root search. It stops when the model dirty value equals the quoted
dirty value. The solved OAS is then an AD input for the final valuation.

`Request::Sensitivities` returns the risk to every discount and forward curve
pillar, plus one extra row:

```text
<identifier>.OAS    dPV/dOAS
```

This row treats the OAS as an independent market input. The curve and
volatility sensitivities hold the OAS fixed.

## Validation and examples

The tests cover invalid schedules, repetition of the last entry, amortization,
clean/dirty spread round trips, AD against central-difference spread risk,
extreme OAS, call boundaries, tree refinement, a Hull-White closed-form
zero-coupon bond option, and agreement between the tree and Fourier engines for
fixed and mixed capped/floored coupons.

Runnable examples:

- `cargo run -p valuation-bonds`
  prices the same mixed-coupon bond with both engines.
- `cargo run --manifest-path examples/scripting/Cargo.toml --bin stepped-coupon-bond`
  compares the tree engine with an equivalent deterministic script.

### Calibrated parameters

- Calibrated Hull–White parameters keep the whole piecewise sigma schedule.
- Transition variance is integrated piece by piece at each sigma change. The
  first sigma applies from time zero, and the last sigma applies beyond the
  final pillar.
- Calibration uses the curve's reference date and day counter, with the
  configured steps per year and maximum steps.
- Callable valuation requires settlement to equal the context evaluation date.

Volatility sensitivities pass through the calibration Jacobian to the original
surface or cube pillars. Limits:

- The schedule is built from separate constant-sigma fits, not a joint fit.
- The Jacobian (one-sided differences, one quote at a time) covers volatility
  inputs only. It does not include how sigma moves with curve quotes or with
  calibration strikes.
- Cube calibration must use at most one underlying tenor per expiry, within the
  range of the source cube.
