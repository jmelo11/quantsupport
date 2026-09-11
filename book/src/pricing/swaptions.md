# Swaptions

## Instrument

```rust,ignore
let swaption = MakeSwaption::<DualFwd>::default()
    .with_identifier("USD_SOFR_1Y5Y_PAYER".to_string())
    .with_expiry(rd + Period::from_str("1Y")?)
    .with_swap_tenor_date(rd + Period::from_str("6Y")?)      // underlying swap maturity
    .with_strike(0.04)
    .with_notional(10_000_000.0)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_swaption_type(SwaptionType::Payer)                 // default
    .build()?;
let trade = EuropeanSwaptionTrade::new(swaption, rd, 10_000_000.0, Side::LongReceive);
```

Required: strike, expiry, identifier, market_index, currency, swap_tenor_date, notional. `SwaptionType::{Payer, Receiver}`. The underlying swap's fixed-leg coupons `(payment_time, accrual_fraction)` are derived from the swaption's frequency settings.

## `ClosedFormHullWhiteSwaptionPricer`

```rust,ignore
let pricer = ClosedFormHullWhiteSwaptionPricer::new(alpha, sigma);
let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
```

Handles `Request::Value` and `Request::Sensitivities`; requests the discount curve of `market_index` (and the policy's discount index if different). The price is Jamshidian's decomposition:

1. Zero-coupon bond prices in Hull-White are affine, \\(P(t,T\mid r_t)=A(t,T)\\,e^{-B(t,T)r_t}\\), with \\(B(t,T)=\frac{1-e^{-\alpha(T-t)}}{\alpha}\\) and \\(A\\) fitted to the initial curve.
2. Find the critical short rate \\(r^{\ast}\\) such that the underlying swap's fixed leg (coupons \\(c_i\\) plus final notional) is worth par at expiry: \\(\sum_i c_i P(T_0,T_i\mid r^{\ast}) = 1\\). The solve is a bisection with up to 200 iterations.
3. Strikes \\(X_i = P(T_0,T_i\mid r^{\ast})\\) turn the swaption into a portfolio of zero-coupon bond options: a payer swaption is \\(\sum_i c_i\\,\text{BondPut}(T_0,T_i,X_i)\\), a receiver the corresponding calls, each priced with the bond volatility \\(\sigma\\,B(T_0,T_i)\sqrt{(1-e^{-2\alpha T_0})/(2\alpha)}\\).

The implicit solve is handled inside the AD framework, so `Request::Sensitivities` returns exact derivatives with respect to the curve quotes without bumping.

## Volatility cubes

Market swaption volatilities live in a `VolatilityCubeConfiguration` built from `Swaption_CCY_Index_Expiry_Tenor_[PayFreq_RecvFreq]_Strike_val_VolType` quotes (see [Volatility Surfaces](../curves/volatility.md)). The cube is used to calibrate LGM/Hull-White sigma schedules for simulation (`VolatilitySourceConfiguration::Calibrated` with `CalibrationSource::Cube`, as in `examples/cva/data/xva_config.json` for `ICP`); pair it with the Hull-White swaption pricer to verify that the calibrated model reprices the calibration instruments.
