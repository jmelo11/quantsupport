# Resolving instrument terms when a market is available

The Rust workflow separates a reusable maker from its resolved contract:

```text
maker → unresolved trade → evaluation context → concrete trade → pricer
```

A maker can be defined before a reference date or market is available. Its
existing date setters accept `Date`; separate tenor setters accept `Period`.
The maker implements `Resolve`, which produces the existing concrete product
type. No alternative date type or implicit conversion is required.

## Define first, resolve when the market is available

This fragment uses the application's market and pricer.

```rust,ignore
use quantsupport::prelude::*;

let maker = MakeSwap::<DualFwd>::default()
    .with_identifier("spot-5y".into())
    .with_start_tenor(Period::from_str("0D")?)
    .with_tenor(Period::from_str("5Y")?)
    .with_notional(1_000_000.0)
    .with_fixed_rate(0.03)
    .with_currency(Currency::EUR)
    .with_market_index(MarketIndex::EURIBOR6m);

// The application supplies a PricingContext, MarketView, or another provider.
let resolution = ResolutionContext::new(&market);
let instrument = maker.resolve(&resolution)?;
let trade = SwapTrade::new(
    instrument, resolution.reference_date(), 1_000_000.0, Side::LongReceive,
);
let result = pricer.evaluate(&trade, &[Request::Value], &market)?;
```

This fragment retains a maker in a trade and resolves it on every evaluation:

```rust,ignore
let trade = UnresolvedTrade::from_maker(maker, |instrument, context| {
    Ok(SwapTrade::new(
        instrument, context.reference_date(), 1_000_000.0, Side::LongReceive,
    ))
});
// Register SwapTrade<DualFwd>'s ordinary pricer in Evaluator.
let result = evaluator.evaluate(&trade, &[Request::Value], &market)?;
// Or use a typed pricer directly:
let result = pricer.evaluate_unresolved(&trade, &[Request::Value], &market)?;
```

The pending trade stays unchanged. Each evaluation obtains a fresh contract from
that market. To retain booked dates and principal across markets, call
`trade.resolve(&ResolutionContext::new(&market))` once and evaluate that concrete
trade thereafter.

`ResolutionContext::new` uses the provider's evaluation date. Use
`with_reference_date(date)` to select an explicit resolution anchor. Resolution
does not change the maker, so the same definition can be resolved against
another reference date or market. Already resolved instruments retain their
dates, notionals and principal repayments.

`build()` constructs terms that are already fully specified. It rejects
unresolved relative inputs and directs the caller to `resolve`. A date setter
replaces the corresponding tenor, and a tenor setter replaces its date.

## Date anchors and conventions

| Terms | Tenor method | Anchor |
| --- | --- | --- |
| Scheduled product start | `with_start_tenor` | Resolution reference date |
| Scheduled product maturity | `with_tenor` / `with_maturity_tenor` | Resolved start |
| Bond or leg first coupon | `with_first_coupon_tenor` | Resolved start |
| Penultimate bond or OIS coupon | `with_next_to_last_coupon_tenor` | Resolved maturity; use a negative period |
| Equity/FX option or equity future expiry | `with_expiry_tenor` | Resolution reference date |
| Equity/FX forward delivery | `with_delivery_tenor` | Resolution reference date |
| NDF fixing | `as_ndf_tenor` | Resolution reference date |
| Swaption expiry | `with_expiry_tenor` | Resolution reference date |
| Swaption underlying start | `with_start_tenor` | Resolved expiry; defaults to expiry |
| Swaption underlying maturity | `with_swap_tenor` | Resolved underlying start |
| Rate future accrual end | `with_end_tenor` | Resolved accrual start |
| Caplet accrual start/end | `with_start_tenor` / `with_end_tenor` | Reference date / resolved start |
| Caplet fixing/payment | `with_fixing_tenor` / `with_payment_tenor` | Accrual start / accrual end |
| Script events | `with_event_tenors` | Resolution reference date |

Relative dates use the maker's calendar, business-day convention and month-end
rule. Absolute dates are retained; normal product schedule generation still
applies its own accrual and payment adjustments. Day tenors count business days
when a calendar is active. `0D` applies the selected adjustment to the anchor.
Without a calendar, day tenors count calendar days. `Period::new(-2,
TimeUnit::Days)` expresses a negative day offset.

`MakeOvernightIndexSwap::from_tenor(index, tenor, fixed_rate)` leaves its
reference date for resolution and retains the existing settlement-lag and
forward-start rules. An explicit `with_start_tenor` replaces those start rules,
just as an explicit start date does.

Relative nested schedules are available through bond call-date/window methods,
loan `with_prepayment_tenors`, and leg `with_disbursements_tenors` and
`with_redemptions_tenors`. Those tenors are measured from the resolved product
start. Their existing validation applies to the resulting dates; duplicate
principal dates are rejected.

## Floating loan amortization

This fragment resolves the initial floating rate from the application's market:

```rust,ignore
let maker = MakeLoan::<DualFwd>::default()
    .with_identifier("floating-french".into())
    .with_start_tenor(Period::from_str("0D")?)
    .with_tenor(Period::from_str("30Y")?)
    .with_notional(250_000.0)
    .with_currency(Currency::EUR)
    .with_coupons(vec![CashflowType::floating(
        MarketIndex::EURIBOR12m, 0.01, None, None,
    )]);

let loan = maker.resolve(&ResolutionContext::new(&market))?;
```

Resolution first generates the observation dates independently of principal.
It then obtains the first coupon's index observation, applies its spread and
payoff rules, and computes the initial French or balloon schedule. Historical
fixings are required when `anchor_fixings` is true (the default), including an
origination fixing whose coupon has already been paid. Today and future
observations project, even if today's fixing is stored. With
`PricingContext::with_anchor_fixings(false)`, all observations project. The
curve must support the requested observation interval. Resolution and the loan
pricer share these rules; `ResolutionContext::with_anchor_fixings` can override
the setting for an individual resolution.

`with_initial_amortization_rate` is an explicit override. Fixed-first loans use
the first fixed coupon, and bullet loans need no initial forecast. Existing
`build()` construction has no provider and therefore still needs an explicit
rate for floating-first amortizing loans.

Resolve the instrument once before calculating contractual price sensitivities
or pricing it under scenarios. Re-resolving the maker against another market
creates a new contract snapshot. Resolution does not automatically recast a
floating loan's principal schedule at subsequent resets.

## Product coverage

All native product makers implement `Resolve`: swaps, basis swaps, both
cross-currency swap types, OIS, swaptions, caps/floors, rate futures, bonds,
stepped bonds, floating-rate notes, deposits, loans, equity options/forwards/
futures, and FX options/forwards. `MakeCreditDefaultSwap`,
`MakeCapletFloorlet`, and `MakeScriptedProduct` provide the same workflow for
products previously constructed directly. `MakeLeg` also supports resolution.

## Python and Excel

Python products expose `from_tenors(**terms)` factories. Date arguments keep
their meaning; relative inputs have separate names:

```python
swap = qs.Swap.from_tenors(
    identifier="spot-5y", start_tenor="0D", tenor="5Y",
    notional=1_000_000, fixed_rate=0.03, currency="EUR", market_index="EURIBOR6m",
)
trade = qs.Trade(swap)  # execution defaults to a pending 0D offset
with context:
    result = context.evaluate(trade)
    booked = trade.resolve_trade(context)  # retain this contract across markets
```

Scheduled products use `start_tenor` and `tenor`; options use `expiry_tenor`,
forwards use `delivery_tenor`, and swaptions use `expiry_tenor` plus `tenor`.
Caplets additionally accept `fixing_tenor` and `payment_tenor`. Bonds support
`first_coupon_tenor`, stepped bonds support `next_to_last_coupon_tenor` and
`call_schedule_tenors`, loans support `prepayment_tenors`, and scripts support
`event_tenors`. Nested schedule tenors are measured from the resolved start;
script tenors are measured from the context date. `resolve_instrument(context)`
materializes an instrument without a position. Do not supply both an absolute
date/schedule and its tenor alternative.

A floating-first `Loan(...)` with absolute dates can also omit
`amortization_rate`; evaluation supplies the initial rate. Use an explicit rate
or `Trade.resolve_trade(context)` to retain its initial principal schedule.
Python contexts accept `anchor_fixings=True` and expose a writable property.

Excel `QS.INSTRUMENT.*` functions append separate tenor arguments. Leave the
corresponding date cell blank when supplying a tenor. `QS.TRADE.CREATE` accepts
`trade_tenor`; blank execution date and tenor default to `0D`. Both instrument
and trade handles retain pending terms until `QS.EVALUATE` receives a context.
`QS.CONTEXT.CREATE` appends `anchor_fixings`, defaulting to TRUE.
