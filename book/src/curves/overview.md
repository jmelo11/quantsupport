# Curves Overview

A _curve_ in QuantSupport is any object implementing `InterestRatesTermStructure<T>`. Curves are usually produced by the bootstrapper from quotes, but the same trait is implemented by simple hand-built structures that are useful for tests, toy models and the scripting example.

## The trait

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

`T` is `f64` or `DualFwd`. Forward rates are always derived from discount factors through `InterestRate::implied_rate`, so any compounding convention is consistent with the curve's discount factors:

\\[
P(t_1,t_2)=\frac{P(0,t_2)}{P(0,t_1)},\qquad
F_{\text{simple}}=\frac{1}{\tau}\left(\frac{1}{P(t_1,t_2)}-1\right),\qquad
F_{\text{cont}}=-\frac{\ln P(t_1,t_2)}{\tau}.
\\]

Constructed curves are wrapped as `Rc<RefCell<dyn ADCurveElement>>` inside a `DiscountCurveElement`, where `ADCurveElement = InterestRatesTermStructure<DualFwd> + Pillars<DualFwd>`. `element.curve()` returns a borrow of the underlying curve.

## Implementations

### `DiscountTermStructure<T>`

The workhorse: a set of pillar dates with discount factors, interpolated on year fractions.

```rust,ignore
let curve = DiscountTermStructure::<DualFwd>::new(
    vec![ref_date, ref_date + Period::from_str("3M")?, ref_date + Period::from_str("1Y")?],
    vec![DualFwd::new(1.0), DualFwd::new(0.99), DualFwd::new(0.957)],
    DayCounter::Actual360,
    Interpolator::LogLinear,
    true,                         // enable_extrapolation
)?
.with_pillar_labels(vec!["SOFR.0M".into(), "SOFR.3M".into(), "SOFR.12M".into()])?;   // Result<Self>
```

- `new(dates, discount_factors, day_counter, interpolator, enable_extrapolation) -> Result<Self>`: the first date is the reference date and must carry DF = 1; lengths must match.
- Accessors: `dates()`, `discount_factors()`, `day_counter()`, `interpolator()`, `enable_extrapolation()`.
- `with_pillar_labels(Vec<String>) -> Result<Self>` names the pillars for sensitivity reporting; `with_pillar_values(Vec<T>) -> Result<Self>` overrides the values exposed through `Pillars` (the bootstrapper stores the _quotes_ here, so sensitivities are reported per quote, not per DF); `with_ift_sensitivities(Vec<Vec<f64>>)` stores the Jacobian used to rebuild AD links (see [Bootstrapping](bootstrapping.md)).
- Interpolation is done on year fractions with the chosen `Interpolator` applied to the discount factors themselves; `LogLinear` therefore gives piecewise-constant forward rates.

### `FlatForwardTermStructure<T>`

`FlatForwardTermStructure::new(reference_date, rate: T, RateDefinition)` – a single rate compounded with the given `RateDefinition` (day counter, compounding, frequency). `with_pillar_label(String)` exposes the rate as one pillar. Use it in unit tests and quick what-ifs.

### `SpreadTermStructure<T>` and `CompositeTermStructure<T>`

`SpreadTermStructure::new(reference_date, year_fractions, spreads, day_counter, interpolator)` stores continuously compounded zero spreads
\\(s(t*i) = -\ln\\!\big(P*{\text{target}}(t*i)/P*{\text{base}}(t_i)\big)/t_i\\) and returns \\(P_s(t)=e^{-s(t)t}\\). `CompositeTermStructure::new(spread_curve, base_curve)` multiplies discount factors, \\(P(t)=P_s(t)\\,P_b(t)\\), taking the reference date from the base. Together they express "base curve plus spread" (funding curves, CSA adjustments) with sensitivities to the spread pillars and the base pillars kept separate.

## Interpolators

`Interpolator::{Linear, LogLinear, CubicSpline}` (serialised as strings). The `Interpolate` trait provides `interpolate(x, xs, ys, enable_extrapolation)`; extrapolation past the last pillar is flat-forward for `LogLinear` and linear for the others, and is an error when disabled.

## `Pillars<T>`

```rust,ignore
pub trait Pillars<T> {
    fn pillar_labels(&self) -> Option<Vec<String>>;
    fn pillars(&self) -> Option<Vec<(String, &T)>>;   // label → tape value
    fn put_pillars_on_tape(&mut self);
}
```

Every curve (and `FxStore`) implements `Pillars<DualFwd>`. Pricers use it to produce named sensitivities: after `Tape::backward()` they iterate `pillars()` and read `value.adjoint()`. `put_pillars_on_tape()` must be called after `Tape::start_recording_fwd()` and before pricing when the curve was built outside the current tape; the bootstrapper's curves are rebuilt from quotes through the IFT matrices so sensitivities are w.r.t. quotes rather than discount factors.

## Rate conventions

- `Compounding::{Simple, Compounded, Continuous, SimpleThenCompounded, CompoundedThenSimple}`.
- `RateDefinition::new(day_counter, compounding, frequency)`; `InterestRate::from_rate_definition(rate, def)`, `InterestRate::new(rate, compounding, frequency, day_counter)`, `compound_factor(t)`, `discount_factor(t)`, `implied_rate(compound, dc, comp, freq, t)`.
- Day counters: `DayCounter::{Actual360, Actual365, Thirty360, Thirty360US, ActualActual, Business252}` with `year_fraction(d1, d2)` and `day_count(d1, d2)`.

The next chapters cover how curves are produced: [Bootstrapping](bootstrapping.md) for single curves, [Multi-Curve Framework](multi-curve.md) for dependent curves and collateral, and [Volatility Surfaces](volatility.md) for option markets.
