# Building instruments explicitly

Every instrument maker implements `MakeInstrument`. This interface fragment
shows its two build methods and the method that resolves dates:

```rust,ignore
pub trait MakeInstrument: Sized {
    type Instrument;
    fn is_resolved(&self) -> bool;
    fn resolve_dates(&self, reference_date: Date) -> Result<Self>;
    fn build(&self) -> Result<Self::Instrument>;
    fn build_using(&self, context: &dyn MarketDataProvider)
        -> Result<Self::Instrument>;
}
```

`MakeInstrument` is in `quantsupport::prelude::*`. Import it to call these
methods.

- `build()` and `build_using()` return the same instrument type.
- Neither method changes the maker, so you can build from it again.

A trade holds a built instrument and an execution date. Pricers and portfolio
evaluators value the trade as built; they do not rebuild its terms.

`is_resolved()` tells you whether all dates and required market observations
are known. It does not validate the rest of the contract. If `is_resolved()`
is false, `build()` returns an error that names the missing input. Supply that
input on the maker, or call `build_using(&context)`.

By default, `build_using()` works in two steps:

1. It calls `resolve_dates()` with the context's evaluation date. This returns
   a new maker with absolute dates. It does not fetch market observations.
2. It calls `build()` on that new maker.

Each maker defines its own date anchors and conventions. The loan maker
overrides `build_using()` so that it can also fetch its initial amortization
rate when needed.

## Build from supplied terms

`build()` builds the instrument from the terms on the maker. It cannot resolve
tenors, including a maturity tenor combined with an explicit start date. Use
`build_using()` for those.

Errors from `build()`:

- Unresolved dates or market observations return
  `QSError::UnresolvedProductErr`.
- Missing required terms and invalid values return their usual error types.

```rust
# extern crate quantsupport;
use quantsupport::prelude::*;

# fn main() -> Result<()> {
let deposit = MakeFixedRateDeposit::<f64>::default()
    .with_identifier("deposit".into())
    .with_start_date(Date::new(2026, 2, 24))
    .with_maturity_date(Date::new(2027, 2, 24))
    .with_currency(Currency::EUR)
    .with_notional(100_000.0)
    .with_rate(0.03)
    .with_rate_definition(RateDefinition::new(DayCounter::Actual360, Compounding::Simple, Frequency::Annual))
    .build()?;
assert_eq!(deposit.maturity_date(), Date::new(2027, 2, 24));
# Ok(())
# }
```

## Build with a context

`build_using()` takes the evaluation date, fixing policy, and market
observations from a market data provider. The provider can be a
`PricingContext`, a `MarketView`, or any other `MarketDataProvider`.

This fragment uses an existing `market` and `pricer` to build and value a swap:

```rust,ignore
let maker = MakeSwap::<Recorded>::default()
    .with_identifier("spot-5y".into())
    .with_start_tenor(Period::from_str("0D")?)
    .with_tenor(Period::from_str("5Y")?)
    .with_notional(1_000_000.0)
    .with_fixed_rate(0.03)
    .with_currency(Currency::EUR)
    .with_market_index(MarketIndex::EURIBOR6m);

assert!(matches!(maker.build(), Err(QSError::UnresolvedProductErr(_))));
let instrument = maker.build_using(&market)?;
let trade = SwapTrade::new(
    instrument, market.evaluation_date(), 1_000_000.0, Side::LongReceive,
);
let result = pricer.evaluate(&trade, &[Request::Value], &market)?;
```

You can build another instrument from the same maker with a different context.
When you revalue an existing trade, its contract dates and principal stay the
same. Market changes affect only its projected cashflows, discounting, and
risk.

## Date anchors and conventions

Each tenor is measured from an anchor date:

| Terms | Tenor method | Anchor |
| --- | --- | --- |
| Scheduled start | `with_start_tenor` | Context evaluation date |
| Scheduled maturity | `with_tenor` / `with_maturity_tenor` | Constructed start |
| First coupon | `with_first_coupon_tenor` | Constructed start |
| Penultimate coupon | `with_next_to_last_coupon_tenor` | Maturity; usually a negative offset |
| Equity/FX option or equity future expiry | `with_expiry_tenor` | Context evaluation date |
| Forward delivery | `with_delivery_tenor` | Context evaluation date |
| NDF fixing | `as_ndf_tenor` | Context evaluation date |
| Swaption expiry | `with_expiry_tenor` | Context evaluation date |
| Swaption underlying start | `with_start_tenor` | Expiry |
| Swaption maturity | `with_swap_tenor` | Underlying start |
| Rate future accrual end | `with_end_tenor` | Accrual start |
| Loan CPR observations | `with_prepayment_tenors` | Loan start |
| Script events | `with_event_tenors` | Explicit script reference date or context date |

Rules:

- A date setter and its tenor setter replace each other. The last one you call
  is used.
- Absolute dates are used exactly as given.
- Tenors use the maker's calendar and business-day convention.
- With a calendar, day tenors count business days. Longer tenors add calendar
  time and then adjust the date.
- Negative offsets are allowed.

## Floating loan amortization

French and balloon loans that start with a floating coupon need an initial
all-in rate to compute their principal schedule. Either:

- supply it with `with_initial_amortization_rate`, or
- call `build_using()`, which reads the first index observation and applies
  the spread and coupon payoff.

Loans that start with a fixed coupon use that coupon. Bullet loans do not need
an amortization rate.

When fixing anchoring is enabled:

- observations strictly before the evaluation date need stored fixings;
- observations on or after the evaluation date are projected from curves.

When anchoring is disabled, every observation is projected, as long as the
curve covers its period. Missing fixings or curves cause the build to fail.

The built loan keeps its initial amortization rate and principal schedule for
all later valuations.
