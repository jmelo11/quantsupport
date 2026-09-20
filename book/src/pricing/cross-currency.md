# Cross-Currency Swaps

Cross-currency swaps exchange interest and principal cashflows denominated in two currencies. Their valuation brings together projection curves, collateral-adjusted discount curves, FX conversion, and basis risk. This chapter explains the two supported swap forms, the discounting flow under a collateral agreement, their sensitivities, and the related FX-forward contract.

QuantSupport provides fixed-versus-floating and floating-versus-floating cross-currency swaps. Both include initial and final notional exchanges and use `DiscountedCashflowPricer`.

## Builders

The builder records each leg's currency, notional, index, and spread. Choosing notionals that are equivalent at the inception spot makes the initial exchange economically balanced. The following example builds a five-year USD/CLP floating-rate swap:

```rust,ignore
let xccy = MakeFloatFloatCrossCurrencySwap::<f64>::default()
    .with_identifier("CLPUSD_XCCY_5Y".to_string())
    .with_start_date(rd)
    .with_maturity_date(rd.advance(5, TimeUnit::Years))
    .with_domestic_notional(10_000_000.0)                 // USD
    .with_foreign_notional(10_000_000.0 * fx_clpusd)      // CLP
    .with_foreign_spread(0.002)
    .with_domestic_currency(Currency::USD)
    .with_foreign_currency(Currency::CLP)
    .with_domestic_market_index(MarketIndex::SOFR)
    .with_foreign_market_index(MarketIndex::ICP)
    .build()?;
let trade = FloatFloatCrossCurrencySwapTrade::new(xccy, rd, 10_000_000.0, Side::LongReceive);
```

| Builder                              | Required                                                                                                                                                     | Defaults                                                                                |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| `MakeFixFloatCrossCurrencySwap<T>`   | start_date, maturity_date, domestic_notional, foreign_notional, fixed_rate, identifier, domestic_currency, foreign_currency, floating_market_index           | spread 0, side LongReceive, and configurable domestic and foreign frequencies           |
| `MakeFloatFloatCrossCurrencySwap<T>` | start_date, maturity_date, domestic_notional, foreign_notional, identifier, domestic_currency, foreign_currency, domestic_market_index, foreign_market_index | domestic/foreign spread 0, side LongReceive                                             |

The builder table separates required economic terms from convenience defaults. Both generated instruments are wrapped in the corresponding trade type, either `FixFloatCrossCurrencySwapTrade` or `FloatFloatCrossCurrencySwapTrade`, before pricing.

## Discounting and FX

Each leg is first valued in its own currency. `FxStore` converts that value into the reporting currency, and the discount policy selects a curve consistent with the collateral agreement. `get_fx_rate` can triangulate through available intermediate currencies when the direct pair is absent.

Under a USD CSA, the policy is installed as follows:

```rust,ignore
pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD)));
```

- USD leg → discounted on `SOFR`.
- CLP leg → discounted on `MarketIndex::Collateral(Currency::CLP, Currency::USD)`, the CLP curve implied by USD collateral. That curve must be bootstrapped from cross-currency basis quotes:

The collateralized CLP curve configuration can use the swap itself as a calibration instrument. This JSON example selects one-, two-, and five-year USD/CLP basis quotes:

```json
{
  "market_index": { "Collateral": ["CLP", "USD"] },
  "quotes": [
    "FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_1Y",
    "FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_2Y",
    "FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_5Y"
  ]
}
```

`MultiCurveBootstrapper` receives the FX store through `with_fx_store(fx)`, which lets the calibration instruments align their notionals at inception spot. The `bootstrap` and `sensitivity` examples demonstrate this construction for USD/CLP. The `cva` example carries the resulting trade and curves into an XVA calculation.

## Sensitivities

With `DualFwd`, the sensitivity table reflects every market path used by the valuation. `OIS_USD_SOFR_*` rows describe USD discounting and projection. `OIS_CLP_ICP_*` rows describe projection of the CLP coupons. `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_*` rows describe the collateralized CLP curve. Registering spot as a `DualFwd::new` leaf through `add_fx_rate` also exposes the FX delta.

## FX forwards

An FX forward exchanges currencies at a future date and provides a simpler view of the same discount-factor relationship. `MakeFxForward` requires an identifier, delivery date, currency pair, and a strike expressed as an outright rate or forward points. Contracts are deliverable by default. `as_ndf(fixing_date, settlement_ccy)` creates a cash-settled non-deliverable forward, and the day counter defaults to Actual/360.

`FxForwardTrade::new` adds the position metadata. `FxForwardPricer` supports value, fair rate, and sensitivities through

\\[
F = S\\,\frac{P_{quote}(T)}{P_{base}(T)},\qquad \text{NPV} = N\\,(F-K)\\,P_{quote}(T).
\\]

The fair forward \\(F\\) follows from spot and the relative discount factors of the two currencies. `Request::FairRate` returns this value, and the NPV compares it with the contractual rate \\(K\\).

## What to remember

Cross-currency valuation keeps four economic roles explicit: each index projects its own coupons, the collateral agreement selects discount curves, spot FX converts currencies, and cross-currency instruments calibrate the collateral adjustment. The resulting sensitivity report follows those same links back to domestic, foreign, basis, and FX market inputs.
