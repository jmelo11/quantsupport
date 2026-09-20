# Bootstrapping

Bootstrapping finds the curve values that make a collection of market instruments reproduce their quoted prices or rates. This chapter follows that process from configuration through numerical solution and risk construction. It also explains how dependencies between curves are ordered, how credit curves use the same broad pattern, and how to interpret common failures.

QuantSupport turns a `CurveConfiguration` into a `DiscountTermStructure<DualFwd>` whose named risk pillars are the original market quotes. Each curve is solved as one global nonlinear system. An implicit-function-theorem step then connects the calibrated discount factors to the quotes on the automatic-differentiation tape.

## `CurveConfiguration`

A curve configuration identifies the market being built and the conventions used between observed pillars. Its quote identifiers define the calibration instruments and their order-independent market membership. The Rust structure makes every available choice explicit:

```rust,ignore
pub struct CurveConfiguration {
    market_index: MarketIndex,        // required
    day_counter: DayCounter,          // default Actual360
    interpolator: Interpolator,       // default LogLinear
    enable_extrapolation: bool,       // default true
    quotes: Vec<String>,              // pillar quote identifiers
}

CurveConfiguration::new(market_index, day_counter, interpolator, enable_extrapolation, quotes)
```

The same information can be supplied through JSON. The following SOFR example uses deposits at the short end and overnight-indexed swaps across the remaining maturities. Fields with documented defaults may be omitted:

```json
{
  "market_index": "SOFR",
  "day_counter": "Actual360",
  "interpolator": "LogLinear",
  "enable_extrapolation": true,
  "quotes": [
    "FixedRateDeposit_USD_SOFR_1D",
    "OIS_USD_SOFR_1Y",
    "OIS_USD_SOFR_2Y",
    "OIS_USD_SOFR_3Y",
    "OIS_USD_SOFR_5Y",
    "OIS_USD_SOFR_7Y",
    "OIS_USD_SOFR_10Y",
    "OIS_USD_SOFR_30Y"
  ]
}
```

The configuration holds identities and conventions until construction begins. `resolve(selector, level, fx_spot)` then looks up every identifier, builds the corresponding calibration instrument at the requested market level, computes its pillar date, and sorts the instruments by maturity. The supported levels are `Mid`, `Bid`, and `Ask`. A missing identifier produces `NotFoundErr("Quote … not found in quotes.")`. Once resolution succeeds, the instruments, pillar dates, labels, and quote values are available to the solver.

## Supported pillar instruments and residuals

Every quote resolves to a concrete calibration instrument. That instrument contributes one residual, which measures the difference between its model value and market target. The following table shows how each supported quote type defines that condition:

| Quote type                                                 | Instrument                               | Residual                                                                                   |
| ---------------------------------------------------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------ |
| `FixedRateDeposit`                                         | fixed-rate deposit                       | implied rate − quoted rate                                                                  |
| `FixedRateBond`                                            | option-free fixed-rate bond              | model price − the target selected by `AnchorYield` or `AnchorPrice`                         |
| `OIS`                                                      | fixed vs overnight swap                  | NPV (fixed − floating)                                                                     |
| `BasisSwap`                                                | float vs float + spread                  | NPV                                                                                        |
| `FixFloatCrossCurrencySwap`, `FloatFloatCrossCurrencySwap` | two-currency swap with notional exchange | NPV in the collateral currency                                                             |
| `Future`                                                   | rate future                              | implied forward − market rate (convexity-adjusted if a `ConvexityAdjustment` quote exists) |
| `FxForward`                                                | FX forward                               | implied outright or forward points − quote, according to its strategy                      |

Instruments whose floating leg references another index, such as a SOFR-versus-Term-SOFR basis swap, project that index from an already solved curve. Every leg obtains its discount curve from the `BootstrapDiscountPolicy`. These requirements create the dependency graph used by the multi-curve bootstrapper.

`CalibrationProcess::residual` delegates valuation to the calibration instrument's pricer. The concrete instrument carries the strategy needed to interpret its quote. A bond yield anchor, for example, converts the quoted yield to a target price using the bond's coupon compounding convention. Bond pricing reuses the fixed-leg present-value routine and normalizes the result to the quote units, whose default is 100. The current bond calibration strategies cover price and yield anchors. They exclude OAS calibration.

## `MultiCurveBootstrapper`

Real markets contain several related curves. The multi-curve bootstrapper owns their shared discount policy, resolves cross-curve dependencies, and solves them in a valid order. The example below creates a policy and supplies an FX spot needed by collateralized or cross-currency instruments:

```rust,ignore
let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
let mut fx_store = FxStore::new();
fx_store.add_fx_rate(Currency::USD, Currency::CLP, DualFwd::new(935.0));

let curves: HashMap<MarketIndex, DiscountCurveElement> =
    MultiCurveBootstrapper::new(curve_specs, policy)
        .with_fx_store(fx_store)             // required when any spec uses Collateral(..) or FX pillars
        .bootstrap(&quote_store, Level::Mid)?;
```

Calling `bootstrap` performs four stages:

1. **Resolve** every configuration. For `MarketIndex::Collateral(ccy, coll_ccy)` specs the FX spot `coll_ccy→ccy` is passed so cross-currency notionals are FX-consistent at inception.
2. **Order** the curves topologically with `dependency_order`. A curve depends on every curve its pillar instruments need for projection or discounting. A missing dependency produces `NotFoundErr("Curve X requires Y for discounting and no curve configuration was provided for it …")`. Cycles produce `InvalidValueErr("Circular dependency detected …")`.
3. **Solve** each curve in order with `bootstrap_next_curve`.
4. **Wrap** the result as `DiscountTermStructure<DualFwd>` with pillar labels, pillar values (the quotes) and IFT matrices, inside a `DiscountCurveElement`.

This sequence keeps configuration resolution, dependency management, numerical calibration, and runtime storage as distinct responsibilities. Errors can therefore identify whether the problem arose in market data, graph construction, or the solve itself.

### The Newton solve

For a curve with \\(n\\) pillars, the unknowns are the discount factors \\(x = (P_1,\dots,P_n)\\) at those dates, with \\(P_0 = 1\\) fixed at the reference date. Every iteration constructs a trial `DiscountTermStructure` with the configured interpolator and reprices the full instrument set. This global fit supports overlapping cashflows and arbitrary maturity order.

- Initial guess \\(x_0 = 0.99\\) for every pillar.
- `VectorNewton::new(1e-12, 200)` uses a tolerance of \\(10^{-12}\\) on the residual norm and allows at most 200 iterations. Failure returns `SolverErr`.
- The Jacobian \\(J = \partial F/\partial x\\) is computed by central finite differences with a relative bump of \\(10^{-6}\\) (floored at \\(10^{-8}\\)) and reused for the IFT step.

### Implicit-function-theorem sensitivities

Calibration introduces an intermediate set of solved discount factors between market quotes and trade prices. The implicit function theorem supplies the derivative through that solve without rerunning calibration for every risk factor. At the solution \\(F(x^{\ast}, q, z) = 0\\), let \\(q\\) denote the curve's quotes and \\(z\\) the discount factors of parent curves. Differentiating gives

\\[
\frac{\partial x}{\partial q} = -J^{-1}\\,\frac{\partial F}{\partial q},\qquad
\frac{\partial x}{\partial z} = -J^{-1}\\,\frac{\partial F}{\partial z}.
\\]

Each quote \\(q_i\\) enters its corresponding residual \\(F_i\\), so \\(\partial F/\partial q\\) is diagonal and `compute_quote_sensitivities` obtains its entries analytically. The parent-curve derivative \\(\partial F/\partial z\\) is calculated by bumping each parent discount factor. The resulting matrices are stored with the curve through `with_ift_sensitivities` and `CrossCurveDep`. During tape setup, `put_pillars_on_tape()` uses them to rebuild each discount factor as

\\[
P_i = P_i^{\ast} + \sum_j \frac{\partial P_i}{\partial q_j}\\,(q_j - q_j^{\ast}) + \sum_k \frac{\partial P_i}{\partial z_k}\\,(z_k - z_k^{\ast}),
\\]

Here, each \\(q_j\\) is a tape leaf. A reverse sweep therefore reports sensitivities under market identifiers such as `OIS_USD_SOFR_5Y` and `BasisSwap_USD_SOFR_TermSOFR3m_2Y`. Cross-curve terms also preserve chained effects, including a Term SOFR swap's exposure to the SOFR OIS quotes used for discounting.

## Reading a bootstrapped curve

After calibration, callers use the ordinary term-structure interface for valuation and the pillar interface for market interpretation. The following example reads an interpolated discount factor, displays the quotes represented by the curve, and derives a continuously compounded zero rate:

```rust,ignore
let elem = &curves[&MarketIndex::SOFR];
let curve = elem.curve();                           // Ref<dyn ADCurveElement>
let df = curve.discount_factor(rd + Period::from_str("4Y")?)?.value();
if let Some(pillars) = curve.pillars() {
    for (label, quote) in pillars {                 // label = quote identifier, value = quote level
        println!("{label:<40} {:>10.4}%", quote.value() * 100.0);
    }
}
let zero = -df.ln() / DayCounter::Actual360.year_fraction(rd, date);
```

The labels and values returned by `pillars()` describe calibration inputs. The discount factor returned by the curve is a solved and possibly interpolated pricing value. Together, these views let an application explain a curve in market language and use its numerical representation for pricing.

The `examples/bootstrap` program demonstrates the same inspection for SOFR, Term SOFR 3M, ICP, a USD-collateralized CLP curve, and a price-anchored corporate bond curve. It prints pillar quotes, solved discount factors, zero rates, and interpolated values at representative maturities.

## Credit curves

Credit calibration applies the same broad construction pattern to default probabilities. `CreditCurveBootstrapper` strips piecewise-constant hazard rates from CDS par spreads. Its result is a `CreditCurveElement` that wraps a `DiscountTermStructure` whose curve value represents the survival probability \\(Q(t)\\). The configuration below identifies the credit name, recovery assumption, discount curve, CDS conventions, and calibration quotes:

```rust,ignore
pub struct CreditCurveConfiguration {
    market_index: MarketIndex,      // MarketIndex::Credit("ACME")
    currency: Currency,
    discount_index: MarketIndex,    // curve discounting premium & protection legs, e.g. SOFR
    recovery: f64,                  // e.g. 0.4
    day_counter: DayCounter,        // default Actual360
    premium_frequency: Frequency,   // default Quarterly
    interpolator: Interpolator,     // default LogLinear (on survival probabilities)
    enable_extrapolation: bool,     // default true
    quotes: Vec<String>,            // "Cds_ACME_USD_1Y", "Cds_ACME_USD_5Y", ...
}
```

Applications may express the same configuration in JSON. This example calibrates the ACME survival curve from one-, five-, and ten-year USD CDS spreads:

```json
{
  "market_index": { "Credit": "ACME" },
  "currency": "USD",
  "discount_index": "SOFR",
  "recovery": 0.4,
  "quotes": ["Cds_ACME_USD_1Y", "Cds_ACME_USD_5Y", "Cds_ACME_USD_10Y"]
}
```

For each maturity, the bootstrapper solves the hazard rate on the newest interval by bisection so that the CDS prices to par given the previously stripped intervals. The search bounds run from \\(10^{-12}\\) to 20 and allow 200 iterations. A finite-difference IFT Jacobian, calculated with a \\(10^{-6}\\) spread bump, links survival probabilities to CDS quotes. `CdsPricer` can then report quote-level credit sensitivity through the same risk mechanism used by rate curves. Duplicate maturities and empty quote lists are rejected during configuration validation.

## Interpreting failures

Bootstrap errors usually point to one of three stages: resolving market inputs, ordering dependencies, or finding a consistent numerical solution. The table below maps the main error forms to the first checks an operator should make:

| Error                                                 | Typical cause                                                                                                                                               |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NotFoundErr("Quote … not found in quotes.")`         | identifier typo or missing quote in the store                                                                                                               |
| `NotFoundErr("Curve X requires Y …")`                 | pillar instrument references an index (projection or collateral) without configuration                                                                      |
| `SolverErr` after 200 iterations                      | inconsistent quotes (e.g. deposit and OIS at the same pillar with very different levels), wrong day counter, or an FX spot inconsistent with forward points |
| `InvalidValueErr("Curve configuration not resolved")` | `instruments()`/`reference_date()` called before `bootstrap`                                                                                                |

These diagnostics preserve the curve or quote identity involved in the failure. Start with that identity, then check its configured market membership and required parent curves before adjusting numerical solver settings.

## What to remember

A successful bootstrap produces more than interpolated discount factors. It records which market instruments define the curve, how dependent curves influence it, and how trade risk flows back to the original quotes. The global solver establishes pricing consistency. The implicit-function Jacobians preserve that market interpretation for automatic differentiation.

The [Multi-Curve Framework](multi-curve.md) chapter develops the discount and dependency policies used here. The [Risk](../risk/aad.md) chapters then show how the stored sensitivities participate in portfolio calculations.
