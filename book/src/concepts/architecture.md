# Architecture

QuantSupport separates product economics, observable data, constructed market objects, and valuation algorithms.

```text
QuoteStore / FixingStore / FxStore
                 |
      serializable configurations
                 |
          PricingContext
                 |
  curves / volatility / simulations
                 |
        Instrument + Trade
                 |
              Pricer
                 |
         EvaluationResults
```

An `Instrument` describes contractual behavior. A trade adds transaction-level state such as trade date, notional, and side. A pricer maps that trade and a `PricingContext` into requested results.

`PricingContext::initialize` constructs the market graph in dependency order. Curves are built before volatility and simulation objects that depend on them. Scenarios modify observable inputs before rebuilding the graph, preserving consistency between NPV and risk.

The scalar type is part of the architecture. Curves and instruments built with `DualFwd` retain derivatives through pricing, allowing quote-level sensitivities to emerge from the valuation graph. Monte Carlo and XVA reuse the same dates, indices, curves, and market stores.

This separation permits direct construction for unit tests and flat-market examples, while production applications can deserialize configurations and let the context orchestrate initialization.
