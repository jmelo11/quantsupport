# Cross-Currency Swaps

Two instruments cover cross-currency swaps, both with initial and final notional exchange and priced by `DiscountedCashflowPricer`.

## Builders

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
| `MakeFixFloatCrossCurrencySwap<T>`   | start_date, maturity_date, domestic_notional, foreign_notional, fixed_rate, identifier, domestic_currency, foreign_currency, floating_market_index           | spread 0, side LongReceive; `with_domestic_leg_frequency`, `with_foreign_leg_frequency` |
| `MakeFloatFloatCrossCurrencySwap<T>` | start_date, maturity_date, domestic_notional, foreign_notional, identifier, domestic_currency, foreign_currency, domestic_market_index, foreign_market_index | domestic/foreign spread 0, side LongReceive                                             |

Trades: `FixFloatCrossCurrencySwapTrade<T>::new(..)`, `FloatFloatCrossCurrencySwapTrade<T>::new(..)`.

## Discounting and FX

Each leg is priced in its own currency, converted to the reporting currency with the `FxStore` (`get_fx_rate` triangulates through intermediate currencies with a BFS when the direct pair is absent) and discounted on the curve chosen by the discount policy. Under a USD CSA:

```rust,ignore
pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD)));
```

- USD leg → discounted on `SOFR`.
- CLP leg → discounted on `MarketIndex::Collateral(Currency::CLP, Currency::USD)`, the CLP curve implied by USD collateral. That curve must be bootstrapped from cross-currency basis quotes:

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

`MultiCurveBootstrapper` needs `with_fx_store(fx)` for such specs so the notionals are FX-consistent at inception. `examples/bootstrap` and `examples/sensitivity` do this for USD/CLP; `examples/cva` runs the same trade through XVA.

## Sensitivities

With `DualFwd` the sensitivity table for the swap above contains rows for `OIS_USD_SOFR_*` (discounting), `OIS_CLP_ICP_*` (projection of the CLP leg) and `FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_*` (collateral curve). Sensitivity to the FX spot is exposed if the spot is registered as a `DualFwd::new` leaf in the `FxStore` (`add_fx_rate(base, quote, DualFwd)`).

## FX forwards

`MakeFxForward` (`with_identifier`, `with_delivery_date`, `with_base_currency`, `with_quote_currency`, and either `with_forward_price`/`with_forward_rate` or `with_forward_points`; `as_deliverable()` default or `as_ndf(fixing_date, settlement_ccy)`; `with_day_counter` default Actual360) produces an `FxForward`, wrapped by `FxForwardTrade::new`. `FxForwardPricer::new()` supports Value, FairRate and Sensitivities with

\\[
F = S\\,\frac{P_{quote}(T)}{P_{base}(T)},\qquad \text{NPV} = N\\,(F-K)\\,P_{quote}(T).
\\]

`Request::FairRate` returns \\(F\\).
