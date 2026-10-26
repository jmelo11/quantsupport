# Working in Rust

A Rust valuation starts with a maker. Define dates or tenors, resolve the maker
against a `ResolutionContext`, and wrap the resulting instrument in a trade.
A pricer evaluates that trade against a `MarketDataProvider` and
returns the requested results, such as value, cashflows and sensitivities.
Import `quantsupport::prelude::*` to bring the common workflow types into scope.

The [resolution guide](../concepts/instrument-resolution.md) covers date anchors,
floating loan amortization and reuse across contexts. Fully specified makers
can also construct instruments directly with `build()`.

The instrument, market provider and pricer have separate interfaces. This
allows an application to reuse a contract with different markets or numerical
methods. For example, tests can supply a flat curve directly, while a portfolio
application can construct its curves from quoted instruments.

## Scalars and ownership

Many mathematical types are generic over `T: Scalar`. Plain `f64` supports
numerical work without derivative recording. `Fwd<T>` carries a value and a
forward derivative seed, and nesting it raises the derivative order.
`Dual<T>` records reverse-mode operations. Standard deterministic pricing and
constructed-market wrappers generally use
`DualFwd = Dual<Fwd2>`, with `Fwd2 = Fwd<Fwd<f64>>`.

Choose a scalar supported by the full valuation workflow. The generic
discounted-cashflow pricer, for example, requires trades implementing
`LegsProvider<DualFwd>`, even though many underlying mathematical types also
support `f64` and other scalars.

Constructed curves commonly use `Rc<RefCell<dyn ADCurveElement>>`. They share
market objects within a workflow and permit controlled mutation such as pillar
registration. Their underlying AD values refer to thread-local tape state.
Parallel scripting and exposure therefore reconstruct worker-local markets;
they do not move a live tape graph freely between threads.

## Requests, dependencies and results

There are two separate requests. The caller's `Request` selects outputs such
as value, cashflows or sensitivities. The pricer's `MarketDataRequest` declares
inputs such as discount curves, fixings, FX or a volatility market. A provider
resolves the latter before the pricer runs its valuation.

`EvaluationResults` contains optional fields for the measures produced.
Request support and cashflow reporting conventions depend on the selected
pricer. Check its reference when choosing requests or reconciling reported
amounts to PV; an unsupported request may raise an error or leave its field
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
