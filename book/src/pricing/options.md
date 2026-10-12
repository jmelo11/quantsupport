# FX and Equity Options

FX and equity options share a common valuation pattern: identify a forward price from spot and carrying costs, obtain an implied volatility at the contract coordinates, and discount the expected payoff. This chapter shows how QuantSupport applies that pattern to European options and how the same market data can drive Monte Carlo valuation and forwards.

## FX options

An FX option grants the right to exchange a base currency for a quote currency at a fixed strike. Its pair direction matters because it determines the spot convention, payoff currency, and surface orientation. The following example creates a six-month call on USD against CLP and requests both value and sensitivities:

```rust
use quantsupport::prelude::*;

# fn main() -> Result<()> {
let rd = Date::new(2026, 1, 2);
let opt = MakeFxEuropeanOption::default()
    .with_identifier("USDCLP_CALL_6M".to_string())
    .with_expiry_date(rd + Period::from_str("6M")?)
    .with_strike(950.0)
    .with_option_type(EuroOptionType::Call)
    .with_base_currency(Currency::USD)
    .with_quote_currency(Currency::CLP)
    .with_pair(FxPair::new(Currency::USD, Currency::CLP)?)
    .build()?;
let trade = FxEuropeanOptionTrade::new(opt, rd, 1_000_000.0, Side::LongReceive);
# Ok(())
# }
```

The builder requires the identifier, expiry, strike, call-or-put type, both currencies, and a typed `FxPair`. Its day counter defaults to Actual/360. The trade wrapper supplies notional, trade date, and position side.

`FxEuropeanOptionPricer` requires an explicit discount policy. For USD collateral, install a SOFR policy before evaluating the trade:

```rust,ignore
let mut pricer = FxEuropeanOptionPricer::new();
pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
    MarketIndex::SOFR,
    Currency::USD,
)));
let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
```

The context must contain SOFR and `MarketIndex::Collateral(Currency::CLP, Currency::USD)` curves, USD/CLP spot, and the pair's volatility surface. The option value is expressed in the quote currency, CLP in this example.

`FxEuropeanOptionPricer` requests both discount curves, the spot from `FxStore`, and the FX volatility surface registered for the pair. It then applies the Garman-Kohlhagen representation:

\\[
F = S\\,\frac{P_{base}(T)}{P_{quote}(T)},\qquad
C = P_{quote}(T)\\,[F\Phi(d_1)-K\Phi(d_2)],\qquad
d_{1,2}=\frac{\ln(F/K)\pm\frac12\sigma^2T}{\sigma\sqrt T}.
\\]

The forward \\(F\\) incorporates the interest-rate differential between the currencies. `OrientedFxVolSurface` maps the requested pair to the stored surface and transforms the strike for a reciprocal pair. Sensitivities cover both curves, the contributing volatility quotes, and spot when it has been registered as a `DualFwd` leaf.

## Equity European options

An equity option uses the same European payoff structure with an equity spot, a dividend yield, and one discount curve. Use `MakeEquityEuropeanOption` to supply the contract terms, then wrap the instrument in a trade:

```rust
use quantsupport::prelude::*;

# fn main() -> Result<()> {
let rd = Date::new(2026, 1, 2);
let option = MakeEquityEuropeanOption::default()
    .with_identifier("AAPL_CALL_1Y".to_string())
    .with_market_index(MarketIndex::Equity("AAPL".to_string()))
    .with_expiry_date(rd + Period::new(1, TimeUnit::Years))
    .with_strike(150.0)
    .with_option_type(EuroOptionType::Call)
    .with_currency(Currency::USD)
    .build()?;
let trade = EquityEuropeanOptionTrade::new(option, 100.0, rd, Side::LongReceive);
# Ok(())
# }
```

Two pricers share this trade type and differ in how they calculate the payoff expectation:

| Pricer                               | Requests             | Method                                                                                                                                                                                                    |
| ------------------------------------ | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BlackEuropeanOptionPricer::new()`   | Value, Sensitivities | Black-Scholes with spot, dividend yield, discount curve and the equity surface at (expiry, strike)                                                                                                        |
| `BlackMCEuropeanOptionPricer::new()` | Value, Sensitivities | reads a pre-generated simulation for `market_index` from the context (`with_simulation_configurations`, `ModelConfiguration::BrownianMotion`) and returns \\(P(T)\\,\frac1n\sum_p \text{payoff}(S^p_T)\\) |

The closed-form pricer is efficient for a vanilla European payoff. The Monte Carlo pricer demonstrates the simulation route used by path-dependent products and retains each path as `DualFwd`, allowing risk to flow to spot, volatility, and curve leaves. `BrownianMotion` also exposes closed-form price and Greek functions that provide reference values for tests.

## Equity forwards

An equity forward fixes a future purchase or sale price without optionality. `MakeEquityForward` requires an identifier, market index, delivery date, strike, and currency. Its day counter defaults to Actual/360. Wrap the instrument in `EquityForwardTrade`, whose side records the position direction:

```rust
use quantsupport::prelude::*;

# fn main() -> Result<()> {
let rd = Date::new(2026, 1, 2);
let forward = MakeEquityForward::default()
    .with_identifier("AAPL_FORWARD_1Y".to_string())
    .with_market_index(MarketIndex::Equity("AAPL".to_string()))
    .with_delivery_date(rd + Period::new(1, TimeUnit::Years))
    .with_strike(150.0)
    .with_currency(Currency::USD)
    .build()?;
let trade = EquityForwardTrade::new(forward, rd, 100.0, Side::LongReceive);
# Ok(())
# }
```

`EquityForwardPricer` supports value, fair forward price, and sensitivities:

```rust,ignore
let mut pricer = EquityForwardPricer::new();
pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(
    MarketIndex::SOFR,
    Currency::USD,
)));
let results = pricer.evaluate(
    &trade,
    &[Request::Value, Request::FairRate, Request::Sensitivities],
    &ctx,
)?;
```

The equity index identifies its forwarding curve and optional dividend curve in `ConstructedElementStore`. Spot is the fixing in `FixingStore` for that index on the context evaluation date, even when the trade date is earlier. A missing spot fixing is an error; a missing dividend curve means zero dividends. With the policy above, the context also needs the SOFR payment-discount curve. Without a policy, the forwarding curve also discounts the payment.

For delivery date `T`, the fair price and signed position value are

```text
F = S * DF_dividend(T) / DF_forward(T)
NPV = notional * side * (F - strike) * DF_payment(T).
```

Sensitivities include spot delta and the pillars of the forwarding, payment-discount, and dividend curves that are present. Unsupported requests return `InvalidValueErr`. `FuturesTrade` remains the position wrapper for a generic listed future, with claim-based and scripted valuation routes.

## Vol quotes

Volatility identifiers preserve the option coordinates used to build each surface. FX examples follow the form `FxCall_USDCLP_6M_Absolute_950`, and equity examples follow `EquityCall_USD_AAPL_1Y_Absolute_150`. Both feed `VolatilitySurfaceConfiguration`. Its smile type can be `Strike`, `Delta`, or `LogMoneyness`, and the pricer supplies the coordinate required by that convention.

## What to remember

FX and equity option pricers assemble their values from clearly identified market components. Curves determine carry and discounting, spot determines the forward level, and the volatility surface determines the option premium across expiry and strike. The shared market interfaces let a caller choose closed-form or simulated valuation and preserve consistent quote-level risk.
