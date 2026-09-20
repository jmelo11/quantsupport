# Curves Overview

Interest-rate curves turn a small set of market observations into discount factors and forward rates for every date needed by a trade. This chapter explains the common curve interface, the available curve representations, and the conventions that connect their numerical values to financial meaning. These ideas matter because pricing, simulation, and risk all rely on the same curve contract, even when the curves were constructed in different ways.

In QuantSupport, a curve is any object that implements `InterestRatesTermStructure<T>`. Production curves usually come from market calibration. Simpler hand-built curves use the same interface and are useful for tests, controlled examples, and scenario analysis.

## The trait

The term-structure trait defines the questions that downstream components may ask. A pricer can request a discount factor or a forward rate without knowing whether the answer comes from bootstrapped nodes, a flat rate, or a composite curve. The following definition shows that shared boundary:

```rust,ignore
pub trait InterestRatesTermStructure<T: Scalar> {
    fn reference_date(&self) -> Date;
    fn discount_factor(&self, date: Date) -> Result<T>;
    fn forward_rate(&self, start: Date, end: Date, comp: Compounding, freq: Frequency) -> Result<T>;
    fn nodes(&self) -> Option<Vec<(Date, T)>>;
    fn day_counter(&self) -> Option<DayCounter>;
    fn discount_factor_from_time(&self, t: f64) -> Result<T>;
    fn forward_rate_from_time(&self, start: f64, end: f64) -> Result<T>;
}
```

The scalar type `T` is usually `f64` for ordinary numerical work or `DualFwd` when automatic differentiation is required. Forward rates are derived from discount factors through `InterestRate::implied_rate`. As a result, the requested compounding convention remains consistent with the discount factors stored by the curve:

\\[
P(t_1,t_2)=\\frac{P(0,t_2)}{P(0,t_1)},\\qquad
F_{\\text{simple}}=\\frac{1}{\\tau}\\left(\\frac{1}{P(t_1,t_2)}-1\\right),\\qquad
F_{\\text{cont}}=-\\frac{\\ln P(t_1,t_2)}{\\tau}.
\\]

The first relation converts two zero-date discount factors into a discount factor over a subperiod. The other relations express that same quantity as a simple or continuously compounded forward rate. These identities are the common economic foundation beneath every curve implementation.

Constructed curves are held in a `DiscountCurveElement`. Internally, that wrapper uses `Rc<RefCell<dyn ADCurveElement>>`, where an `ADCurveElement` supports both term-structure queries and named risk pillars. Calling `element.curve()` borrows the underlying curve through this wrapper. Shared ownership allows several pricers to refer to one market curve. Interior mutability supports the controlled tape operations needed for automatic differentiation.

## Implementations

Different representations suit different jobs. A nodal curve is appropriate for calibrated market data, a flat curve offers a compact controlled assumption, and composite curves express adjustments to an existing base. Each representation implements the same trait, so choosing one leaves pricer interfaces unchanged.

### `DiscountTermStructure<T>`

`DiscountTermStructure<T>` is the main nodal representation. It stores discount factors at dated pillars and interpolates between them using year fractions measured by a specified day counter. The example below constructs a three-node curve and attaches labels that will later identify its risk sensitivities:

```rust,ignore
let curve = DiscountTermStructure::<DualFwd>::new(
    vec![ref_date, ref_date + Period::from_str("3M")?, ref_date + Period::from_str("1Y")?],
    vec![DualFwd::new(1.0), DualFwd::new(0.99), DualFwd::new(0.957)],
    DayCounter::Actual360,
    Interpolator::LogLinear,
    true,
)?
.with_pillar_labels(vec!["SOFR.0M".into(), "SOFR.3M".into(), "SOFR.12M".into()])?;
```

Construction enforces the mathematical invariants of a discount curve. The date and value vectors must have equal lengths. The first date is the reference date and its discount factor must equal one. The `dates()`, `discount_factors()`, `day_counter()`, `interpolator()`, and `enable_extrapolation()` accessors expose those choices for inspection.

Pillar metadata connects the numerical curve to market risk. `with_pillar_labels` assigns stable names to nodes. `with_pillar_values` controls the values exposed through the `Pillars` interface. A bootstrapper uses market quotes for these exposed values, which makes the reported sensitivities quote sensitivities even though pricing uses discount factors. `with_ift_sensitivities` stores the Jacobian that reconnects calibrated nodes to those quotes, as explained in [Bootstrapping](bootstrapping.md).

Interpolation operates on year fractions and applies the selected interpolator to the discount factors. For example, log-linear interpolation makes the logarithm of the discount factor linear between adjacent nodes, which produces a piecewise-constant instantaneous forward rate.

### `FlatForwardTermStructure<T>`

A flat-forward curve describes the whole term structure with one rate and one rate convention. `FlatForwardTermStructure::new(reference_date, rate, rate_definition)` combines the rate with its day counter, compounding rule, and frequency. `with_pillar_label` exposes that rate as a single risk factor. This compact representation is useful when the purpose is to isolate product logic or study a simple parallel-rate scenario.

### `SpreadTermStructure<T>` and `CompositeTermStructure<T>`

A spread curve represents continuously compounded zero-rate adjustments at a set of year fractions. At each pillar, the spread is inferred from the ratio between a target discount factor and a base discount factor:

\\[
s(t_i)=-\\frac{\\ln\\!\\left(P_{\\text{target}}(t_i)/P_{\\text{base}}(t_i)\\right)}{t_i},
\\qquad P_s(t)=e^{-s(t)t}.
\\]

`SpreadTermStructure::new` stores and interpolates these adjustments. `CompositeTermStructure::new(spread_curve, base_curve)` then multiplies the spread and base discount factors, so \\(P(t)=P_s(t)P_b(t)\\). This construction is useful for funding spreads and collateral adjustments because the resulting curve preserves separate sensitivities to the base market and the spread market.

## Interpolators

Interpolation determines how a finite set of pillars becomes a continuous term structure, so it is part of the financial model. `Interpolator` provides `Linear`, `LogLinear`, and `CubicSpline`, which are serialized by name in configuration. The `Interpolate` trait evaluates a point from coordinates and values and applies the curve's extrapolation policy.

Beyond the last pillar, log-linear interpolation continues with a flat forward rate. Linear and cubic-spline curves use linear extrapolation. When extrapolation is disabled, a query outside the supported range returns an error. A caller can therefore choose explicitly whether an unavailable market horizon should fail or follow a documented continuation rule.

## `Pillars<T>`

Risk reports need names and differentiable values in addition to prices. The `Pillars<T>` trait supplies that information independently of the curve's pricing interface:

```rust,ignore
pub trait Pillars<T> {
    fn pillar_labels(&self) -> Option<Vec<String>>;
    fn pillars(&self) -> Option<Vec<(String, &T)>>;
    fn put_pillars_on_tape(&mut self);
}
```

Every differentiable curve, along with `FxStore`, implements `Pillars<DualFwd>`. After a reverse sweep, a pricer iterates over `pillars()` and reads each value's adjoint under its label. When a curve was created outside the active tape, `put_pillars_on_tape()` records its independent values after `Tape::start_recording_fwd()` and before pricing.

Bootstrapped curves require one additional connection. Their pricing values are calibrated discount factors, and their market inputs are quotes. The stored implicit-function Jacobian rebuilds the dependency between the two, which lets the final report describe sensitivity to the original quotes.

## Rate conventions

Rates only have meaning when their time measurement and compounding rules are known. `Compounding` supports simple, periodic, continuous, and the two mixed conventions commonly used around short maturities. `RateDefinition::new` groups a day counter, a compounding convention, and a payment frequency so that a rate can travel with its interpretation.

`InterestRate` uses that definition to calculate compound factors, discount factors, and implied rates. Available day counters include Actual/360, Actual/365, 30/360, 30/360 US, Actual/Actual, and Business/252. Their `year_fraction` and `day_count` methods provide the time measure used by both interest-rate conversions and curve interpolation.

## What to remember

All curve implementations answer the same discounting and forwarding questions. Their differences describe how values are represented, interpolated, extrapolated, and linked to risk factors. This separation keeps pricer interfaces stable as market construction evolves from a flat test curve to a calibrated multi-curve environment.

The next chapters develop that process. [Bootstrapping](bootstrapping.md) explains how quotes determine curve nodes, [Multi-Curve Framework](multi-curve.md) describes dependencies and collateral policies, and [Volatility Surfaces](volatility.md) introduces the option markets used by volatility-dependent products and model calibration.
