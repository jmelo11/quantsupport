# QuantSupport

QuantSupport is a quantitative-finance library written in Rust, with Python and Excel bindings provided in the same repository. It combines instrument construction, market-data bootstrapping, pricing, automatic differentiation, payoff scripting, Monte Carlo exposure simulation, and XVA in one toolkit.

A crate is available for usage in your project:

```rust
cargo add quantsupport
```

## Bindings

- **Python:** typed instruments, market data, pricing, and risk analytics. See the [Python README](bindings/python/README.md) for installation, build instructions, examples, and the API reference.
- **Excel:** a native XLL for 64-bit Windows Excel. See the [Excel README](bindings/excel/README.md) for downloads, installation, build instructions, and the worksheet API reference.

## Documentation

| Reading purpose | Entry point |
| --- | --- |
| Financial concepts, architecture, conventions, and all three interfaces | [QuantSupport Book](https://jmelo11.github.io/quantsupport/) |
| Exact native Rust signatures, errors, and numerical contracts | [Rust API reference](https://docs.rs/quantsupport/latest/quantsupport/) |
| Independently usable binding guides and declarations | [Python documentation](https://jmelo11.github.io/quantsupport/api/python/) and [Excel documentation](https://jmelo11.github.io/quantsupport/api/excel/) |
| Executable quantitative-finance studies | [Notebook collection](bindings/python/examples/README.md) |

## Quick start: price and risk a swap

This example values a five-year receive-fixed USD swap and asks for NPV, par
rate, cashflows, and curve sensitivity. The fixed coupon is a simple ACT/360
rate; the flat curve uses continuous ACT/360 compounding. Equal numeric rates
therefore do not imply a par swap. SOFR identifies the curve here, but this
`Swap` uses term-style floating coupons rather than daily-compounded OIS coupons.

```rust
use std::{cell::RefCell, rc::Rc};

use quantsupport::prelude::*;

fn main() -> Result<()> {
    let valuation_date = Date::new(2024, 1, 15);
    let maturity_date = Date::new(2029, 1, 15);
    let notional = 10_000_000.0;

    let swap = MakeSwap::<DualFwd>::default()
        .with_identifier("USD_IRS_5Y".to_string())
        .with_start_date(valuation_date)
        .with_maturity_date(maturity_date)
        .with_fixed_rate(0.03)
        .with_notional(notional)
        .with_rate_definition(RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Semiannual,
        ))
        .with_currency(Currency::USD)
        .with_market_index(MarketIndex::SOFR)
        .with_side(Side::LongReceive)
        .with_fixed_leg_frequency(Frequency::Semiannual)
        .with_floating_leg_frequency(Frequency::Semiannual)
        .build()?;
    let trade = SwapTrade::new(swap, valuation_date, notional, Side::LongReceive);

    let curve = FlatForwardTermStructure::new(
        valuation_date,
        DualFwd::from(0.03),
        RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Continuous,
            Frequency::Annual,
        ),
    )
    .with_pillar_label("SOFR_flat".to_string());

    let mut elements = ConstructedElementStore::default();
    elements.discount_curves_mut().insert(
        MarketIndex::SOFR,
        DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
    );

    let context = PricingContext::new()
        .with_quote_store(QuoteStore::new(valuation_date))
        .with_fixing_store(FixingStore::default())
        .with_base_currency(Currency::USD)
        .with_constructed_elements(elements);

    let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
    let requests = [
        Request::Value,
        Request::FairRate,
        Request::Cashflows,
        Request::Sensitivities,
    ];
    let results = pricer.evaluate(&trade, &requests, &context)?;

    let npv = results.price()
        .ok_or_else(|| QSError::ValueNotSetErr("swap NPV".into()))?;
    let par_rate = results.fair_rate()
        .ok_or_else(|| QSError::ValueNotSetErr("swap par rate".into()))?;
    println!("NPV (USD): {npv:.2}");
    println!("Par coupon (decimal): {par_rate:.6}");

    if let Some(risk) = results.sensitivities() {
        for (pillar, exposure) in risk.instrument_keys().iter().zip(risk.exposure()) {
            println!("dPV/dPillar {pillar}: {exposure:.4}");
            println!("Approximate USD P&L for +1 bp: {:.2}", exposure * 1e-4);
        }
    }

    if let Some(cashflows) = results.cashflows() {
        println!("Cashflows: {}", cashflows.payment_dates().len());
    }

    Ok(())
}
```

A related program with a detailed cashflow report is available in
[`examples/valuation/swaps`](examples/valuation/swaps).

The NPV is approximately -10,723.59 USD and the flat-rate derivative is
-47,405,484.05 USD per unit decimal rate, corresponding to about -4,740.55 USD
for a one-basis-point increase. This is risk to the supplied flat curve, not
to bootstrapping instruments. Raw cashflow rows are undiscounted and omit leg
directions; they cannot be summed directly to reproduce NPV.

## Architecture and components

QuantSupport separates market observables, calibrated market objects, product
definitions, and valuation engines. This keeps the same market construction and
risk machinery reusable across deterministic pricing, Monte Carlo, scripting,
and XVA.

```mermaid
flowchart LR
    A[Quotes, fixings, and FX] --> B[PricingContext]
    C[Curve, volatility, and model configuration] --> B
    B --> D[Curves, surfaces, cubes, and simulations]
    E[Native trade or scripted payoff] --> F[Pricing engine]
    D --> F
    F --> G[NPV, cashflows, fair values, and sensitivities]
    E --> H[Exposure and XVA engine]
    D --> H
    H --> I[EPE, ENE, CVA, and FVA]
```

### Market inputs and conventions

`QuoteStore`, `FixingStore`, and `FxStore` contain observable data: instrument
quotes, historical index fixings, and spot FX rates. Dates, calendars, schedules,
day-count rules, compounding, currencies, and indices provide the conventions
used to interpret those observations and build product cashflows.

Scenarios also operate at this input layer. A shocked valuation rebuilds the
dependent market rather than modifying an already-built curve, so curves,
volatility objects, and simulations remain consistent with one another.

### Market construction

`PricingContext` is the boundary between raw inputs and objects that can be used
for valuation. It combines the stores with serializable configuration and builds
the dependency graph in order:

1. discount and forwarding curves, including multi-curve and collateralized FX
   dependencies;
2. credit curves bootstrapped from CDS quotes;
3. volatility surfaces and cubes;
4. calibrated model configurations and Monte Carlo simulations.

The resulting `ConstructedElementStore` is shared by all downstream engines.
Pricers request only the curves, fixings, FX rates, volatility objects, or paths
needed by a particular trade. The [market lifecycle](book/src/concepts/pricing-context.md)
chapter develops the distinction between observations, construction, and
valuation dependencies.

For example, a production context can be assembled from JSON-backed stores and
configuration, then initialized once before pricing a portfolio:

```rust,ignore
let mut context = PricingContext::new()
    .with_quote_store(quotes)
    .with_fixing_store(fixings)
    .with_fx_store(fx)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR)
    .with_curve_configurations(curve_configs)
    .with_volatility_surface_configurations(surface_configs)
    .with_simulation_configurations(simulation_configs);

context.initialize()?;
```

A scenario borrows the context and builds an independent market view. Here
SOFR quote inputs rise by one basis point. Pricing requests construct the
required curves and their dependencies; the base market stays available:

```rust,ignore
let scenario = Scenario::new(vec![
    Shock::new("SOFR", 0.0001, ScenarioType::Absolute),
]);
let shocked_market = scenario.apply(&context)?;
let stressed = pricer.evaluate(&trade, &[Request::Value], &shocked_market)?;
let base = pricer.evaluate(&trade, &[Request::Value], &context)?;
```

See [`examples/bootstrap`](examples/bootstrap) for configuration loading and
multi-curve construction, and [`examples/sensitivity`](examples/sensitivity)
for quote-level risk across dependent curves.

### Products: native instruments and scripts

There are two ways to represent a product:

- **Native instruments** model standard products such as bonds, swaps, caps,
  swaptions, equity and FX options, cross-currency swaps, futures, and CDSs.
  An instrument defines the economics and cashflows; a trade adds ownership
  information such as notional, side, and trade date.
- **Scripted products** describe bespoke payoffs as dated events that observe
  rates, discount factors, FX, or equity spots and emit payments. Scripts use
  the same market models and automatic-differentiation tape as native products,
  so they produce NPV, expected cashflows, and quote-level sensitivities without
  requiring a new Rust instrument or pricer.

Native products are the preferred path when a standard cashflow or closed-form
model exists. Scripting is intended for structured coupons, digitals, range
accruals, autocallables, and products whose terms change more quickly than the
library API. The [scripting language](book/src/scripting/language.md) and
[model-backed runtime](book/src/scripting/engine.md) chapters explain this route.

A scripted product is a dated financial event stream. For example, the following event observes SOFR for one
accrual period and adds the discounted coupon to the `note` variable, which would represent the value of the product:

```rust,ignore
let events = vec![CodedEvent::new(
    Date::new(2026, 1, 2),
    r#"
        accrual = cvg("2026-01-02", "2026-04-02", "Actual360");
        coupon = RateIndex("SOFR", "2026-01-02", "2026-04-02");
        note pays 1000000 * coupon * accrual on "2026-04-02" in "USD";
    "#
    .to_string(),
)];

let stream = EventStream::try_from(events)?;
let engine = ScriptEngine::new(
    stream,
    reference_date,
    Currency::USD,
    MarketIndex::SOFR,
)?;
let (values, cashflows) =
    engine.evaluate_with_cashflows(&mut market_model, Some("note"))?;
```

The same `EventStream` can be wrapped in `ScriptedProduct` and converted to
contingent claims for XVA. [`examples/scripting`](examples/scripting) compares
this path with a native swap for NPV, pillar sensitivities, and exposure.

### Pricing and risk

Pricers combine a trade with the market objects supplied by `PricingContext`.
Cashflow products use generic discounting, while options and optional rates
products can use Black, Hull-White, LGM, or Monte Carlo engines. A successful
valuation returns an `EvaluationResults` object. Its optional fields contain
the measures requested and supported by that engine; an absent measure is not
a zero value.

Automatic differentiation runs through market construction and valuation.
Market pillars become labelled leaves on the AAD tape, and calibrated curves
reconnect first-order risk through numerical calibration Jacobians. Reported
derivatives apply to the inputs actually differentiated by each engine;
stored `f64`-generated paths do not acquire model or volatility risk merely
because their values are wrapped in AD scalars. This is the common risk
mechanism for native pricers, scripted payoffs, and XVA. Full revaluation under
quote scenarios complements AAD for stress tests and non-linear moves.

### Exposure and XVA

Supported trades and scripted products can be converted into contingent claims,
each representing a payment. The exposure engine evaluates them across simulated
paths and aggregates them by netting set and CSA. The high-level XVA engine
produces pre-margin NPV cubes, pointwise exposure profiles, and CVA/FVA from
net-of-margin exposures when variation margin is enabled. DVA is available
through the lower-level aggregator interface. FVA uses signed exposure; ENE
profiles are likewise signed, not absolute loss magnitudes.

## Installation

Add the latest Rust crate release:

```bash
cargo add quantsupport
```

To work from this checkout instead:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

The main Rust workflows are re-exported through the prelude:

```rust
use quantsupport::prelude::*;
```

Build and test the library with:

```bash
cargo build -p quantsupport
cargo test -p quantsupport
```

## Runnable Rust examples

All examples below are workspace packages and use local JSON market data where appropriate.

| Example | Demonstrates | Run |
| --- | --- | --- |
| [`valuation/swaps`](examples/valuation/swaps) | Flat-curve swap NPV, cashflows, and AAD sensitivity | `cargo run -p valuation-swaps` |
| [`valuation/bonds`](examples/valuation/bonds) | Callable mixed stepped-coupon bond priced by tree and backward Fourier evolution | `cargo run -p valuation-bonds` |
| [`valuation/caps`](examples/valuation/caps) | Black caplet and cap-strip valuation with curve and volatility quote risk | `cargo run -p valuation-caps` |
| [`valuation/loans`](examples/valuation/loans) | Fixed contractual principal, reset requests, prepayment functions, fair-rate modes, credit and quote risk | `cargo run -p valuation-loans` |
| [`bootstrap`](examples/bootstrap) | JSON quote loading, USD/CLP multi-curve bootstrapping, and a corporate bond curve | `cargo run -p bootstrap` |
| [`sensitivity`](examples/sensitivity) | Multi-curve pricing of SOFR, Term SOFR, ICP, and cross-currency swaps with pillar DV01 | `cargo run -p sensitivity` |
| [`volatilitysurface`](examples/volatilitysurface) | Building and querying an interpolated SOFR caplet Black-volatility surface | `cargo run -p volatilitysurface` |
| [`hullwhite`](examples/hullwhite) | Curve construction, caplet-vol calibration, Hull-White pricing, simulation, and plots | `cargo run -p hullwhite` |
| [`pfe`](examples/pfe) | Multi-currency LGM exposure simulation for swaps, FX products, and cross-currency swaps | `cargo run -p pfe` |
| [`cva`](examples/cva) | High-level netting-set XVA with CSA, credit/funding inputs, CVA/FVA values, exposure profiles, and AAD sensitivities | `cargo run -p cva` |
| [`scripting`](examples/scripting) | Scripted swap vs native swap: NPV and pillar sensitivities through `ScriptEngine` | `cargo run -p scripting-examples --bin valuation` |
| [`scripting`](examples/scripting) | Scripted product as XVA contingent claims: EPE and CVA/FVA sensitivities vs native swap | `cargo run -p scripting-examples --bin xva` |
| [`scripting`](examples/scripting) | Native callable stepped-coupon bond checked against an equivalent deterministic script | `cargo run -p scripting-examples --bin stepped-coupon-bond` |

The `plot` Cargo feature enables the library's plotting helpers:

```bash
cargo add quantsupport --features plot
```

## Contributing

Contributions are welcome. For small fixes, feel free to open a pull request directly. For larger changes or design discussions, please open an issue first.

## License

QuantSupport is released under the [MIT License](LICENSE).

## Contact

For business inquiries, contact <jmelo@live.cl>.
