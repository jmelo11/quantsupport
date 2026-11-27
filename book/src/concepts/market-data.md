# Market data and historical markets

Curves and other market objects are built from three kinds of observed data,
each held in its own store:

- `QuoteStore` holds the prices or rates used for calibration on a reference
  date.
- `FixingStore` holds past index fixings.
- `FxStore` holds spot exchange rates.

A pricing context combines these stores with the configuration that says how to
build curves and other market objects.

## Quotes and identities

A `Quote` has two parts:

- `QuoteDetails` describes the quoted instrument.
- `QuoteLevels` holds optional bid, mid, and ask values.

Curve building uses one `Level`. If that level is missing, building fails; the
library does not fall back to another side.

```rust
# extern crate quantsupport;
use quantsupport::prelude::{Quote, QuoteDetails, QuoteLevels, QSError};

let details: QuoteDetails = "FixedRateDeposit_USD_SOFR_6M".parse()?;
let quote = Quote::new(details, QuoteLevels::with_mid(0.04));
assert_eq!(quote.levels().mid(), Some(0.04));
# Ok::<(), QSError>(())
```

The quote identifier defines the instrument used for calibration. For example,
`Swap_USD_TermSOFR3m_5Y_Semiannual_Quarterly` gives the product, currency,
projection index, tenor, and payment frequencies. Option identifiers also give
the expiry and strike conventions. For a swaption, the expiry and the tenor of
the underlying swap are separate fields.

Each rate index defines its swap conventions in
`RateIndexDetails::swap_conventions(tenor)`:

- fixed coupon day count and compounding;
- fixed and floating payment frequencies;
- floating accrual day count;
- settlement lag and business-day adjustment.

Swap quotes, swaption underlyings, and `MakeSwap` all use these conventions.
There is no shared default such as semiannual fixed against quarterly floating;
each index has its own.

Examples:

- Euribor: annual 30/360 fixed coupons; floating payments at the index tenor
  with ACT/360 accrual; TARGET calendar; two settlement days.
- TIBOR: semiannual ACT/365 fixed coupons.
- NIBOR: annual 30/360 fixed coupons.
- SONIA: annual ACT/365 coupons; same-day settlement.
- SOFR: annual ACT/360 coupons; two settlement days.
- AUD BBSW: quarterly fixed payments below four years, semiannual from four
  years.

Quote fields are positional. A regular swap identifier has the form
`Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq[_SettlementDays[_FixedDC[_FloatDC]]]]]`.
An empty field keeps the index default. For example,
`Swap_EUR_EURIBOR3m_1Y___1_Actual365` sets settlement to one business day and
the fixed day count to ACT/365. Add `_Actual360` to also set the floating day
count. Overrides apply only to the quote identifier that contains them.

Deposits, futures, bonds, zero rates, CDS, caps, caplets, OIS, basis and
cross-currency swaps, and swaptions also take day counts as positional fields.
Bonds have separate coupon and yield day counts. Two-leg contracts can have a
different accrual convention on each leg. See the `QuoteDetails` identifier
schemas for the field order of each product.

Option examples:

- `Swaption_USD_SOFR_1Y_5Y_ATM_Black` is a one-year option on a five-year swap.
- `CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black` is an option
  on a three-month index with a one-year expiry and a 4.5% absolute strike.

Black volatility is lognormal (relative). Normal volatility is in absolute rate
units per square-root year.

`MarketIndex` links contracts to the market objects built for them. Some
examples:

- `SOFR` is a standard rate index.
- `Collateral(CLP, USD)` discounts CLP payments collateralized in USD.
- `Credit("NAME")` identifies a credit survival curve.

Use the same [serialized forms](../reference/configuration.md) in
configurations, models, and contracts.

### Direct curve quotes

A curve can also be built directly from discount factors or zero rates listed
in `CurveConfiguration.quotes`:

- `DiscountFactor_USD_SOFR_1Y` is a discount factor.
- `ZeroRate_USD_SOFR_2027-02-24` is an annual decimal zero rate.

Pillars can be tenors or ISO dates. Zero rates default to continuous
compounding with annual frequency. Add a suffix such as
`_Compounded_Semiannual` to use another convention. The curve configuration
sets the day count, interpolation, and extrapolation.

Rules for direct quotes:

- Use one quote type per curve, with the same currency and rate convention.
- The quoted index must match the configured index.
- Direct quotes keep their identifiers, selected side, and scenario risk, like
  other quotes.
- A discount-factor curve adds a factor of one at the reference date and needs
  at least one future pillar.

## Fixings

Fixings supply index values that are already known on the valuation date.

- A missing fixing that is required is an error.
- A fixing on the valuation date is used if present. Otherwise, coupon
  calculations project it from the curve.
- The fixing date decides whether a fixing is needed, not the payment date or
  trade date.

Daily overnight coupons need a fixing for each business day, with weekend and
holiday accrual weights. A term coupon may use one reset for several payments.
Supply all required past fixings yourself.

If your data has gaps, fill them with your own checked data-preparation step.
`FixingStore::fill_missing_fixings` has limitations: it interpolates on a
reversed time axis and fills weekends and holidays too. Its
[reference](https://docs.rs/quantsupport/latest/quantsupport/quotes/fixingstore/struct.FixingStore.html)
gives the details.

## FX observations

A rate stored in `FxStore` from a base currency to a quote currency is the
number of quote-currency units per one base unit. Storing USD→CLP at 935 means
one USD buys 935 CLP.

To find a rate, the store tries, in order:

1. the direct rate;
2. the reciprocal of the opposite rate;
3. a path through other currencies (breadth-first search).

If no path connects the two currencies, the lookup returns an error.

Pricers ask the store for spot rates and decide which amounts to convert and
which currency to report in. FX requests always use the stored spot market,
even if the request has a date. To value against a past FX market, load that
market's snapshot, as described below.

## Coherent historical markets

A `MarketSnapshot` is the complete set of inputs needed to build a market at
one point in time. `MarketSnapshotStore` holds many snapshots.

A snapshot stores quote levels, FX rates, and configuration, including direct
zero rates. It does not store built curves or AD state. You cannot snapshot a
context that holds market objects you built yourself, because those objects
cannot be serialized as build instructions.

To pick a snapshot by timestamp:

- `Previous` picks the latest snapshot at or before the requested time.
- `Exact` requires a snapshot at exactly that time.
- `Closest` picks the nearest snapshot, preferring the earlier one on ties. It
  can pick a snapshot after the requested time.

Timestamps with an offset are converted to UTC. Two snapshots at the same
instant are rejected.

The snapshot keeps its own reference date. If you pick Friday's market for a
Monday request, the curves stay at Friday; they are not rolled to Monday.
Inputs from different snapshots are never mixed.

If you select the bid or ask side, every instrument quote, zero rate, and
discount factor is rebuilt at that side. Spot FX has a single value for all
sides.

Fixings are stored separately from snapshots, and they are not filtered by
publication date. For a historical valuation, supply the fixings that were
known at that time, including any revisions you want to apply.
