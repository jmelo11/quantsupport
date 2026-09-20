# Caps and Floors

Caps and floors protect a borrower or lender against adverse movements in a floating rate. Each contract is a sequence of options on individual reset periods, which makes the product a useful bridge between volatility surfaces and short-rate models. This chapter explains how to construct these instruments, how the Black and Hull-White pricers interpret them, and how their market sensitivities differ.

## Instruments

A cap pays when the observed floating rate exceeds its strike. A floor pays when the rate falls below the strike. `MakeCapFloor` constructs the reset schedule and creates one optionlet for every accrual period. The following example builds a two-year quarterly SOFR cap:

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

The required fields define the economic contract: notional, dates, strike, currency, market index, identifier, and cap-or-floor direction. The trade side defaults to `LongReceive` and payment frequency defaults to quarterly. A single reset period is represented by `CapletFloorlet`, whose type is either `Caplet` or `Floorlet`.

## Black-76 pricers

The Black pricers value each optionlet from the projected forward rate and the market implied volatility. `ClosedFormBlackCapletPricer` handles one period. `ClosedFormBlackCapPricer` sums a full schedule. Both support value and sensitivities. For a caplet fixed at \\(T\\), accruing over \\([T,S]\\), with year fraction \\(\tau\\), the formula is

\\[
F = \frac{1}{\tau}\left(\frac{P(T)}{P(S)}-1\right),\qquad
\text{Caplet} = N\\,\tau\\,P_d(S)\\,[F\\,\Phi(d_1) - K\\,\Phi(d_2)],\quad
d_{1,2}=\frac{\ln(F/K)\pm\tfrac12\sigma^2 T}{\sigma\sqrt T}.
\\]

- The curve of `market_index` supplies the forward rate. The discount policy supplies the discount factor and supports dual-curve valuation through `SingleCurveCSADiscountPolicy`.
- The strike is a `Strike` (`Absolute`, `Atm`, `Relative`) resolved against \\(F\\).
- \\(\sigma\\) is read from the `VolatilitySurfaceElement` for `market_index` at `(fixing_date, strike)` through `volatility_from_date`. A `VolatilityType::Normal` surface selects the Bachelier formula.
- A cap is the sum of its caplets. Floors use the corresponding put formula.

The forward curve determines the expected reset, the discount curve converts the payoff to present value, and the surface supplies its market option price scale. Consequently, the pricer requests the relevant curves and the volatility surface of the index. Sensitivities carry both curve quote labels and the `CapletFloorlet_*` identifiers that define the surface.

## Hull-White pricers

The Hull-White pricers express a caplet as an option on a zero-coupon bond. They take the model's mean reversion \\(\alpha\\) and short-rate volatility \\(\sigma\\) directly, then price the same contract through the model dynamics:

\\[
\text{Caplet} = N\\,(1+\tau K)\\;\text{BondPut}\bigl(T, S, X\bigr),\qquad X=\frac{1}{1+\tau K},
\\]

where the zero-coupon bond option uses the volatility

\\[
\sigma_P = \sigma\\,B(T,S)\sqrt{\frac{1-e^{-2\alpha T}}{2\alpha}},\qquad B(t,T)=\frac{1-e^{-\alpha(T-t)}}{\alpha}.
\\]

Here, \\(B(T,S)\\) measures the sensitivity of the later bond price to the short rate at option expiry. Summing the resulting bond options produces a cap or floor value. These pricers are useful for checking how closely a calibrated model reproduces the Black surface used as its target. [Hull-White](../simulation/hull-white.md) explains the calibration process.

## Sensitivities

Both pricer families run a reverse sweep from the option value. `results.sensitivities()` therefore contains curve pillars such as `OIS_USD_SOFR_*`. Black pricers also produce one row per contributing volatility quote, such as `CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black`, which forms a vega ladder on the quoted grid. A fixed-parameter Hull-White pricer has curve sensitivity and model-parameter dependence. Calibrated-model risk reaches calibration quotes through the model construction path.

## What to remember

A cap or floor is a portfolio of optionlets sharing one schedule and strike rule. The Black route reads implied volatility directly from the market surface. The Hull-White route obtains option values from a short-rate model whose parameters may have been fixed or calibrated. Both routes use the same contract and curve infrastructure, which makes their values and risks directly comparable.
