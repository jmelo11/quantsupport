# Caps and Floors

## Instruments

```rust,ignore
let cap = MakeCapFloor::default()
    .with_identifier("USD_SOFR_CAP_2Y".to_string())
    .with_start_date(rd)
    .with_maturity_date(rd + Period::from_str("2Y")?)
    .with_notional(10_000_000.0)
    .with_strike(0.045)
    .with_cap_floor_type(CapFloorType::Cap)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_frequency(Frequency::Quarterly)       // default
    .build()?;
let trade = CapFloorTrade::new(cap, rd, 10_000_000.0, Side::LongReceive);
```

Required: notional, start_date, maturity_date, strike, currency, market_index, identifier, cap_floor_type. Defaults: side `LongReceive`, frequency `Quarterly`. `CapFloorType::{Cap, Floor}`; a single period is a `CapletFloorlet` (`CapletFloorletType::{Caplet, Floorlet}`) with trade `CapletFloorletTrade::new(..)`.

## Black-76 pricers

`ClosedFormBlackCapletPricer::new()` and `ClosedFormBlackCapPricer::new()` handle `Request::Value` and `Request::Sensitivities`. For each caplet with fixing \\(T\\), accrual \\([T,S]\\), \\(\tau=S-T\\):

\\[
F = \frac{1}{\tau}\left(\frac{P(T)}{P(S)}-1\right),\qquad
\text{Caplet} = N\\,\tau\\,P*d(S)\\,[F\\,\Phi(d_1) - K\\,\Phi(d_2)],\quad
d*{1,2}=\frac{\ln(F/K)\pm\tfrac12\sigma^2 T}{\sigma\sqrt T}.
\\]

- The forward comes from the curve of `market_index`; the discount factor from the discount policy (dual-curve when a `SingleCurveCSADiscountPolicy` is set).
- The strike is a `Strike` (`Absolute`, `Atm`, `Relative`) resolved against \\(F\\).
- \\(\sigma\\) is read from the `VolatilitySurfaceElement` for `market_index` at `(fixing_date, strike)` through `volatility_from_date`; `VolatilityType::Normal` surfaces switch to the Bachelier formula.
- A cap is the sum of its caplets; floors use the put formula.

Market data requested: the discount curve(s) and the volatility surface of the index. Sensitivities are labelled with the OIS quotes and the `CapletFloorlet_*` quotes that define the surface.

## Hull-White pricers

`ClosedFormHullWhiteCapletPricer::new(alpha, sigma)` and `ClosedFormHullWhiteCapPricer::new(alpha, sigma)` price the same trades without a surface, using the one-factor Hull-White model with constant \\(\sigma\\):

\\[
\text{Caplet} = N\\,(1+\tau K)\\;\text{BondPut}\bigl(T, S, X\bigr),\qquad X=\frac{1}{1+\tau K},
\\]

where the zero-coupon bond option uses the volatility

\\[
\sigma_P = \sigma\\,B(T,S)\sqrt{\frac{1-e^{-2\alpha T}}{2\alpha}},\qquad B(t,T)=\frac{1-e^{-\alpha(T-t)}}{\alpha}.
\\]

They are useful to cross-check a calibrated model (`HullWhite::calibrate_with_configuration`, see [Hull-White](../simulation/hull-white.md)) against the Black surface it was fitted to.

## Sensitivities

Both pricer families run the reverse sweep from the option value; `results.sensitivities()` therefore contains curve pillars (`OIS_USD_SOFR_*`) and, for Black pricers, one row per volatility quote (`CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black`), i.e. a vega ladder on the quoted grid.
