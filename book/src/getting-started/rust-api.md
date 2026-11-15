# Working in Rust

A Rust valuation starts with a maker. Define dates or tenors, call `build()`
or `build_using(&market)`, and wrap the resulting instrument in a trade.
A pricer evaluates that trade against a `MarketDataProvider` and
returns the requested results, such as value, cashflows and sensitivities.
Import `quantsupport::prelude::*` to bring the common workflow types into scope.

The [construction guide](../concepts/instrument-resolution.md) covers date anchors,
floating loan amortization and reuse across contexts. Fully specified makers
can also construct instruments directly with `build()`.

The instrument, market provider and pricer have separate interfaces. This
allows an application to reuse a contract with different markets or numerical
methods. For example, tests can supply a flat curve directly, while a portfolio
application can construct its curves from quoted instruments.

## Scalars and ownership

Many mathematical types are generic over `T: Scalar`. Use `f64` for numerical
calculations that do not need derivatives. Differentiable instruments and
market objects use `Recorded`, which is available through the prelude. For
example, the discounted-cashflow pricer consumes trades that implement
`LegsProvider<Recorded>`.

During an ordinary valuation, the pricer registers its labelled market pillars
and captures the arithmetic that depends on them. A sensitivity request runs
compiled first derivatives of that calculation. The application continues to
call the same pricing method; it does not manage captures or intermediate
derivatives. The [AD workflow](../risk/aad.md) explains those stages and the
optional kernel interface for second derivatives.
The [implementation chapter](../implementation/ad-jit.md) follows the recorded
operations through reverse differentiation and Cranelift native compilation.

Constructed curves commonly use `Rc<RefCell<dyn ADCurveElement>>` so that a
workflow can share market objects and register their inputs when needed. A
persistent object can retain numerical observations, while the expression
identities created during valuation belong to that capture. Prepared scripting
and exposure programs share immutable machine code across workers, but each
worker owns the mutable input and output buffers used by its execution.

## Requests, dependencies and results

There are two separate requests. The caller's `Request` selects outputs such
as value, cashflows or sensitivities. The pricer's `MarketDataRequest` declares
inputs such as discount curves, fixings, FX or a volatility market. A provider
resolves the latter before the pricer runs its valuation.

`EvaluationResults` contains optional fields for the measures produced.
Request support and cashflow reporting conventions depend on the selected
pricer. Check its reference when choosing requests or reconciling reported
amounts to PV; an unsupported request leaves its result field
unpopulated.

Concrete pricers provide compile-time dispatch. For a heterogeneous portfolio,
`Evaluator` associates an erased pricer with a trade's `TypeId` and performs
runtime dispatch. Registration is an application decision; `PricerConfiguration`
does not turn every direct Rust pricer into an automatically selected service.
For example, configured swaptions use
`ClosedFormHullWhiteSwaptionPricer::from_configuration`.

## Choosing an extension boundary

An instrument owns contractual terms. Its capabilities, such as access to
legs, determine which pricers can consume it. A `DiscountPolicy` selects the
market index used to discount an object; it should not rewrite the contract.
An `InterestRatesTermStructure<T>` answers discount and forward queries without
requiring the consumer to know how the curve was constructed.

Choose the interface that corresponds to the component you need to extend:
`Pricer` for a valuation method, `MarketDataProvider` for a market-data source,
and the term-structure interface for a curve representation. A `MarketModel`
provides pathwise observations for scripts and exposure claims.

Fallible public operations use `Result<T>` with `QSError`. Context attached to
an error identifies the failing construction, event or path while preserving
the original category through `root_cause()`. Propagate errors with `?` and
interpret them at an application boundary. Numerical solvers also expose
convergence status; successful return alone is not a universal proof that an
iteration tolerance was met.

The [first valuation](first-swap.md) provides a complete Rust program.
Use the [Rust reference](https://docs.rs/quantsupport) for signatures,
parameter descriptions and implementation-specific behavior.
