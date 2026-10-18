# Observations and historical markets

Market construction starts from quotes, historical fixings and spot FX
rates. QuantSupport stores these observations separately because they are used
at different stages of valuation. `QuoteStore` supplies the prices or rates
used for calibration at a reference date. `FixingStore` supplies realized index
observations, and `FxStore` supplies spot exchange rates. A pricing context
combines these stores with the instructions needed to construct curves and
other market objects.

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

Quote identifiers specify the calibration contract.
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

Daily overnight accrual needs business-day observations with weekend and holiday
accrual weights. Term coupons may require one reset for several payments. Supply
the required historical observations explicitly. Handle gaps through a validated
data-preparation policy. The current `FixingStore::fill_missing_fixings` uses
reversed interpolation coordinates and applies no business-day filter; its
[reference](https://docs.rs/quantsupport/latest/quantsupport/quotes/fixingstore/struct.FixingStore.html)
documents that restriction.

## FX observations

`FxStore` interprets a stored rate from base to quote as quote-currency units
per base unit: storing USD→CLP at 935 means one USD buys 935 CLP. Lookup uses a
direct rate, a reciprocal rate, or breadth-first triangulation through connected
currencies. A disconnected pair is an error.

Pricers request spot rates from this graph and determine which amounts to
convert and which currency to report. Ordinary FX requests use the stored
spot market even when the request carries a date. To select a historical FX
market, load the corresponding snapshot as described below.

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
[Excel](../excel/historical-pricing.md) workflows use the same native archive
and selection rules.
