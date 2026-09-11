# Bootstrapping

Bootstrapping turns a `CurveConfiguration` (a list of quote identifiers) into a `DiscountTermStructure<DualFwd>` whose pillars are the market quotes. The implementation is a global Newton solve per curve followed by an implicit-function-theorem (IFT) step that connects the discount factors to the quotes on the AD tape.

## `CurveConfiguration`

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

JSON (all optional fields may be omitted):

```json
{
  "market_index": "SOFR",
  "day_counter": "Actual360",
  "interpolator": "LogLinear",
  "enable_extrapolation": true,
  "quotes": [
    "FixedRateDeposit_USD_SOFR_1D",
    "OIS_USD_SOFR_1Y", "OIS_USD_SOFR_2Y", "OIS_USD_SOFR_3Y",
    "OIS_USD_SOFR_5Y", "OIS_USD_SOFR_7Y", "OIS_USD_SOFR_10Y", "OIS_USD_SOFR_30Y"
  ]
}
```

`resolve(selector, level, fx_spot)` looks every identifier up in the `QuoteSelector`, builds the calibration instrument at the requested `Level` (`Mid`, `Bid`, `Ask`), computes its pillar date and sorts the instruments by pillar date. Missing quotes produce `NotFoundErr("Quote … not found in quotes.")`. After resolution `instruments()`, `pillar_dates()`, `pillar_labels()` (the identifiers) and `quote_values()` are available.

## Supported pillar instruments and residuals

Each quote becomes a `CalibrationInstrumentType` and contributes one residual \(F_i(x)\) to the solver:

| Quote type | Instrument | Residual |
| --- | --- | --- |
| `FixedRateDeposit` | zero-coupon deposit | NPV of the deposit legs |
| `OIS` | fixed vs overnight swap | NPV (fixed − floating) |
| `BasisSwap` | float vs float + spread | NPV |
| `FixFloatCrossCurrencySwap`, `FloatFloatCrossCurrencySwap` | two-currency swap with notional exchange | NPV in the collateral currency |
| `Future` | rate future | implied forward − market rate (convexity-adjusted if a `ConvexityAdjustment` quote exists) |
| `FxForwardPoints`, `FxOutrightForward` | FX forward | implied FX forward − market forward |

Instruments whose floating leg references another index (e.g. a `BasisSwap_USD_SOFR_TermSOFR3m_*` pillar in the `TermSOFR3m` curve) project the other index from the already-solved curve, and all legs are discounted according to the `BootstrapDiscountPolicy`.

## `MultiCurveBootstrapper`

```rust,ignore
let policy = BootstrapDiscountPolicy::new(MarketIndex::SOFR, Currency::USD);
let mut fx_store = FxStore::new();
fx_store.add_fx_rate(Currency::USD, Currency::CLP, DualFwd::new(935.0));

let curves: HashMap<MarketIndex, DiscountCurveElement> =
    MultiCurveBootstrapper::new(curve_specs, policy)
        .with_fx_store(fx_store)             // required when any spec uses Collateral(..) or FX pillars
        .bootstrap(&quote_store, Level::Mid)?;
```

`bootstrap` proceeds in four steps:

1. **Resolve** every configuration. For `MarketIndex::Collateral(ccy, coll_ccy)` specs the FX spot `coll_ccy→ccy` is passed so cross-currency notionals are FX-consistent at inception.
2. **Order** the curves topologically with `dependency_order`. A curve depends on every curve its pillar instruments need for projection or discounting. A dependency without configuration fails with `NotFoundErr("Curve X requires Y for discounting but no curve configuration was provided for it …")`; cycles fail with `InvalidValueErr("Circular dependency detected …")`.
3. **Solve** each curve in order with `bootstrap_next_curve`.
4. **Wrap** the result as `DiscountTermStructure<DualFwd>` with pillar labels, pillar values (the quotes) and IFT matrices, inside a `DiscountCurveElement`.

### The Newton solve

For a curve with \(n\) pillars the unknowns are the discount factors \(x = (P_1,\dots,P_n)\) at the pillar dates, with \(P_0 = 1\) fixed. The trial curve is a `DiscountTermStructure` with the configured interpolator, so *all* instruments are repriced on the *whole* curve at every iteration—this is a global fit rather than a sequential strip, and it handles overlapping and non-monotone pillars.

- Initial guess \(x_0 = 0.99\) for every pillar.
- `VectorNewton::new(1e-12, 200)`: tolerance \(10^{-12}\) on the residual norm, at most 200 iterations; failure returns `SolverErr`.
- The Jacobian \(J = \partial F/\partial x\) is computed by central finite differences with a relative bump of \(10^{-6}\) (floored at \(10^{-8}\)) and reused for the IFT step.

### Implicit-function-theorem sensitivities

At the solution \(F(x^{\ast}, q, z) = 0\), where \(q\) are the curve's own quotes and \(z\) the discount factors of parent curves. Differentiating gives

\[
\frac{\partial x}{\partial q} = -J^{-1}\,\frac{\partial F}{\partial q},\qquad
\frac{\partial x}{\partial z} = -J^{-1}\,\frac{\partial F}{\partial z}.
\]

Because quote \(q_i\) enters only residual \(F_i\), \(\partial F/\partial q\) is diagonal and its entries are computed analytically (`compute_quote_sensitivities`). \(\partial F/\partial z\) is computed by bumping each parent discount factor. The resulting matrices are stored with the curve (`with_ift_sensitivities`, `CrossCurveDep`) and used by `put_pillars_on_tape()` to rebuild each discount factor as

\[
P_i = P_i^{\ast} + \sum_j \frac{\partial P_i}{\partial q_j}\,(q_j - q_j^{\ast}) + \sum_k \frac{\partial P_i}{\partial z_k}\,(z_k - z_k^{\ast}),
\]

with \(q_j\) as tape leaves. Consequently, when a pricer back-propagates through a curve, sensitivities land on the **quotes**—`OIS_USD_SOFR_5Y`, `BasisSwap_USD_SOFR_TermSOFR3m_2Y`, …—including chained effects such as a TermSOFR3m swap's exposure to the SOFR OIS quotes used for discounting.

## Reading a bootstrapped curve

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

`examples/bootstrap` (`cargo run -p bootstrap`) prints, for each of SOFR, TermSOFR3m, ICP and `Collateral(CLP, USD)`, the pillar quotes, discount factors, zero rates, and interpolated DFs at 6M/4Y/15Y/20Y.

## Credit curves

`CreditCurveBootstrapper::new(Vec<CreditCurveConfiguration>).bootstrap(&quote_store, Level::Mid, &discount_curves)` strips piecewise-constant hazard rates from CDS par spreads. The result is a `CreditCurveElement` wrapping a `DiscountTermStructure` whose "discount factor" is the survival probability \(Q(t)\).

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

```json
{ "market_index": { "Credit": "ACME" }, "currency": "USD", "discount_index": "SOFR",
  "recovery": 0.4, "quotes": ["Cds_ACME_USD_1Y", "Cds_ACME_USD_5Y", "Cds_ACME_USD_10Y"] }
```

For each maturity in order, the hazard rate on the last interval is solved by bisection (bounds \(10^{-12}\) to 20, 200 iterations) so that the CDS prices to par given the previously stripped intervals. A finite-difference IFT Jacobian (spread bump \(10^{-6}\)) is attached, so `CdsPricer` sensitivities are reported per CDS quote exactly like rate sensitivities. Duplicate maturities or empty quote lists are configuration errors.

## Interpreting failures

| Error | Typical cause |
| --- | --- |
| `NotFoundErr("Quote … not found in quotes.")` | identifier typo or missing quote in the store |
| `NotFoundErr("Curve X requires Y …")` | pillar instrument references an index (projection or collateral) without configuration |
| `SolverErr` after 200 iterations | inconsistent quotes (e.g. deposit and OIS at the same pillar with very different levels), wrong day counter, or an FX spot inconsistent with forward points |
| `InvalidValueErr("Curve configuration not resolved")` | `instruments()`/`reference_date()` called before `bootstrap` |
