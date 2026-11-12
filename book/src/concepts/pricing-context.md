# Constructing and querying a market

A valuation needs a consistent set of observations and constructed market
objects. `PricingContext` brings these together: it holds quote, fixing and FX
stores, curve and model configurations, and the objects built from them. Its
evaluation date is the quote store's reference date.

A pricer requests the market data needed for a trade, and the context constructs
and caches the corresponding dependencies. Applications can also call
`initialize()` to build the configured market in advance.

The following fragment illustrates configuration-driven setup; `quotes` and
`curves` are previously loaded typed inputs:

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

`validate()` checks compatibility and duplicate configuration without calibrating
curves or generating paths. `initialize()` clears request caches, starts from
supplied runtime elements, then constructs configured discount curves, credit
curves, volatility surfaces, cubes and simulations in that order. Later
components can therefore consume earlier dependencies. Reinitialization
rebuilds from the original inputs.

Native pricers and simulation services manage their own differentiation
recording scopes. In Python, entering a context also establishes the binding's
scope and enables construction on request; explicit `initialize()` performs
the eager build.

## Dependency-driven construction

A `MarketDataRequest` asks for a focused subset of the market: projection and
discount curves, a volatility surface/cube, simulations, historical fixings,
spot FX, or typed pricing/calibration configuration. A request through the
provider constructs and caches required objects and dependencies.

For example, a swap may need only two rate curves and a fixing series even if
the context also describes unrelated equities and simulations. A calibrated
swaption additionally needs source option identities and its volatility market.
The context reports missing configuration for the requested resource.

`ConstructedElementStore` groups constructed objects by `MarketIndex`.
Directly supplied curves are useful for controlled tests such as the first
valuation. Configuration-driven markets are more suitable for serialization,
snapshots and coherent scenario reconstruction.

## Discounting is a policy

An index identifies a curve; a `DiscountPolicy` determines which curve applies
to an object. `SingleCurveCSADiscountPolicy` selects its collateral index for
payments in the collateral currency and `Collateral(payment_ccy, collateral_ccy)`
for other currencies. `FixedIncomeDiscountPolicy` can prefer an instrument
index or choose a configured per-currency risk-free index.

The bootstrap policy uses these rules while solving calibration instruments.
Valuation pricers must also receive the appropriate policy. The context's
base currency/index guide base collateral construction, but they are not a
promise that every direct Rust pricer will automatically install a policy or
convert its output to that currency.

`PricerConfiguration` associates an instrument family with a typed pricing
implementation and model assignments. Binding services use these assignments
for supported families. Direct Rust callers construct the selected concrete
pricer.

## Independent market views

A `Scenario` borrows the base context and presents transformed quotes,
including direct zero rates, through an independent request cache. It does not
mutate the base store or require reconstructing every configured simulation.
Unchanged calibrated values can be reused with detached AD wrappers; affected
dependencies are rebuilt when consumed.

Construction inputs can be retained between valuations, while each market
view has its own cache and each valuation uses temporary differentiation
state. These lifetimes determine when a curve can be reused and when it must
be rebuilt. The [risk chapter](../risk/aad.md) explains how market inputs
remain connected to the reported derivatives.

`with_zero_rate_risk()` and `reproject_risk()` create `RiskContext` views for
reporting sensitivities in another curve basis. They retain the pricing curves
and change their AD connections to the reported pillars. Zero-rate views use
rates implied from constructed discount factors; instrument views use the
implied quotes of a supplied calibration basket. Both are accepted by existing
Rust pricers through `MarketDataProvider`. See the
[risk basis examples](../risk/aad.md#changing-the-curve-risk-basis).

## Generating a future market

`PricingContext::advance_to` generates horizon quotes, spot observations and
intervening fixing histories, then returns an ordinary independent context.
This fragment uses an existing source context, horizon, scenario, and trade:

```rust,ignore
let assumptions = ForwardMarketConfiguration::new(horizon)
    .with_volatility(VolatilityRoll::ConstantTenor)
    .with_fixings(FixingRoll::Project);
let future = context.advance_to(&assumptions)?;
let stressed = scenario.apply(&future)?;
let result = pricer.evaluate(&trade, &[Request::Value], &stressed)?;
```

`AdvanceInTime` is the component contract. Its configuration is serializable;
its borrowed `ForwardMarketSource` supplies the market dependencies. Discount,
projection, dividend and survival curves use source discount-factor ratios by
default. `CurveRoll::ConstantTenor` instead preserves remaining-time shape.
Calibration instruments are built using horizon settlement and schedule
conventions; prices, spreads, yields and FX points remain in native quote units.
Composite curves are composed from their advanced components. An explicit override
on a composite exports that curve independently on its configured target grid.
The resulting context rebuilds its own curves, surfaces and calibrated models.
Stored simulations are regenerated from the horizon when requested.

`VolatilityRoll::ConstantTenor` preserves quoted volatility at the same remaining
expiry and smile coordinate. `ForwardVariance` subtracts elapsed variance at
that coordinate and rejects negative increments. It is an explicit deterministic
scenario convention; it does not infer conditional smile dynamics from today's
option prices. Source interpolation and extrapolation restrictions still apply.

`FixingRoll::Project` retains source history and generates intervening business-day
rate observations, equity spots and FX observations. Later-dated observations in a source fixing
file are excluded to prevent look-ahead. `Preserve` retains the known history
without generating intervening observations; horizon equity and FX spots still
advance. Ordinary historical fixing requirements
continue to apply when pricing at the horizon.

Call `scenario.apply(&context)?.advance_to(&assumptions)` to shock the source before
advancement. Applying a scenario to the returned context shocks the horizon.
These operations need not produce the same market. Generate different horizons
directly from the original context to avoid repeated rebootstrap approximation.

The selected source quote level is retained; other levels are not manufactured.
Dated contracts retain their dates. Use `with_quote_replacement` to replace an
expired futures contract or another target definition. Supplied curves are
exported into direct quote grids; `with_curve_grid` chooses their target tenors.
Without a grid, future nodes are used, or 0D/1Y/2Y/5Y/10Y/30Y for curves without
nodes. Supplied volatility markets require calibration instrument identities or
an explicit `with_surface_grid` / `with_cube_grid`.

`market_advancement_details()` records the source date, source observation instant and
assumptions. Snapshots retain this metadata and dividend-curve construction
inputs. As for other snapshots, retain the fixing store separately for replay. Its JSON schema supports parameterized index
keys such as `EQ:AAPL` and `FX:EURUSD`, as well as ordinary rate indices.
Generated quotes define horizon sensitivity coordinates. Sensitivity to today's
inputs requires advancing and repricing shocked source markets; numeric quote
generation does not retain the source AD graph. Rebootstrap preserves calibration
quotes, with possible interpolation differences between target pillars.
