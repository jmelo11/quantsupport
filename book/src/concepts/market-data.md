# Market Data

Market data enters the library through three stores: `QuoteStore` (prices), `FixingStore` (historical index fixings) and `FxStore` (spot FX). This chapter documents the quote identifier grammar that ties quotes to instruments, and the API of each store.

## Quote identifiers

Every quote is identified by an underscore-separated string that is parsed by `QuoteDetails::from_str` into a `QuoteInstrument`. The identifier is what curve configurations, vol configurations and scenarios refer to, and it doubles as the pillar label in sensitivity reports.

| Instrument                      | Pattern                                                                                                 | Example                                                                                                            |
| ------------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Overnight deposit / cash        | `FixedRateDeposit_<CCY>_<Index>_<Tenor>`                                                                | `FixedRateDeposit_USD_SOFR_1D`                                                                                     |
| OIS / fixed–float swap          | `OIS_<CCY>_<Index>_<Tenor>[_<FixedFreq>_<FloatFreq>]`                                                   | `OIS_USD_SOFR_5Y`, `OIS_CLP_ICP_6M`                                                                                |
| Tenor basis swap                | `BasisSwap_<CCY>_<PayIndex>_<RecvIndex>_<Tenor>[_<PayFreq>_<RecvFreq>]`                                 | `BasisSwap_USD_SOFR_TermSOFR3m_2Y`                                                                                 |
| Fix–float cross-currency swap   | `FixFloatCrossCurrencySwap_<FixedCCY>_<FloatIndex>_<FloatCCY>_<Tenor>[..]`                              | `FixFloatCrossCurrencySwap_CLP_SOFR_USD_5Y`                                                                        |
| Float–float cross-currency swap | `FloatFloatCrossCurrencySwap_<DomCCY>_<DomIndex>_<ForIndex>_<ForCCY>_<Tenor>[..]`                       | `FloatFloatCrossCurrencySwap_USD_SOFR_ESTR_EUR_5Y`                                                                 |
| FX forward points               | `FxForwardPoints_<PAIR>_<Tenor>`                                                                        | `FxForwardPoints_USDCLP_3M`                                                                                        |
| FX outright forward             | `FxOutrightForward_<PAIR>_<Tenor>`                                                                      | `FxOutrightForward_EURUSD_1Y`                                                                                      |
| Rate future                     | `Future_<CCY>_<Index>_<IMM>`                                                                            | `Future_USD_SOFR_H26`                                                                                              |
| Convexity adjustment            | `ConvexityAdjustment_<CCY>_<Index>_<IMM>`                                                               |                                                                                                                    |
| Cap / floor volatility          | `CapFloor_<CCY>_<Index>_<Tenor>_<Freq>_<StrikeKind>_<Strike>_<VolType>`                                 | `CapFloor_USD_SOFR_5Y_Quarterly_Absolute_0.04_Black`                                                               |
| Caplet / floorlet volatility    | `CapletFloorlet_<CCY>_<Index>_<IndexTenor>_<Expiry>_<StrikeKind>_<Strike>_<Strategy>_<VolType>`         | `CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black`                                                      |
| Swaption volatility             | `Swaption_<CCY>_<Index>_<Expiry>_<SwapTenor>[_<FixedFreq>_<FloatFreq>]_<StrikeKind>_<Strike>_<VolType>` | `Swaption_CLP_ICP_1Y_1Y_Absolute_0.045_Black`, `Swaption_USD_SOFR_3M_2Y_Semiannual_Semiannual_Absolute_0.04_Black` |
| Equity option                   | `EquityCall_<CCY>_<Index>_<Tenor>_<Strike>`, `EquityPut_...`                                            | `EquityCall_USD_AAPL_6M_150`                                                                                       |
| FX option                       | `FxCall_<PAIR>_<Tenor>_<Strike>`, `FxPut_...`                                                           | `FxPut_USDCLP_3M_950`                                                                                              |
| Credit default swap             | `Cds_<Entity>_<CCY>_<Tenor>`                                                                            | `Cds_ACME_USD_5Y`                                                                                                  |

- `<Tenor>` and `<Expiry>` are `Period` strings (`1D`, `3M`, `5Y`, `1Y6M`).
- `<StrikeKind>` is `Absolute` (strike follows as a decimal), `Atm`, or `Relative` (offset from ATM).
- `<VolType>` is `Black` or `Normal`; `<Strategy>` for caplets is `Cap`, `Floor` or `Straddle`.
- Currency pairs are concatenated ISO codes (`USDCLP` = price of 1 USD in CLP).

`QuoteInstrument` has one variant per row (`Ois`, `FixedRateDeposit`, `BasisSwap`, `FixFloatCrossCurrencySwap`, `FloatFloatCrossCurrencySwap`, `FxForwardPoints`, `FxOutrightForward`, `Future`, `ConvexityAdjustment`, `CapFloor`, `CapletFloorlet`, `Swaption`, `EquityOption`, `FxOption`, `Cds`) carrying the parsed fields. Bootstrappers call `CurveConfiguration::instruments()` to turn these into instruments at the quoted levels.

## `QuoteStore`

```rust,ignore
let mut store = QuoteStore::new(Date::new(2026, 2, 24));
let details = QuoteDetails::from_str("OIS_USD_SOFR_5Y")?;
store.add_quote(Quote::new(details, QuoteLevels::with_mid(0.0407677739)));
store.add_quote(Quote::new(
    QuoteDetails::from_str("FxForwardPoints_USDCLP_3M")?,
    QuoteLevels::new(Some(5.30), Some(5.20), Some(5.40)),   // mid, bid, ask
));

store.reference_date();                        // Date
store.quote("OIS_USD_SOFR_5Y");                // Option<&Quote>
store.quotes();                                // &HashMap<String, Quote>
let mid = store.quote("OIS_USD_SOFR_5Y").and_then(|q| q.levels().mid());
```

`QuoteLevels::with_mid(mid)` sets only the mid; `QuoteLevels::new(mid, bid, ask)` takes three `Option<f64>`; `levels.value(Level::Mid | Bid | Ask)` returns `Result<f64>` and fails if that level was not supplied. Bootstrappers and builders take a `Level` argument, so one store can produce a mid curve and a bid/ask pair.

`QuoteStore` implements `QuoteSelector`, the trait bootstrappers read from. `PricingContext::quote_store()` returns the shocked copy when scenarios are attached and the base store otherwise (`base_quote_store()` always returns the original).

### JSON

The examples use this schema (`examples/bootstrap/data/quotes.json`):

```json
{
  "reference_date": "2026-02-24",
  "quotes": [
    { "identifier": "FixedRateDeposit_USD_SOFR_1D", "mid": 0.045 },
    { "identifier": "OIS_USD_SOFR_1Y", "mid": 0.0483664339 },
    { "identifier": "BasisSwap_USD_SOFR_TermSOFR3m_1Y", "mid": 0.00028 },
    { "identifier": "FxForwardPoints_USDCLP_3M", "mid": 5.3 },
    { "identifier": "FixFloatCrossCurrencySwap_CLP_SOFR_USD_5Y", "mid": 0.0512 }
  ]
}
```

The loader in `examples/bootstrap/src/main.rs` is ten lines:

```rust,ignore
#[derive(Deserialize)] struct QuoteRecord { identifier: String, mid: f64 }
#[derive(Deserialize)] struct JsonQuotes { reference_date: Date, quotes: Vec<QuoteRecord> }

let json: JsonQuotes = serde_json::from_reader(BufReader::new(File::open(path)?))?;
let mut store = QuoteStore::new(json.reference_date);
for rec in json.quotes {
    store.add_quote(Quote::new(QuoteDetails::from_str(&rec.identifier)?, QuoteLevels::with_mid(rec.mid)));
}
```

The Python binding `QuoteStore.from_json` reads the same file.

## `FixingStore`

```rust,ignore
let mut fixings = FixingStore::default();
fixings.add_fixing(&MarketIndex::SOFR, Date::new(2025, 5, 12), 0.0428);
fixings.fixing(&MarketIndex::SOFR, Date::new(2025, 5, 12))?;  // Result<f64>, NotFoundErr if missing
fixings.fixings(&MarketIndex::SOFR)?;                         // Result<&BTreeMap<Date, f64>>
fixings.fill_missing_fixings(Interpolator::Linear)?;          // fill every calendar day between first and last fixing
```

Fixings are needed for any floating coupon whose fixing date is on or before the valuation date. `DiscountedCashflowPricer` reads them through `MarketDataRequest`; the XVA `FixingPreprocessor` uses them to set `realized_fixing` / `partial_fixing` on claims (compounding daily fixings for in-arrears indices such as SOFR). JSON schema used by the examples:

```json
{
  "SOFR": [
    { "date": "2025-05-12", "rate": 0.0428 },
    { "date": "2025-05-13", "rate": 0.0429 }
  ]
}
```

## `FxStore`

```rust,ignore
let mut fx = FxStore::new();
fx.add_fx_rate(Currency::USD, Currency::CLP, DualFwd::new(935.0));   // 1 USD = 935 CLP
fx.get_fx_rate(Currency::CLP, Currency::USD)?;                       // 1/935, inverted automatically
fx.get_fx_rate(Currency::EUR, Currency::CLP)?;                       // triangulated via USD if EURUSD is stored

let fx = FxStore::from_records(vec![FxRateRecord { base: Currency::CLP, quote: Currency::USD, rate: 1.0 / 900.0 }]);
```

`get_fx_rate` returns `DualFwd::one()` for identical currencies, a direct lookup if the pair is stored, and otherwise breadth-first triangulation over stored pairs (multiplying along `base→quote` edges and dividing along reversed ones); it fails with `NotFoundErr` when the currencies are disconnected. `FxStore` implements `Pillars<DualFwd>` (labels `"USD/CLP"`), so `put_pillars_on_tape()` turns every stored rate into a tape leaf and FX-spot sensitivities appear next to curve pillars. `from_records` stores rates with `DualFwd::from`, i.e. off-tape until you call `put_pillars_on_tape()`. The bootstrapper uses the store to build `MarketIndex::Collateral(CLP, USD)` curves from cross-currency quotes; `FxForwardPricer` and `FxOptionPricer` read spot from it.

## Currencies and indices

`Currency` variants: `USD, EUR, JPY, ZAR, CLP, CLF, CHF, BRL, COP, MXN, AUD, CAD, CNY, GBP, NZD, NOK, SEK, PEN, CNH, INR, TWD, HKD, KRW, DKK, IDR`, with `as_str()`, `name()`, `symbol()`, `precision()`, `numeric_code()` and `Currency::try_from("USD")`.

`MarketIndex` variants: `SOFR, SOFRCompounded, TermSOFR1m, TermSOFR3m, TermSOFR6m, TermSOFR12m, ESTR, EURIBOR1m, EURIBOR3m, EURIBOR6m, EURIBOR12m, SONIA, TONAR, TIBOR3m, TIBOR6m, SARON, CORRA, AONIA, NZONIA, NOWA, SWESTR, ICP, VIX, Equity(String), FxPair(FxPair), Collateral(Currency, Currency), Credit(String), Other(String)`. Rate indices know their currency, tenor and day counter (`MarketIndex::SOFR.currency() == Currency::USD`); `Collateral(CLP, USD)` names the curve that discounts CLP cashflows collateralised in USD. In JSON, unit variants serialise as strings (`"SOFR"`) and tuple variants as objects (`{"Collateral": ["CLP", "USD"]}`, `{"Equity": "AAPL"}`).
