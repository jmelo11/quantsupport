# Observations and historical markets

A market contains several kinds of observations with different temporal
meaning. `QuoteStore` holds current calibration prices or rates at a reference
date. `FixingStore` holds dated realized index observations. `FxStore` holds
spot currency relationships. None of these stores alone is a calibrated market.

## Quotes and identities

A `Quote` combines `QuoteDetails` with `QuoteLevels`. Details describe the
quoted instrument, while levels hold optional bid, mid and ask values.
Construction chooses one `Level`; a missing consumed level is an error, not a
request to substitute another side.

```rust
# extern crate quantsupport;
use quantsupport::prelude::{Quote, QuoteDetails, QuoteLevels, QSError};

let details: QuoteDetails = "FixedRateDeposit_USD_SOFR_6M".parse()?;
let quote = Quote::new(details, QuoteLevels::with_mid(0.04));
assert_eq!(quote.levels().mid(), Some(0.04));
# Ok::<(), QSError>(())
```

An identifier encodes a calibration contract rather than merely a label.
`Swap_USD_TermSOFR3m_5Y_Semiannual_Quarterly` identifies product, currency,
projection index, tenor and payment frequencies. An option identifier also
specifies expiry and strike conventions; for a swaption, expiry and underlying
swap tenor are separate coordinates.

`Swaption_USD_SOFR_1Y_5Y_ATM_Black` quotes a one-year-expiry option on a
five-year swap. `CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black`
quotes an option with a three-month index tenor, one-year expiry and 4.5%
absolute strike. Black volatility is relative lognormal volatility; Normal
volatility is in absolute rate units per square-root year.

`MarketIndex` connects these contracts to constructed objects. A standard rate
index such as `SOFR` is distinct from `Collateral(CLP, USD)`, which discounts
CLP payments under USD collateral, or `Credit("NAME")`, which identifies credit
survival. Their [serialized forms](../reference/configuration.md) must agree
across configurations, models and contracts.

## Fixings

Fixings resolve observations already known at valuation. Missing required past
observations are errors. Same-day observations can be used when present and
forecast otherwise in the relevant coupon workflows. The fixing date, not
merely the payment date or trade date, determines whether history is required.

Daily overnight accrual needs business-day observations with weekend and
holiday accrual weights. Term coupons may require one reset for several
payments. Supply the required historical observations explicitly.
`FixingStore::fill_missing_fixings` currently evaluates days remaining to the
last observation against knots measured from the first. This reverses its
interpolation coordinate for inserted calendar dates; it is not reliable
chronological gap filling and applies no business-day filter. The workflows
here do not use it to manufacture missing history.

## FX observations

`FxStore` interprets a stored rate from base to quote as quote-currency units
per base unit: storing USD→CLP at 935 means one USD buys 935 CLP. Lookup uses a
direct rate, a reciprocal rate, or breadth-first triangulation through connected
currencies. A disconnected pair is an error.

This graph resolves spot relationships. It does not imply a universal
reporting-currency conversion service or a dated FX history. The context's
ordinary FX request handling resolves currency-pair spot inputs; its request
date is not a historical-selection instruction. Individual pricers determine
which amounts are converted and into which currency.

## Coherent historical markets

`MarketSnapshot` records one observation instant together with a complete set
of native construction inputs. `MarketSnapshotStore` holds such observations.
Snapshots preserve quote levels, FX and configuration—including explicit zero
rates—but not constructed curves or an AD tape. Contexts supplied with runtime
market objects cannot be captured as if those objects were serializable
construction instructions.

Timestamp selection and valuation date answer different questions.
`Previous` selects the latest instant at or before the request; `Exact`
requires the instant itself; `Closest` chooses the nearest observation, preferring
the earlier on ties, and can select future data. Timestamp values normalize
offset-aware input to UTC. Duplicate equivalent instants are rejected.

The selected snapshot retains its own market reference date. Selecting Friday's
market for a Monday request does not roll Friday's curves to Monday. Inputs
from several snapshots are not blended. A selected bid or ask market rebuilds
the consumed quoted inputs at that level; explicit zero-rate pillars and spot
FX are side-independent.

Fixing histories are supplied separately and do not carry publication-vintage
filtering. Historical valuation therefore requires the application to supply
the appropriate historical observations, including any revision policy.
The [Python](../python/getting-started.md) and
[Excel](../excel/historical-pricing.md) workflows use this same native archive,
not separate historical-pricing engines.
