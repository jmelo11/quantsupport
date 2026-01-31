# Core Pricing Philosophy

## Purpose

The **core** module defines how pricing is orchestrated. It provides the abstractions for pricers,
pricing context, and result aggregation so that products can be priced consistently and
extensibly across the system.

## Pricers

- A **pricer** encapsulates the logic needed to value a product or instrument.
- Multiple pricers can exist for the same product type (e.g., analytic vs. numerical, fast vs.
  accurate, or model-specific implementations).
- Each pricer focuses on *how to price* a product, not how to obtain data; it relies on the
  pricing context to supply the needed inputs.

## Pricing Context

- The **pricing context** is the structured container for all information a pricer needs:
  market data, model parameters, valuation settings, and other shared inputs.
- It acts as the contract between data/model providers and the pricer itself.
- Pricers should request data through the context rather than directly accessing providers,
  keeping implementations decoupled from the data source.

## Pricing Results

- All outputs from pricers are captured in **pricing results**.
- Results provide a unified place to store values (price, Greeks, diagnostics, etc.) and make
  them accessible for reporting or downstream workflows.

## Parallelism and Scalability

- Pricing should be **parallelizable by design**. Each pricer should be able to operate
  independently given a pricing context and produce its results without side effects.
- This enables concurrent evaluation of multiple trades, scenarios, or models and supports
  scaling across cores or distributed systems.

## Example: Implementing and Running a Pricer

The example below sketches how a pricer could use a pricing context and return results that can
be aggregated alongside other pricers.

```rust
use rustatlas::core::{Pricer, PricingContext, PricingResults};
use rustatlas::instruments::Swap;

struct SwapPricer;

impl Pricer<Swap> for SwapPricer {
    fn price(&self, instrument: &Swap, context: &PricingContext) -> PricingResults {
        let curve = context.discount_curve("USD");
        let price = instrument.present_value(curve);

        PricingResults::new().with_value("pv", price)
    }
}

fn run_pricer(pricer: &dyn Pricer<Swap>, swap: &Swap, context: &PricingContext) {
    let results = pricer.price(swap, context);
    // Results are stored in PricingResults for downstream reporting.
    context.results_store().record(swap.id(), results);
}
```

> The example is illustrative: the pricer requests data through the context, returns structured
> results, and can be run in parallel with other pricers as long as it remains side-effect free.

## Expected Outcomes
By enforcing clear separation between pricer logic, shared context, and results aggregation,
this module enables:
- Plug-and-play pricing implementations for different products or models.
- Consistent data access patterns across the pricing stack.
- Scalable evaluation pipelines that can run in parallel.