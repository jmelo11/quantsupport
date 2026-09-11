# FX and Equity Options

## FX options

```rust,ignore
let opt = MakeFxOption::default()
    .with_identifier("USDCLP_CALL_6M".to_string())
    .with_expiry_date(rd + Period::from_str("6M")?)
    .with_strike(950.0)
    .with_option_type(FxOptionType::Call)
    .with_base_currency(Currency::USD)
    .with_quote_currency(Currency::CLP)
    .with_pair("USDCLP".to_string())
    .build()?;
let trade = FxOptionTrade::new(opt, rd, 1_000_000.0, Side::LongReceive);
let results = FxOptionPricer::new().evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
```

Required: identifier, expiry_date, strike, option_type (`FxOptionType::{Call, Put}`), base_currency, quote_currency, pair. Default day counter Actual360.

`FxOptionPricer` requests both discount curves, the spot from the `FxStore` and the FX volatility surface registered for the pair. It prices with Garman-Kohlhagen:

\\[
F = S\\,\frac{P*{base}(T)}{P*{quote}(T)},\qquad
C = P*{quote}(T)\\,[F\Phi(d_1)-K\Phi(d_2)],\qquad
d*{1,2}=\frac{\ln(F/K)\pm\frac12\sigma^2T}{\sigma\sqrt T}.
\\]

The volatility is read through `OrientedFxVolSurface`, which inverts the strike when the surface is quoted for the reverse pair. Sensitivities cover the two curves, the vol quotes and (if registered as a `DualFwd` leaf) the spot.

## Equity European options

```rust,ignore
let option = EquityEuropeanOption::new(/* identifier, market_index, strike, expiry, EuroOptionType::Call, currency */);
let trade = EquityEuropeanOptionTrade::new(option, notional, rd, Side::LongReceive); // note argument order
```

Two pricers share the trade type:

| Pricer                               | Requests             | Method                                                                                                                                                                                                    |
| ------------------------------------ | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BlackEuropeanOptionPricer::new()`   | Value, Sensitivities | Black-Scholes with spot, dividend yield, discount curve and the equity surface at (expiry, strike)                                                                                                        |
| `BlackMCEuropeanOptionPricer::new()` | Value, Sensitivities | reads a pre-generated simulation for `market_index` from the context (`with_simulation_configurations`, `ModelConfiguration::BrownianMotion`) and returns \\(P(T)\\,\frac1n\sum_p \text{payoff}(S^p_T)\\) |

The Monte Carlo pricer keeps paths as `DualFwd`, so sensitivities to the spot/vol/curve leaves flow through the simulation. `BrownianMotion::closed_form_price / delta / vega / rho / theta(fwd, strike, vol, tau, is_call)` give reference values for tests.

## Equity forwards

`MakeEquityForward` (required identifier, market_index, delivery_date, strike, currency; defaults Actual360, LongReceive) creates an `EquityForward` priced as a cashflow-based trade (`EquityForwardTrade`). `FuturesTrade` wraps a generic listed future.

## Vol quotes

FX surfaces: `FxCall_USDCLP_6M_950_Black`-style identifiers; equity surfaces: `EquityCall_USD_AAPL_1Y_150_Black`. Both feed `VolatilitySurfaceConfiguration` with `smile_type` `Strike`, `Delta` or `LogMoneyness` — the pricer passes the `key` consistent with the configured smile type.
