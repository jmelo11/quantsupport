# Working in Rust

Rust exposes the separation between contract, position, market and methodology
directly. A `Make*` builder creates an instrument, a trade wrapper supplies
position metadata, and a concrete pricer evaluates requested outputs against a
`MarketDataProvider`. `quantsupport::prelude::*` is the usual starting point.

This structure matters more than a list of constructors. A curve-building
application and a portfolio application can share financial objects without
sharing their I/O or reporting infrastructure. An alternative market provider
can supply a controlled market in tests; an alternative pricer can value the
same contract under another numerical method.

## Scalars and ownership

Many mathematical types are generic over `T: Scalar`. Plain `f64` supports
numerical work without derivative recording. `Fwd<T>` carries a value and a
forward derivative seed, and nesting it raises the derivative order.
`Dual<T>` records reverse-mode operations. Standard deterministic pricing and
constructed-market wrappers generally use
`DualFwd = Dual<Fwd2>`, with `Fwd2 = Fwd<Fwd<f64>>`.

Generic mathematical support does not imply that every pricer accepts every
scalar. In particular, the generic discounted-cashflow pricer's implemented
trade bounds require `LegsProvider<DualFwd>`. Use the scalar required by the
whole connected workflow, rather than changing only the instrument parameter.

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

`EvaluationResults` is a non-generic reporting envelope with optional fields.
Submit only requests supported by the selected methodology: unsupported
requests can be rejected or left unpopulated depending on the implementation.
A populated cashflow table is not necessarily a signed or discounted
decomposition of reported PV.

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

These boundaries provide a disciplined extension strategy. Add a new pricing
method at `Pricer`, a new observation source at `MarketDataProvider`, and a new
curve representation at the term-structure interface. Use a `MarketModel` when
the extension must resolve pathwise observables for scripts or claims.

Fallible public operations use `Result<T>` with `QSError`. Context attached to
an error identifies the failing construction, event or path while preserving
the original category through `root_cause()`. Propagate errors with `?` and
interpret them at an application boundary. Numerical solvers also expose
convergence status; successful return alone is not a universal proof that an
iteration tolerance was met.

The [first valuation](first-swap.md) is executable Rust, not an API sketch.
The [Rust reference](https://docs.rs/quantsupport) supplies declarations when
the concepts developed here need a precise signature.
