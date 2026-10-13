# Bond Pricing

QuantSupport keeps straight fixed-rate debt separate from callable
stepped-coupon debt. This distinction prevents call-model terms from leaking
into the ordinary `FixedRateBond` contract.

| Instrument | Pricer | Supported analytics |
| --- | --- | --- |
| `FixedRateBond` | `BondPricer` | value, cashflows, yield to maturity, modified duration, Z-spread, sensitivities |
| `SteppedCouponBond` | `TreeBasedBondPricer` or `BackwardFourierEvolutionBondPricer` | value, cashflows, OAS, sensitivities |

The stepped-coupon bond normally trades in price, so it deliberately has no
yield-rate convention. Requests for yield to maturity or modified duration on
that instrument return an error.

## Pricing anchors

Every bond trade stores exactly one required `BondPricingAnchor`. Pricers
are reusable and take no quote in their constructors:

- `CleanPrice(value)` is a price per bond units excluding accrued interest.
- `DirtyPrice(value)` is a price per bond units including accrued interest.
- `ZSpread(value)` is a continuously compounded spread for a straight bond.
- `OAS(value)` is an option-adjusted spread for a bond with issuer calls.

Clean and dirty prices are anchor variants; there is no separate price-kind
enum. For a straight bond, a price anchor implies Z-spread. For a callable
stepped-coupon bond, it implies OAS. OAS on a straight bond and Z-spread on a
callable bond are rejected.

The solved or supplied spread becomes an independent AD variable. The common
sensitivity table therefore reports curve risk together with either
`dPV/dZSpread` or `dPV/dOAS`.

## Straight fixed-rate bonds

Only coupons and principal strictly after settlement enter valuation. A clean
quote is converted to dirty value by adding accrued interest. Z-spread `z`
solves

```text
dirty value = sum_i CF_i P(0,t_i) / P(0,S)
                       exp(-z year_fraction(S,t_i)).
```

The solver expands a bracket and bisects inside it. Yield to maturity is
solved independently under the fixed bond's day-count, frequency, and
compounding convention. Modified duration is `-(dP/dy)/P`, using the analytic
derivative of each yield discount factor.

`Request::Value` remains signed position PV at the evaluation date. A quoted
price anchors the spread; it does not redefine PV.

## Stepped-coupon contract

The builder delegates date generation to `MakeSchedule`. In addition to
`payment_frequency`, it accepts a calendar, business-day convention,
termination-date convention, date-generation rule, end-of-month flag,
first-coupon date and next-to-last-coupon date. The final date defaults to the
same adjustment convention as intermediate dates. Python exposes the same
schedule controls.

Side belongs to `SteppedCouponBondTrade` or `FixedRateBondTrade`, together with
the pricing anchor and optional settlement/spread day-count terms. Instrument
builders do not accept side. Cashflows describe the contract; the pricer applies
the trade direction to PV and sensitivities, and claim decomposition applies
that direction to XVA claims.

`SteppedCouponBond` is built from three independent schedules:

1. `coupons` is positional. Entry zero applies to the first generated coupon
   period, entry one to the second, and so on. If the payment schedule is
   longer, the last coupon definition repeats through maturity. A list longer
   than the generated schedule is rejected rather than silently truncated.
2. `notional_percentages` is also positional and uses the same carry-forward
   rule. Each value is the fraction of original principal outstanding during
   that period. The first value must be `1.0`; values must remain in `(0, 1]`
   and cannot increase. Extra entries beyond the generated schedule are
   rejected.
3. `call_schedule` is not positional. Each entry supplies `start_date`, an
  optional `end_date`, and `strike`. Without an end date it is a discrete
  call. Otherwise it is an inclusive exercise window. Windows are sorted,
  must follow the bond start and end before final payment, and cannot overlap,
  including at a shared endpoint. The legacy serialized `date` field remains
  accepted as an alias for `start_date`.

A coupon schedule contains the existing `CashflowType<T>` variants:

```text
FixedRateCoupon(FixedRateCoupon<T>)
FloatingRateCoupon(FloatingRateCoupon<T>)
OptionEmbeddedCoupon(OptionEmbeddedCoupon<T>)
```

For a floating fixing `L`, the all-in coupon rate is

```text
R = min(max(L + spread, floor), cap),
```

with a missing floor interpreted as negative infinity and a missing cap as
positive infinity. Fixed coupons use the bond's coupon day counter. Floating
coupons use the referenced index convention.

An outstanding-notional reduction creates a principal redemption at the end
of that coupon period. The remaining principal is redeemed at maturity.

Calls are issuer calls only; there is no exercise-type enum and no holder-put
branch. A strike is quoted per bond units and excludes a coupon paid on the
same date. During backward valuation the engine first discounts continuation,
then applies the issuer obstacle `min(V, K)`, then adds contractual cashflows
mapped to that date. Consequently, same-day coupon and scheduled amortization
are paid before the remaining bond is called.

```rust,ignore
let bond = MakeSteppedCouponBond::<DualFwd>::default()
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

  The stepped bond and its builder have no discount index. Discounting is a
  pricing choice, resolved exclusively by the pricer's `DiscountPolicy`.
  Straight-bond pricing also requires a policy; an embedded instrument index
  does not bypass it. A policy can explicitly choose whether to prefer such
  legacy indices. `CashflowType::fixed` and `CashflowType::floating` are
  conveniences for creating templates, not another coupon type hierarchy.
  The builder rebinds existing coupon cashflows to the generated dates and
  outstanding notional, preserving rates, spreads, payoff operations, and
  fixed-rate conventions unless a coupon day counter is explicitly overridden.

The numerical implementations live in separate modules:
`treebasedbondpricer` and `backwardfourierevolutionbondpricer`.
Common settlement, quote conversion, spread solving and risk reporting live in
`bondanalytics`; contract-to-grid mapping lives in `bondevents`. Each engine
prepares its own numerical controls and implements its valuation function.
Hull–White parameter resolution, calibration state and transition variance live
under `models/hullwhite`. `BondPricingAnchor` is exported from
`instruments/fixedincome/bondpricinganchor` and the prelude.

## Floating-coupon projection convention

Historical in-advance coupons use the fixing stored on their accrual-start
date. Future coupons use the current forward curve for their reference index,
then apply spread, floor, and cap. The resulting amount remains an AD
expression, so its forward-curve sensitivity appears in the ordinary risk
table.

The current callable engines model the issuer option through the Hull-White
short-rate state but do not make future coupon fixings state-dependent. In
other words, future floating coupons are deterministic forward projections at
the valuation date. This convention is explicit so a later stochastic-index
extension can be introduced without changing the product contract.

## Context-level pricer and model selection

`PricingContext` owns `PricerConfiguration` entries keyed by
`InstrumentType`. This is the common selection mechanism for every product
family, not a bond-specific model store. Models are typed
`ModelConfiguration` values assigned to a `MarketIndex`; neither the model nor
the pricer is selected by an arbitrary string at runtime.

A tree configuration for the stepped-coupon bond is:

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

Backward Fourier evolution uses the same model assignment:

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

The configured model is resolved using the policy-selected discount index. Automatic
Python dispatch reads the same configuration and instantiates the selected
engine.

## Hull-White dynamics and curve fit

Both callable engines use the one-factor Hull-White state

```text
dx_t = -alpha x_t dt + sigma dW_t,
r_t = x_t + phi(t).
```

For one interval `dt`, the exact transition is

```text
E[x_(t+dt) | x_t=x] = exp(-alpha dt) x,
Var[x_(t+dt) | x_t] = sigma^2 (1-exp(-2 alpha dt)) / (2 alpha),
```

with limit `sigma^2 dt` when `alpha` is zero. The deterministic shift
`phi(t)` fits the initial discount curve. OAS is added to the short rate after
that fit, so changing OAS does not recalibrate drift.

Contractual payments and discrete call dates are mapped to their nearest time
grid points. Call windows permit exercise at every numerical time inside the
inclusive interval in both engines, not only at endpoints. A window that
already started remains exercisable at settlement until its end date.
Strikes use principal remaining after amortization mapped to the exercise
step. Increasing `steps_per_year` improves time resolution; `max_steps` caps
total work.

### Recombining tree

`TreeBasedBondPricer` places nodes at spacing
`sqrt(3 Var[dx])`. At each node it selects the nearest middle successor to the
conditional mean and assigns three probabilities that match the first two
conditional moments. State prices propagate forward, and the deterministic
shift at each layer is chosen so their discounted sum matches the input curve
discount factor. The bond value is then obtained by backward induction with
the call obstacle.

### Backward Fourier evolution

`BackwardFourierEvolutionBondPricer` uses a uniform state grid. If
`V_(i+1)(x)` is the next-step value, its conditional Gaussian expectation is
a convolution. In Fourier space,

```text
F[E[V_(i+1)(x exp(-alpha dt) + eta)]](k)
    = F[V_(i+1)](k) exp(-0.5 Var[eta] k^2).
```

The implementation performs an FFT, multiplies by the Gaussian characteristic
function, and performs an inverse FFT. It interpolates at the mean-reverted
state, discounts by `exp(-(x + phi_i + OAS)dt)`, and applies the same call
obstacle as the tree.

`fourier_grid.points` must be a power of two. More points reduce interpolation
and spectral error. `standard_deviations` controls the truncated state-domain
half-width; increasing it reduces periodic wrap-around at the cost of a wider
grid.

The Fourier curve fit uses Gaussian integrated-state variance. With
`I_n = dt sum_(i<n) x_i`, Gaussianity gives

```text
E[exp(-I_n)] = exp(0.5 Var[I_n]),
sum_(i<n) phi_i dt = -log(P(0,t_n)) + 0.5 Var[I_n].
```

This supplies an independent numerical construction of the same initial-curve
fit used by the tree.

## OAS inversion and risk

For a clean or dirty price anchor, the selected callable engine is evaluated
inside a bracketed scalar root solve until model dirty value equals quoted
dirty value. The converged OAS is then placed on the AD tape for final
valuation. `Request::Sensitivities` contains all discount/forward-curve pillar
adjoints and one additional row:

```text
<identifier>.OAS    dPV/dOAS
```

This is the partial derivative with OAS treated as an independent market
factor. It intentionally does not re-solve OAS under a curve bump.

## Validation and examples

Tests cover invalid schedules, carry-forward behavior, amortization,
clean/dirty spread round trips, AD-versus-central-difference spread risk,
extreme OAS, call boundaries, tree refinement, a Hull-White closed-form
zero-coupon bond-option benchmark, and agreement between the tree and Fourier
engines for fixed and mixed capped/floored coupons.

Runnable examples are:

- `cargo run -p valuation-bonds`
  prices the same mixed-coupon instrument with both engines.
- `cargo run --manifest-path examples/scripting/Cargo.toml --bin stepped-coupon-bond`
  compares the native tree with an equivalent scripted deterministic payoff.
- `bindings/python/examples/valuation/bonds/stepped_coupon_bond.ipynb` constructs and
  prices the product from Python with typed terms and auxiliary model settings.
- `bindings/python/examples/scripting/scripted_cashflows.ipynb` explains the
  scripted underlying and its deterministic native comparison.

Calibrated Hull–White parameters retain the entire piecewise sigma schedule.
Transition variance integration splits at every sigma boundary, using the
first sigma from time zero and extrapolating the last beyond its final pillar.
Calibration uses the curve's reference date and day counter while preserving
the configured step density and maximum steps. Callable valuation currently
requires settlement to equal the context evaluation date.

Volatility sensitivities chain the calibration Jacobian through sampled
surface or cube quotes to their original risk pillars. The current calibrator
supplies the volatility Jacobian only: sigma's response to curve quotes and
changes in calibration strikes is not included. Cube calibration must select
at most one underlying tenor per expiry and stay within the source domain.
