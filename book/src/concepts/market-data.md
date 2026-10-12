# Market Data

Market data enters the library through three stores: `QuoteStore` (prices), `FixingStore` (historical index fixings) and `FxStore` (spot FX). This chapter documents the quote identifier grammar that ties quotes to instruments, and the API of each store.

## Quote identifiers

`QuoteDetails` describes the instrument being quoted: its product family, currencies,
indices, term and any coupon, strike or payment conventions.

`QuoteDetails` keeps those contract terms together. The observed bid, mid and
ask values are stored in `QuoteLevels`; `Quote` combines the details and values.
For example, `FixedRateDeposit_USD_SOFR_6M` describes a six-month USD deposit
associated with SOFR. A mid value of `0.04` is its quoted rate of 4%.

```rust
use quantsupport::prelude::{Quote, QuoteDetails, QuoteLevels};

let details: QuoteDetails = "FixedRateDeposit_USD_SOFR_6M".parse()?;
let quote = Quote::new(details, QuoteLevels::with_mid(0.04));
# Ok::<(), quantsupport::prelude::QSError>(())
```

### Reading an identifier

Identifiers use underscores to separate contract terms. The first word selects
the product, which determines the meaning of the remaining fields. For example,
`Swap_USD_TermSOFR3m_5Y_Semiannual_Quarterly` describes a five-year USD swap
against three-month Term SOFR, with semiannual fixed payments and quarterly
floating payments.

`CCY` is a currency code such as `USD`; `Index` is a market index such as
`SOFR`, `ICP` or `TermSOFR3m`. `Tenor` and `Expiry` are periods such as `1D`,
`3M`, `5Y` or `1Y6M`. Frequency names include `Annual`, `Semiannual`, `Quarterly`
and `Monthly`. Square brackets mark optional parts of an identifier.

### Parsed fields

`QuoteDetails` exposes the terms through getters. Optional fields return `None`
when absent or inapplicable to the product. An omitted payment frequency,
for example, stays unset in the details; the instrument builder supplies its
default when constructing the contract.

| Getter | Meaning |
| --- | --- |
| `identifier()` | Original market label used to find the quote and identify its sensitivity. |
| `instrument()` | Product family, such as `FixedRateDeposit` or `EuropeanSwaption`. |
| `currency()`, `market_index()` | Main currency and index for a rate, bond, credit or equity quote. |
| `secondary_market_index()` | Receive index of a basis swap, or foreign index of a floating cross-currency swap. |
| `pay_currency()`, `receive_currency()` | Leg currencies of a cross-currency swap, or base and quote currencies of an FX pair. |
| `tenor()` | Instrument term; for a swaption, the underlying swap's term. |
| `option_expiry()` | Time to option expiry. |
| `index_tenor()` | Underlying accrual period of a caplet/floorlet. |
| `contract_code()` | IMM contract code of a future or convexity-adjustment quote. |
| `pay_leg_frequency()`, `receive_leg_frequency()` | Payment frequencies supplied in the identifier. |
| `coupon_rate()`, `strike()`, `vol_type()` | Bond coupon, option strike and volatility quoting convention, where applicable. |
| `bond_calibration_strategy()` | Whether the bond quote supplies a yield or a price. |
| `fx_forward_calibration_strategy()` | Whether the FX forward quote supplies forward points or an outright rate. |
| `option_strategy()` | Caplet/floorlet quote strategy, such as `Straddle`. |

Tenors are relative periods. Builders use the market reference date and the
instrument's conventions to resolve actual start, expiry and payment dates.
For `Swaption_USD_SOFR_1Y_5Y_Absolute_0.04_Black`, `option_expiry()` is one year,
`tenor()` is five years, and `strike()` is 4%. The associated `QuoteLevels`
supplies the observed implied volatility.

### Rates, bonds and credit

| Identifier | What it describes |
| --- | --- |
| `FixedRateDeposit_CCY_Index_Tenor` | A deposit's currency, curve index and term. |
| `OIS_CCY_Index_Tenor[_OisFields]` | A fixed rate against an overnight index; optional conventions are described under [OIS parameters](#ois-parameters). |
| `Swap_CCY_Index_Tenor[_FixedFreq[_FloatFreq]]` | A fixed rate against a floating index, with optional fixed and floating payment frequencies. |
| `FixedRateBond_CCY_Index_Tenor[_PayFreq]_CouponRate[_BondStrategy]` | A bond's currency, curve index, term, coupon and optional payment frequency and calibration strategy. |
| `Future_CCY_Index_IMM` | An interest-rate futures contract identified by its currency, index and IMM code. |
| `ConvexityAdjustment_CCY_Index_IMM` | An adjustment quote associated with the specified futures contract. |
| `Cds_Entity_CCY_Tenor` | A CDS par spread for a credit entity, currency and term. |

Deposit and swap quote values are rates, while CDS quote values are par spreads.
Rates, coupons and spreads use decimals: `0.04` means 4%.

In `FixedRateBond_USD_CORP_5Y_Semiannual_0.04_AnchorPrice`, the `0.04` in the
identifier is the coupon. `AnchorPrice` tells calibration to use the observed
quote as the bond price; `AnchorYield` uses it as yield to maturity and is the
default if the strategy is omitted. `IMM` is a futures contract code such as `H6`.

### Basis and cross-currency swaps

| Identifier | What it describes |
| --- | --- |
| `BasisSwap_CCY_PayIndex_RecvIndex_Tenor[_PayFreq[_RecvFreq]]` | Two floating legs in one currency; the primary index is the pay index and the secondary index is the receive index. |
| `FixFloatCrossCurrencySwap_DomCCY_FloatIndex_ForCCY_Tenor[_DomFreq[_ForFreq]]` | A fixed domestic-currency leg against a floating foreign-currency leg. |
| `FloatFloatCrossCurrencySwap_DomCCY_DomIndex_ForIndex_ForCCY_Tenor[_DomFreq[_ForFreq]]` | A domestic floating leg against a foreign floating leg, each with its own index. |

`DomCCY` and `ForCCY` identify the domestic and foreign legs; `DomFreq` and
`ForFreq` set their payment frequencies. For example,
`FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_5Y` describes five years of USD
SOFR cashflows against CLP ICP cashflows.

### FX and equity

| Identifier | What it describes |
| --- | --- |
| `FxForward_PAIR_Tenor_ForwardStrategy` | A forward's currency pair, delivery term and choice of forward-points or outright-price quote. |
| `EquityCall_CCY_Ticker_Expiry_StrikeKind_StrikeValue` | A call on the specified equity, with its currency, expiry and strike. |
| `EquityPut_CCY_Ticker_Expiry_StrikeKind_StrikeValue` | A put on the specified equity, with its currency, expiry and strike. |
| `FxCall_PAIR_Expiry_StrikeKind_StrikeValue` | A currency call with the specified pair, expiry and strike. |
| `FxPut_PAIR_Expiry_StrikeKind_StrikeValue` | A currency put with the specified pair, expiry and strike. |

`PAIR` contains the base currency followed by the quote currency: `EURUSD`
means USD per EUR. For an FX forward, `AnchorForwardPoints` uses forward points
as the observed quote and `AnchorOutrightPrice` uses the outright forward rate.
For example, `FxForward_EURUSD_1M_AnchorOutrightPrice` identifies a one-month
EUR/USD forward whose quote is the outright rate.

Equity and FX option identifiers accept `Absolute` and `Relative` strike kinds.
`EquityCall_USD_AAPL_6M_Absolute_150` identifies a six-month USD call on AAPL
with a strike of 150. A relative strike is an offset from the forward.

### Interest-rate options

| Identifier | What it describes |
| --- | --- |
| `CapFloor_CCY_Index_Tenor[_Freq]_Strike_VolType` | A cap/floor volatility quote with the full instrument term and optional coupon frequency. |
| `CapletFloorlet_CCY_Index_IndexTenor_Expiry_Strike_Strategy_VolType` | A caplet/floorlet volatility quote; index tenor is the underlying accrual period and expiry is the time until it starts. |
| `Swaption_CCY_Index_Expiry_SwapTenor[_FixedFreq[_FloatFreq]]_Strike_VolType` | A swaption volatility quote; expiry is the exercise term and swap tenor is the length of the underlying swap. |

`Strike` expands to `Absolute_0.04`, `Relative_0.01` or `ATM`. `Absolute` specifies
the strike itself; `Relative` adds an offset to the forward; `ATM` uses the
forward and needs no numeric strike. For example,
`Swaption_USD_SOFR_1Y_5Y_ATM_Black` quotes the volatility of an at-the-money
option expiring in one year on a five-year SOFR swap.

`VolType` is `Black` for lognormal volatility or `Normal` for absolute-rate
volatility. Volatilities use decimals: a Black value of
`0.20` means 20%. The caplet/floorlet `Strategy` field accepts `Straddle`,
`Strangle`, `RiskReversal` or `Butterfly`, as defined by `OptionStrategy`.

### OIS parameters

OIS quotes can specify settlement, payment schedules, calendars and overnight
observation rules after the tenor. `OIS_USD_SOFR_1Y` uses the index preset.
To override a later field while keeping earlier defaults, leave the earlier
fields empty: `OIS_USD_SOFR_1Y___0_3` keeps both default frequencies, starts
with zero settlement days and pays three business days after accrual ends.
Omit any trailing fields you do not need.

| Order | Field | Accepted values / examples | Default |
| --- | --- | --- | --- |
| 1 | Fixed frequency | `Frequency`, e.g. `Annual`, `Semiannual` | `Annual` |
| 2 | Overnight frequency | `Frequency`, e.g. `Annual`, `Quarterly` | `Annual` |
| 3 | Settlement days | Nonnegative business-day count | SONIA: `0`; CORRA: `1`; other indices: `2` |
| 4 | Payment lag | Nonnegative business-day count | `SOFR` / `SOFRCompounded`: `2`; other indices: `0` |
| 5 | Fixed day counter | `DayCounter`, e.g. `Actual360`, `Actual365` | Index day counter |
| 6 | Settlement calendar | Canonical calendar name | Index calendar |
| 7 | Fixed accrual calendar | Canonical calendar name | Index calendar |
| 8 | Overnight accrual calendar | Canonical calendar name | Index calendar |
| 9 | Payment calendar | Canonical calendar name | Index calendar |
| 10 | Accrual adjustment | Accrual business-day convention, e.g. `Following` | `ModifiedFollowing` |
| 11 | Termination adjustment | Termination business-day convention | Accrual convention |
| 12 | Payment adjustment | Payment business-day convention | `Following` |
| 13 | Schedule generation | e.g. `Backward`, `Forward` | `Backward` |
| 14 | End of month | `true` or `false` | Inferred from the effective date |
| 15 | Forward start | `Period`, e.g. `1M` | `0D` (start at spot) |
| 16 | First coupon date | ISO date, e.g. `2027-01-05` | No explicit front stub |
| 17 | Penultimate coupon date | ISO date, e.g. `2030-01-05` | No explicit back stub |
| 18 | Overnight averaging | `Compound` or `Simple` | `Compound` |
| 19 | Lookback days | Nonnegative business-day observation count | `0` |
| 20 | Lockout days | Nonnegative business-day observation count | `0` |
| 21 | Observation shift | `true` or `false` | `false` |
| 22 | Daily spread compounding | `true` or `false` | `false` |
| 23 | Calibration pillar | `LastRelevantDate`, `MaturityDate`, `CustomDate(YYYY-MM-DD)` | `LastRelevantDate` |

Calendar values use names such as `UnitedStates(Sofr)` or `TARGET`.
`OIS_USD_SOFR_2Y_Annual_Annual_2_2_Actual360` specifies annual payments,
two settlement days, a two-business-day payment lag and ACT/360 fixed coupons.
`CustomDate(2027-01-06)` selects an explicit calibration pillar; that date must
lie between the contract's effective and last relevant dates.

### Constructing details

Parse a standard identifier with `str::parse()` or `QuoteDetails::from_str()`.
`QuoteDetails::parse(identifier, separator)` accepts a different separator.
For a structured data feed, `QuoteDetails::new` and the `with_*` setters let
you supply the terms directly.

See [OIS conventions](../curves/bootstrapping.md#ois-conventions) for settlement
and coupon calculations, and [Volatility Surfaces](../curves/volatility.md) for
how option quote terms define a surface or cube.

## `QuoteStore`

`QuoteStore` combines the parsed identifiers with bid, mid, and ask values at a
single reference date. The following example adds one curve quote and one FX
forward quote, then retrieves the mid value used by a builder.

```rust,ignore
let mut store = QuoteStore::new(Date::new(2026, 2, 24));
let details = QuoteDetails::from_str("OIS_USD_SOFR_5Y")?;
store.add_quote(Quote::new(details, QuoteLevels::with_mid(0.0407677739)));
store.add_quote(Quote::new(
    QuoteDetails::from_str("FxForward_USDCLP_3M_AnchorForwardPoints")?,
    QuoteLevels::new(Some(5.30), Some(5.20), Some(5.40)),   // mid, bid, ask
));

store.reference_date();                        // Date
store.quote("OIS_USD_SOFR_5Y");                // Option<&Quote>
store.quotes();                                // &HashMap<String, Quote>
let mid = store.quote("OIS_USD_SOFR_5Y").and_then(|q| q.levels().mid());
```

Each store lookup returns a `Quote`: `details()` gives its contract terms and
`levels()` gives its observed market values.

`QuoteLevels::with_mid(mid)` sets the mid. `QuoteLevels::new(mid, bid, ask)` takes three `Option<f64>` values. `levels.value(Level::Mid | Bid | Ask)` returns `Result<f64>` and reports an absent level. Bootstrappers and builders take a `Level` argument, so one store can produce a mid curve and bid or ask constructions.

`QuoteStore` implements `QuoteSelector`, the trait bootstrappers read from. `PricingContext::quote_store()` returns the shocked copy when scenarios are attached and the base store otherwise (`base_quote_store()` always returns the original).

### JSON

Applications commonly receive observations from files or services. The
examples use the following small envelope in
`examples/bootstrap/data/quotes.json`, where the reference date applies to
every record.

```json
{
  "reference_date": "2026-02-24",
  "quotes": [
    { "identifier": "FixedRateDeposit_USD_SOFR_1D", "mid": 0.045 },
    { "identifier": "OIS_USD_SOFR_1Y", "mid": 0.0483664339 },
    { "identifier": "BasisSwap_USD_SOFR_TermSOFR3m_1Y", "mid": 0.00028 },
    { "identifier": "FxForward_USDCLP_3M_AnchorForwardPoints", "mid": 5.3 },
    { "identifier": "FixFloatCrossCurrencySwap_CLP_SOFR_USD_5Y", "mid": 0.0512 }
  ]
}
```

The document contains observations only. Curve membership and construction
choices belong to their own configuration files, which allows the same quote
store to support several market builds.

The loader in `examples/bootstrap/src/main.rs` deserializes the envelope,
parses every identifier, and creates a store. Its compact form is shown here so
the boundary between application I/O and library types is explicit.

```rust,ignore
#[derive(Deserialize)] struct QuoteRecord { identifier: String, mid: f64 }
#[derive(Deserialize)] struct JsonQuotes { reference_date: Date, quotes: Vec<QuoteRecord> }

let json: JsonQuotes = serde_json::from_reader(BufReader::new(File::open(path)?))?;
let mut store = QuoteStore::new(json.reference_date);
for rec in json.quotes {
    store.add_quote(Quote::new(QuoteDetails::from_str(&rec.identifier)?, QuoteLevels::with_mid(rec.mid)));
}
```

The result is the same `QuoteStore` used by programmatic construction. The
Python binding `QuoteStore.from_json` reads the same file, so both languages
share the identifier grammar and validation rules.

## `FixingStore`

Historical fixings settle coupon observations whose fixing dates have already
occurred. A fixing store is indexed by market index and date, which makes a
missing observation an explicit valuation error. Future observation dates use
the applicable curve forecast.

```rust,ignore
let mut fixings = FixingStore::default();
fixings.add_fixing(&MarketIndex::SOFR, Date::new(2025, 5, 12), 0.0428);
fixings.fixing(&MarketIndex::SOFR, Date::new(2025, 5, 12))?;  // Result<f64>, NotFoundErr if missing
fixings.fixings(&MarketIndex::SOFR)?;                         // Result<&BTreeMap<Date, f64>>
fixings.fill_missing_fixings(Interpolator::Linear)?;          // fill every calendar day between first and last fixing
```

Interpolation is an application choice for genuinely missing daily history.
The original endpoints remain observations. Generated dates fill the
interior required by compounded overnight coupons.

Fixings are needed for any floating coupon whose fixing date is on or before the valuation date. `DiscountedCashflowPricer` reads them through `MarketDataRequest`. The XVA `FixingPreprocessor` uses them to set `realized_fixing` or `partial_fixing` on claims, including compounding daily observations for in-arrears indices such as SOFR. The examples use this JSON schema:

```json
{
  "SOFR": [
    { "date": "2025-05-12", "rate": 0.0428 },
    { "date": "2025-05-13", "rate": 0.0429 }
  ]
}
```

Keeping fixings in a dedicated file makes their historical role visible and
prevents them from being confused with current calibration quotes.

## `FxStore`

Spot FX forms a connected graph of currencies. `FxStore` accepts whichever
direct pairs are available and resolves reciprocal or triangulated rates when
a pricer asks for another orientation.

```rust,ignore
let mut fx = FxStore::new();
fx.add_fx_rate(Currency::USD, Currency::CLP, DualFwd::new(935.0));   // 1 USD = 935 CLP
fx.get_fx_rate(Currency::CLP, Currency::USD)?;                       // 1/935, inverted automatically
fx.get_fx_rate(Currency::EUR, Currency::CLP)?;                       // triangulated via USD if EURUSD is stored

let fx = FxStore::from_records(vec![FxRateRecord { base: Currency::CLP, quote: Currency::USD, rate: 1.0 / 900.0 }]);
```

The example demonstrates all three lookup paths: direct storage, reciprocal
orientation, and triangulation through an intermediate currency.

`get_fx_rate` returns `DualFwd::one()` for identical currencies and uses a direct lookup for a stored pair. For other connected currencies, it performs breadth-first triangulation, multiplying along `base→quote` edges and dividing along reversed edges. Disconnected currencies produce `NotFoundErr`. `FxStore` implements `Pillars<DualFwd>` with labels such as `"USD/CLP"`, so `put_pillars_on_tape()` registers every stored rate as a tape leaf. `from_records` creates off-tape values with `DualFwd::from`, ready for later registration. The bootstrapper uses the store to build `MarketIndex::Collateral(CLP, USD)` curves from cross-currency quotes. `FxForwardPricer` and `FxOptionPricer` read spot from it.

## Currencies and indices

Currencies and market indices provide typed keys for the stores described
above. Currency metadata controls display and rounding. Market indices connect
contract references to curves, volatility markets, fixings, and simulations.

`Currency` variants: `USD, EUR, JPY, ZAR, CLP, CLF, CHF, BRL, COP, MXN, AUD, CAD, CNY, GBP, NZD, NOK, SEK, PEN, CNH, INR, TWD, HKD, KRW, DKK, IDR`, with `as_str()`, `name()`, `symbol()`, `precision()`, `numeric_code()` and `Currency::try_from("USD")`.

`MarketIndex` variants include standard rates such as SOFR, ESTR, EURIBOR, SONIA, and ICP, along with `VIX`, `Equity(String)`, `FxPair(FxPair)`, `Collateral(Currency, Currency)`, `Credit(String)`, and `Other(String)`. Rate indices know their currency, tenor, and day counter. `Collateral(CLP, USD)` names the curve that discounts CLP cashflows collateralized in USD. In JSON, unit variants serialize as strings such as `"SOFR"`, and structured variants serialize as tagged objects such as `{"Collateral": ["CLP", "USD"]}` and `{"Equity": "AAPL"}`.

## From observations to a market

These stores deliberately stop at observed data. Quote identifiers describe
calibration instruments, fixings preserve realized history, and FX rates
connect currencies. The pricing context combines them with configuration in
the next construction step, where interpolation, bootstrapping, and model
calibration begin.
