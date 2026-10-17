# Constructing and querying a market

`PricingContext` binds observations, construction specifications and market
identities into one valuation environment. Its evaluation date comes from the
quote store's reference date. The context can build everything eagerly or
resolve only the dependencies requested by a pricer.

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

Eager initialization does not mean that the context owns a universal AD tape.
Native pricers and simulation services manage their recording scopes.
Python context entry establishes its own scope and enables lazy construction;
it should not be described as an implicit eager `initialize()` call.

## Dependency-driven construction

A `MarketDataRequest` asks for a focused subset of the market: projection and
discount curves, a volatility surface/cube, simulations, historical fixings,
spot FX, or typed pricing/calibration configuration. A request through the
provider constructs and caches required objects and dependencies.

For example, a swap may need only two rate curves and a fixing series even if
the context also describes unrelated equities and simulations. A calibrated
swaption additionally needs source option identities and its volatility market.
Missing configuration is reported at the requested resource rather than
replaced by an unrelated available curve.

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

`PricerConfiguration` is keyed by instrument family and contains typed
implementation/model assignments. It is not an arbitrary string registry.
Direct Rust callers still select the concrete pricer, while binding services
can use the typed selection mechanism for supported families.

## Independent market views

A `Scenario` borrows the base context and presents transformed quotes and
explicit zero-rate pillars through an independent request cache. It does not
mutate the base store or require reconstructing every configured simulation.
Unchanged calibrated values can be reused with detached AD wrappers; affected
dependencies are rebuilt when consumed.

This separation gives three useful lifetimes: persistent construction inputs,
constructed objects cached for a market view, and temporary numerical/tape
state for a valuation. Keeping these lifetimes distinct avoids stale curves,
invalid derivative pointers and unnecessary construction work. The
[risk chapters](../risk/aad.md) explain what is retained when the final output
is a derivative rather than just a value.
