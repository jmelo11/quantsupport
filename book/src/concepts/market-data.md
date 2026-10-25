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

Rate indices own their swap defaults through
`RateIndexDetails::swap_conventions(tenor)`: fixed coupon day count and
compounding, both payment frequencies, floating accrual day count, settlement
lag and business-day adjustment. Swap quotes and swaption underlyings consume
these defaults. Direct `MakeSwap` builders use the same metadata. There is no
shared semiannual/quarterly fallback.

For example, Euribor uses annual 30/360 fixed coupons, floating payments at the
index tenor with ACT/360 accrual, TARGET and two settlement days. TIBOR uses
semiannual ACT/365 fixed coupons; NIBOR uses annual 30/360 fixed coupons.
SONIA uses annual ACT/365 coupons and same-day settlement, while SOFR uses
annual ACT/360 coupons and two settlement days. AUD BBSW fixed payments are
quarterly below four years and semiannual from four years.

All quote parameters are positional. A regular swap accepts
`Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq[_SettlementDays[_FixedDC[_FloatDC]]]]]`.
Empty fields retain the index defaults. For example,
`Swap_EUR_EURIBOR3m_1Y___1_Actual365` changes settlement to one business day
and fixed accrual to ACT/365. Appending `_Actual360` explicitly sets the
floating day count too. Overrides apply independently, including when supplied
through Python quote records or Excel quote identifiers.

Day counts are also positional for deposits, futures, bonds, zero rates, CDS,
caps, caplets, OIS, basis and cross-currency swaps, and swaptions. Bond coupon
and yield day counts are separate; two-leg contracts allow separate accrual
conventions. See the `QuoteDetails` identifier schemas for each field order.

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

Curves may also consume direct observations through
`CurveConfiguration.quotes`: `DiscountFactor_USD_SOFR_1Y` supplies a discount
factor, and `ZeroRate_USD_SOFR_2027-02-24` supplies an annual decimal zero rate.
Pillars accept tenors or ISO dates. Zero rates default to Continuous compounding
and Annual frequency; append `_Compounded_Semiannual`, for example, to change
the rate convention. The configuration supplies day count, interpolation and
extrapolation. Use one direct quote family with consistent currency and rate
conventions per curve, with the quoted index matching the configured index.
These inputs retain their quote identifiers, selected sides and scenario risk.
Discount-factor curves automatically include the fixed reference-date factor
of one and require at least one future pillar.

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
Snapshots preserve quote levels, FX and configuration—including direct zero
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
all consumed instrument, zero-rate and discount-factor quotes at that level;
spot FX is side-independent.

Fixing histories are supplied separately and do not carry publication-vintage
filtering. Historical valuation therefore requires the application to supply
the appropriate historical observations, including any revision policy.
The [Python](../python/getting-started.md) and
[Excel](../excel/historical-pricing.md) workflows use the same native archive
and selection rules.
