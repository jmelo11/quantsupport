# Building instruments explicitly

Instrument makers implement `MakeInstrument`. This interface fragment shows
its two construction methods and date-resolution hook:

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

Both methods return the same concrete product type and leave the maker unchanged.
Import `MakeInstrument` to call these trait methods; it is included in
`quantsupport::prelude::*`.
A trade contains a built instrument and an explicit execution date. Pricers and
portfolio evaluators value those contracts without rebuilding their terms.

`is_resolved()` checks whether date inputs and required market observations are
already resolved. It does not validate the contract. `build()` uses
this check and, when it returns false, reports the missing dependency and directs
the caller to supply it explicitly or call `build_using(&context)`.

The trait's default `build_using()` calls `resolve_dates()` with the context's
evaluation date, then calls `build()` on the returned maker. Each maker defines
its date anchors and conventions. `resolve_dates()` leaves the original maker
unchanged and does not fetch market observations. The loan maker overrides
`build_using()` to obtain its initial amortization rate when needed.

## Build from supplied terms

`build()` constructs directly from the maker's concrete terms. Date tenors,
including a maturity tenor paired with an explicit start date, are resolved by
`build_using()`. Unresolved date inputs or market observations return
`QSError::UnresolvedProductErr`. Missing mandatory terms and invalid values keep
their normal error categories.

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

`build_using()` supplies an evaluation date, fixing policy, and market
observations explicitly. The provider can be a `PricingContext`, `MarketView`,
or another `MarketDataProvider`.

This fragment uses an existing `market` and `pricer` to build and value a swap:

```rust,ignore
let maker = MakeSwap::<DualFwd>::default()
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

The maker can build another product using another context. Revaluing the existing
trade preserves its contractual dates and principal. Market changes affect its
projected receipts, discounting, and risk.

## Date anchors and conventions

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

Date and tenor setters replace the alternative input. Supplied absolute dates
are preserved. Relative inputs use the maker's calendar and business-day
conventions. With a calendar, day tenors count business days; longer tenors use
calendar arithmetic followed by adjustment. Negative offsets are supported.

## Floating loan amortization

Floating-first French and balloon loans need an initial all-in rate to establish
contractual principal. Supply `with_initial_amortization_rate`, or call
`build_using()` to obtain the first index observation and apply the spread
and coupon payoff. Fixed-first loans can use their fixed coupon; bullet loans do
not need an amortization forecast.

With fixing anchoring enabled, observations strictly before the evaluation date
require stored fixings. Today and future observations project from curves.
Disabling anchoring projects all observations, provided the curve supports their
interval. Missing fixings or curves fail during construction. The built loan
retains its initial amortization rate and principal schedule across valuations.

## Python

Python exposes named makers with the same methods. A maker is separate from its
product and cannot be placed into `Trade`.

```python
maker = qs.MakeSwap(
    identifier="spot-5y", start_tenor="0D", tenor="5Y",
    notional=1_000_000, fixed_rate=0.03, currency="USD", market_index="SOFR",
)
# maker.build() raises qs.UnresolvedProductErr: no reference date was supplied.
with context:
    instrument = maker.build_using(context)
    trade = qs.Trade(instrument, context.evaluation_date)
    result = context.evaluate(trade)
```

`MakeLoan` follows the same approach for its initial amortization observation.
`MakeTrade` can construct an explicit execution date from `trade_tenor` using
`build_using(context)`. Existing product constructors remain convenient
for fully specified contracts. No instrument or trade is resolved during pricing.

## Excel

`QS.INSTRUMENT.*` and `QS.TRADE.CREATE` take a construction context immediately
after the object name. They publish handles only after construction succeeds.
For example:

```text
=QS.INSTRUMENT.FX.FORWARD("fx",B2,"","EUR","USD",1.10,"1Y")
=QS.TRADE.CREATE("position",B2,D2,"","Receive",1000000)
```

Here B2 contains a context handle. Relative dates and initial loan amortization
are calculated during construction. Background construction may show `#N/A`
while waiting for committed market inputs; it never publishes an unresolved
product. Context revisions invalidate dependent constructor calculations.
