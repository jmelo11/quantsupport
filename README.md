# QuantSupport

QuantSupport is a quantitative-finance library written in Rust, with Python bindings provided in the same repository. It combines instrument construction, market-data bootstrapping, pricing, automatic differentiation, Monte Carlo exposure simulation, and XVA in one toolkit.

> **Project status:** QuantSupport is alpha software. The core workflows are implemented and covered by tests and runnable examples, but public APIs and configuration schemas may still change between releases.

## Capabilities

| Area | Current support |
| --- | --- |
| Instruments | Fixed-rate deposits and bonds, floating-rate notes, rate futures, swaps, basis swaps, caps/floors, caplets/floorlets, European swaptions, fixed/float and float/float cross-currency swaps, equity forwards and European options, FX forwards and options, futures, and credit default swaps |
| Pricing | Generic discounted-cashflow pricing; Black equity, FX, caplet, and cap/floor pricing; Monte Carlo equity option pricing; Hull-White caplet and cap/floor pricing; rate-futures and CDS pricing |
| Results and risk | NPV, fair rate, cashflow tables, and quote-pillar sensitivities through automatic differentiation; type-erased pricer dispatch through `Evaluator` |
| Curves | Flat and interpolated term structures, multi-curve bootstrapping, cross-curve dependencies, FX-implied collateral curves, and CDS-based survival-curve bootstrapping |
| Volatility | Interpolated volatility surfaces and cubes, Black and normal volatility conventions, FX surface orientation, and constant, surface-, cube-, or calibration-driven volatility sources |
| Models and simulation | Brownian motion, Hull-White, and LGM models; Hull-White/LGM volatility calibration; seeded Monte Carlo path generation from serializable configurations |
| Exposure and XVA | Contingent-claim decomposition, fixing preprocessing, claim compression, netting sets, CSA terms, NPV cubes, EPE/ENE/EE, CVA, DVA, FVA, and parallel AAD sensitivities |
| Market data | Quote, fixing, and FX stores; bid/mid/ask selection; absolute and relative quote scenarios that rebuild dependent curves, volatility objects, and simulations |
| Conventions and numerics | Dates, periods, schedules, IMM dates, calendars, business-day conventions, day counts, compounding, interpolation, root solvers, FFT, and probability utilities |
| Languages | Native Rust API and PyO3-based Python bindings with pandas result tables |

The Rust prelude re-exports the types used by the main workflows:

```rust
use quantsupport::prelude::*;
```

## Installation

Add the Rust crate to `Cargo.toml`:

```toml
[dependencies]
quantsupport = "0.1.4"
```

To work from this checkout instead:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

Build and test the Rust library with:

```bash
cargo build -p quantsupport
cargo test -p quantsupport
```

## Quick start: price and risk a swap

This complete example values a five-year receive-fixed USD swap against a flat SOFR curve and asks for NPV, par rate, cashflows, and curve sensitivity.

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

    println!("NPV: {:.2}", results.price().unwrap_or_default());
    println!(
        "Par rate: {:.6}",
        results.fair_rate().unwrap_or_default()
    );

    if let Some(risk) = results.sensitivities() {
        for (pillar, exposure) in risk.instrument_keys().iter().zip(risk.exposure()) {
            println!("dPV/dQuote {pillar}: {exposure:.4}");
        }
    }

    if let Some(cashflows) = results.cashflows() {
        println!("Cashflows: {}", cashflows.payment_dates().len());
    }

    Ok(())
}
```

The same program, with a more detailed cashflow report, is available in [`examples/valuation`](examples/valuation).

## Configuration-driven market setup

`PricingContext::initialize` builds the requested market objects in dependency order: scenario-shocked quotes, discount curves, credit curves, volatility surfaces, volatility cubes, then model-driven simulations. All configuration types support Serde, so production inputs can live in JSON rather than application code.

```rust,ignore
// `quotes`, `fixings`, `fx`, and the configuration vectors can be
// deserialized from the JSON schemas used under examples/*/data/.
let mut context = PricingContext::new()
    .with_quote_store(quotes)
    .with_fixing_store(fixings)
    .with_fx_store(fx)
    .with_base_currency(Currency::USD)
    .with_base_index(MarketIndex::SOFR)
    .with_curve_configurations(curve_configs)
    .with_credit_curve_configurations(credit_curve_configs)
    .with_volatility_surface_configurations(surface_configs)
    .with_volatility_cube_configurations(cube_configs)
    .with_simulation_configurations(simulation_configs);

context.initialize()?;

let market = context.constructed_elements();
let sofr_curve = market
    .discount_curve(&MarketIndex::SOFR)
    .expect("SOFR was configured");
let five_year_df = sofr_curve
    .curve()
    .discount_factor(context.evaluation_date() + Period::from_str("5Y")?)?;
println!("SOFR 5Y discount factor: {:.8}", five_year_df.value());
```

For a complete configuration-loading implementation, see [`examples/bootstrap`](examples/bootstrap).

## Scenario analysis

A scenario can target one exact quote identifier or match identifier segments such as `SOFR`, `OIS_USD_SOFR`, or `Swaption_USD`. Absolute shocks are added to quote values; relative shocks multiply them by `1 + shock`.

```rust
use std::str::FromStr;

use quantsupport::prelude::*;

fn main() -> Result<()> {
    let mut quotes = QuoteStore::new(Date::new(2025, 11, 11));
    let details = QuoteDetails::from_str("OIS_USD_SOFR_1Y")?;
    quotes.add_quote(Quote::new(details, QuoteLevels::with_mid(0.04)));

    // Add 100 basis points to every quote with a SOFR identifier segment.
    let scenario = Scenario::new("SOFR", 0.01, ScenarioType::Absolute);
    let shocked_quotes = scenario.apply(&mut quotes)?;

    let shocked_mid = quotes
        .quote("OIS_USD_SOFR_1Y")
        .and_then(|quote| quote.levels().mid())
        .unwrap_or_default();
    println!(
        "Shocked {shocked_quotes} quote(s); new 1Y OIS rate: {:.2}%",
        shocked_mid * 100.0
    );

    Ok(())
}
```

Attach scenarios with `.with_scenarios(...)` before `PricingContext::initialize()` to rebuild the full market consistently from shocked inputs.

## Runnable Rust examples

All examples below are workspace packages and use local JSON market data where appropriate.

| Example | Demonstrates | Run |
| --- | --- | --- |
| [`valuation`](examples/valuation) | Flat-curve swap NPV, cashflows, and AAD sensitivity | `cargo run -p valuation` |
| [`bootstrap`](examples/bootstrap) | JSON quote loading and dependent USD/CLP multi-curve bootstrapping | `cargo run -p bootstrap` |
| [`sensitivity`](examples/sensitivity) | Multi-curve pricing of SOFR, Term SOFR, ICP, and cross-currency swaps with pillar DV01 | `cargo run -p sensitivity` |
| [`volatilitysurface`](examples/volatilitysurface) | Building and querying an interpolated SOFR caplet Black-volatility surface | `cargo run -p volatilitysurface` |
| [`hullwhite`](examples/hullwhite) | Curve construction, caplet-vol calibration, Hull-White pricing, simulation, and plots | `cargo run -p hullwhite` |
| [`pfe`](examples/pfe) | Multi-currency LGM exposure simulation for swaps, FX products, and cross-currency swaps | `cargo run -p pfe` |
| [`cva`](examples/cva) | High-level netting-set XVA with CSA, credit/funding inputs, CVA/FVA values, exposure profiles, and AAD sensitivities | `cargo run -p cva` |

The `plot` Cargo feature enables the library's plotting helpers:

```toml
quantsupport = { version = "0.1.4", features = ["plot"] }
```

## Python bindings

The Python package exposes typed dates and enums, market-data/configuration objects, curve/volatility/simulation exploration, the supported trade specifications, pricing results as pandas tables, quote scenarios, and the high-level XVA workflow.

Build it into the active virtual environment from the repository root:

```bash
python -m pip install maturin
maturin develop -m bindings/python/Cargo.toml --release
```

Minimal usage:

```python
import quantsupport as qs

quotes = qs.QuoteStore.from_json("quotes.json")
curves = qs.CurveConfiguration.from_json("curve_specs.json")
discounting = qs.DiscountingConfig(
    currency=qs.Currency.USD,
    index=qs.MarketIndex.SOFR,
)

with qs.PricingContext(
    quotes=quotes,
    curves=curves,
    discounting=discounting,
) as context:
    sofr = context.curve(qs.MarketIndex.SOFR)
    print(sofr.nodes())
    print(sofr.discount_factor(quotes.reference_date + "5Y"))
```

See the [Python README](bindings/python/README.md) and [guided notebook](bindings/python/examples/tour.ipynb) for pricing and XVA examples.

## Current limitations

- The project is still in alpha and does not promise API or serialized-configuration stability yet.
- The high-level XVA FX model currently accepts constant FX volatility; sourcing FX volatility directly from a constructed surface remains on the roadmap.
- Some instrument representations are used for curve/volatility calibration or claim decomposition even when no standalone public pricer exists for that product.

## Contributing

Contributions are welcome. For small fixes, feel free to open a pull request directly. For larger changes or design discussions, please open an issue first.

## License

QuantSupport is released under the [MIT License](LICENSE).

## Contact

For business inquiries, contact <jmelo@live.cl>.
