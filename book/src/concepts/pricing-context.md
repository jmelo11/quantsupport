# Constructing and querying a market

`PricingContext` holds everything a valuation needs from the market:

- the quote, fixing, and FX stores;
- the curve and model configurations;
- the market objects built from them.

The evaluation date of the context is the reference date of its quote store.

When a pricer asks for market data, the context builds what is needed and
caches it. You can also call `initialize()` to build the whole configured
market up front.

The following fragment sets up a context from configuration. `quotes` and
`curves` are inputs you have already loaded:

```rust,ignore
let mut context = PricingContext::new()
    .with_quote_store(quotes)
    .with_curve_configurations(curves)
    .with_fixing_store(fixings)
    .with_fx_store(fx)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR)
    .with_quote_level(Level::Mid);
context.initialize()?;
```

- `validate()` checks the configuration for conflicts and duplicates. It does
  not calibrate curves or generate paths.
- `initialize()` clears the cached results and starts from any market objects
  you supplied directly. It then builds, in order: discount curves, credit
  curves, volatility surfaces, volatility cubes, and simulations. Each step can
  use the objects built before it.
- Calling `initialize()` again rebuilds everything from the original inputs.

Pricers and simulations set up AD recording themselves; you do not need to.

## Dependency-driven construction

A `MarketDataRequest` asks for part of the market. It can ask for:

- projection and discount curves;
- a volatility surface or cube;
- simulations;
- historical fixings;
- spot FX rates;
- pricing or calibration configuration.

The context builds and caches the requested objects and everything they depend
on. For example, a swap may need only two rate curves and one fixing series,
even if the context also describes equities and simulations. A calibrated
swaption also needs its volatility market and the identifiers of the options
used to calibrate it. If the configuration for a requested object is missing,
the context returns an error that names it.

`ConstructedElementStore` holds built objects, grouped by `MarketIndex`. You
can insert curves directly, which is useful for tests such as the
[first valuation](../getting-started/first-swap.md). Markets built from
configuration are better when you need serialization, snapshots, or scenarios.

## Discounting is a policy

A `MarketIndex` names a curve. A `DiscountPolicy` decides which curve
discounts a given payment.

- `SingleCurveCSADiscountPolicy` uses its collateral index for payments in the
  collateral currency, and `Collateral(payment_ccy, collateral_ccy)` for
  payments in other currencies.
- `FixedIncomeDiscountPolicy` can use the instrument's own index, or a
  risk-free index configured for each currency.

Curve bootstrapping uses these policies to value calibration instruments. When
you price, pass the right policy to the pricer as well.

The context's base currency and base index are used to build the base
collateral curves. A pricer you create directly may not apply a
discount policy or convert results to the base currency on its own. Check the
pricer's documentation.

`PricerConfiguration` maps an instrument family to a pricing method and model.
The bindings use it to pick pricers for supported families. When you call the
library directly, you create the pricer you want yourself.

## Independent market views

A `Scenario` gives a shocked view of a base context. The shocked quotes,
including direct zero rates, are used only in the view; the base context does
not change. Market objects that the shock does not affect are reused. Affected
objects are rebuilt when a pricer first asks for them. Simulations are not all
rebuilt up front.

The [risk chapter](../risk/aad.md) explains how sensitivities are linked back
to the market inputs.

### Reporting risk in another basis

`with_zero_rate_risk()` and `reproject_risk()` return a `RiskContext`. It
reports sensitivities against different pillars while pricing on the same
curves.

- `with_zero_rate_risk()` reports risk to zero rates implied by the built
  discount factors.
- `reproject_risk()` reports risk to the implied quotes of a calibration basket
  that you supply.

A `RiskContext` is a `MarketDataProvider`, so any pricer accepts it. See
the [risk basis examples](../risk/aad.md#changing-the-curve-risk-basis).

## Generating a future market

`PricingContext::advance_to` builds a market at a future date (the horizon).
It generates quotes and spot values at the horizon and the fixings between
today and the horizon. It returns a new, independent context.

This fragment uses an existing context, horizon, scenario, and trade:

```rust,ignore
let assumptions = ForwardMarketConfiguration::new(horizon)
    .with_volatility(VolatilityRoll::ConstantTenor)
    .with_fixings(FixingRoll::Project);
let future = context.advance_to(&assumptions)?;
let stressed = scenario.apply(&future)?;
let result = pricer.evaluate(&trade, &[Request::Value], &stressed)?;
```

`AdvanceInTime` is the trait that performs this step. Its configuration can be
serialized. A borrowed `ForwardMarketSource` supplies the market data it reads.

### Curves

- By default, discount, projection, dividend, and survival curves roll forward
  using ratios of today's discount factors.
- `CurveRoll::ConstantTenor` keeps the curve shape by time to maturity instead.
- Calibration instruments are rebuilt with settlement and schedule conventions
  at the horizon. Prices, spreads, yields, and FX points stay in their usual
  quote units.
- A composite curve is rebuilt from its advanced components. If you give a
  composite curve its own override, it is exported on its configured target
  grid instead.
- The new context builds its own curves, surfaces, and calibrated models.
  Stored simulations are regenerated from the horizon when requested.

### Volatility

- `VolatilityRoll::ConstantTenor` keeps the same volatility at the same time to
  expiry and smile point.
- `ForwardVariance` removes the variance that has elapsed at that point. It
  fails if the remaining variance would be negative.

These are fixed scenario rules. They are not a model of how the smile evolves.
The interpolation and extrapolation limits of the source surface still apply.

### Fixings

- `FixingRoll::Project` keeps the existing history and generates business-day
  fixings for rates, equity spots, and FX up to the horizon. Fixings in the
  source file dated after today are dropped, to avoid using future data.
- `Preserve` keeps the existing history and generates no new fixings. Equity
  and FX spots still move to their horizon values.

When you price at the horizon, the usual rules for required past fixings apply.

### Combining with scenarios

- `scenario.apply(&context)?.advance_to(&assumptions)` shocks today's market,
  then rolls it forward.
- Applying a scenario to the returned context shocks the horizon market.

The two orders do not have to give the same result. To build several horizons,
advance each one directly from the original context. Advancing step by step
repeats the bootstrap and adds approximation error.

### Quotes and grids

- Only the selected quote level is carried forward. Other levels are not
  generated.
- Contracts with fixed dates keep those dates. Use `with_quote_replacement` to
  replace an expired futures contract or change another target definition.
- Curves you supplied directly are exported as direct quotes on a grid.
  `with_curve_grid` sets the tenors. Without a grid, the curve's future nodes
  are used, or 0D/1Y/2Y/5Y/10Y/30Y if the curve has no nodes.
- Volatility markets you supplied directly need the identifiers of their
  calibration instruments, or an explicit `with_surface_grid` /
  `with_cube_grid`.

### Metadata and risk

`market_advancement_details()` records the source date, source observation
time, and assumptions. Snapshots keep this metadata and the inputs for dividend
curves. As with other snapshots, keep the fixing store separately if you want
to replay the market. Its JSON format accepts index keys such as `EQ:AAPL` and
`FX:EURUSD`, as well as ordinary rate indices.

Sensitivities in the advanced market are to the generated horizon quotes. The
generated quotes are not linked to today's inputs by AD. To get sensitivity to
today's inputs, shock today's market, advance it, and reprice.

Rebootstrapping at the horizon reprices the calibration quotes. Values between
the target pillars can differ because of interpolation.
