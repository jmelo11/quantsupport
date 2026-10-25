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
