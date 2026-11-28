# Library workflow

A valuation follows four steps:

1. Create the instrument with a maker. Give it dates or tenors, then call
   `build()`, or `build_using(&market)` when it needs market data to resolve
   its terms.
2. Wrap the instrument in a trade.
3. Pass the trade to a pricer together with a `MarketDataProvider`.
4. Read the requested results, such as value, cashflows, and sensitivities.

Import `quantsupport::prelude::*` to bring the common types into scope.

The [construction guide](../concepts/instrument-resolution.md) covers date
anchors, floating loan amortization, and reusing an instrument with different
markets. If you give a maker every term it needs, `build()` creates the
instrument directly.

Instruments, market data providers, and pricers are separate. You can value
the same contract with different markets or numerical methods. For example, a
test can pass a flat curve directly, while a portfolio application builds its
curves from quoted instruments.

## Scalars and ownership

Many numerical types are generic over `T: Scalar`:

- Use `f64` when you do not need derivatives.
- Use `Recorded` for instruments and market objects that should produce
  sensitivities. `Recorded` is in the prelude.

For example, the discounted-cashflow pricer accepts trades that implement
`LegsProvider<Recorded>`.

You get sensitivities by calling the same pricing method with a sensitivity
request. During valuation, the pricer registers the labelled market pillars and
records the calculations that depend on them. It then runs compiled first
derivatives of that calculation. You do not handle the recorded calculation or
intermediate derivatives yourself.

The [AD workflow](../risk/aad.md) describes these steps and an optional
interface for second derivatives. The
[implementation chapter](../implementation/ad-jit.md) explains how recorded
operations are differentiated and compiled to native code with Cranelift.

Built curves are usually held as `Rc<RefCell<dyn ADCurveElement>>`. Several
parts of a workflow can then share the same curve, and the curve registers its
inputs when a sensitivity calculation needs them.

Compiled scripting and exposure programs can run on several worker threads.
The compiled code is shared, and each worker has its own input and output
buffers.

## Requests, dependencies and results

There are two kinds of request:

- `Request` is what you ask for: value, cashflows, sensitivities, and so on.
- `MarketDataRequest` is what the pricer needs: discount curves, fixings, FX
  rates, or a volatility market. The market data provider supplies these before
  the pricer runs.

`EvaluationResults` has one optional field per measure. Which requests are
supported, and how cashflows are reported, depends on the pricer. Check the
pricer's documentation when you choose requests or reconcile cashflows to PV.
If a pricer does not support a request, the matching field stays empty.

Each concrete pricer is its own type, so the compiler picks the pricing code. For a portfolio with many trade types, use `Evaluator`. It maps the
`TypeId` of each trade type to a pricer and selects the pricer at run time.

You register the pricers you want with `Evaluator` yourself. Some pricers can
also be created from a `PricerConfiguration`. For example, configured swaptions
use `ClosedFormHullWhiteSwaptionPricer::from_configuration`.

## Choosing an extension boundary

Each part of the library has its own job:

- An instrument holds the contract terms. The capabilities it implements, such
  as access to its legs, decide which pricers can value it.
- A `DiscountPolicy` chooses the market index used to discount a product. It
  does not change the contract.
- An `InterestRatesTermStructure<T>` returns discount factors and forward rates.
  Code that uses it does not need to know how the curve was built.

To add your own component, implement the matching interface:

- `Pricer` for a new valuation method;
- `MarketDataProvider` for a new source of market data;
- the term-structure interface for a new curve type;
- `MarketModel` for a model that provides values along simulated paths to
  scripts and exposure claims.

## Errors

Operations that can fail return `Result<T>` with a `QSError`. The error carries
context about where the failure happened, such as the instrument being built,
the script event, or the simulation path. Call `root_cause()` to get the
original error category.

Scripting, numerical recording, compilation, and execution use the same error
type. For example, `QSError::ComparisonChanged` means that current inputs would
take a different Rust branch from the recorded calculation. Record the
calculation again for those inputs before reusing it.

Propagate errors with `?` and handle them where your application reports them.

Numerical solvers also report whether they converged. A successful return does
not by itself mean that every solver reached its tolerance, so check the
convergence status when it matters.

The [first valuation](first-swap.md) is a complete program. See the
[API reference](https://docs.rs/quantsupport) for signatures, parameters, and
details of each type.
