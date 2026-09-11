# QuantSupport

QuantSupport is a quantitative-finance library written in Rust, with Python bindings provided in the same repository. It combines instrument construction, market-data bootstrapping, pricing, automatic differentiation, payoff scripting, Monte Carlo exposure simulation, and XVA in one toolkit.

## Capabilities

<table>
  <thead>
    <tr>
      <th>Area</th>
      <th>Current support</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>Instruments</td>
      <td>
        <ul>
          <li>Fixed-rate deposits and bonds</li>
          <li>Floating-rate notes</li>
          <li>Rate futures</li>
          <li>Swaps and basis swaps</li>
          <li>Caps/floors and caplets/floorlets</li>
          <li>European swaptions</li>
          <li>Fixed/float and float/float cross-currency swaps</li>
          <li>Equity forwards and European options</li>
          <li>FX forwards and options</li>
          <li>Futures</li>
          <li>Credit default swaps</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Pricing</td>
      <td>
        <ul>
          <li>Generic discounted-cashflow pricing</li>
          <li>Black equity, FX, caplet, and cap/floor pricing</li>
          <li>Monte Carlo equity option pricing</li>
          <li>Hull-White caplet, cap/floor, and European swaption pricing</li>
          <li>Rate-futures and CDS pricing</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Results and risk</td>
      <td>
        <ul>
          <li>NPV, fair rate, and cashflow tables</li>
          <li>Quote-pillar sensitivities through automatic differentiation</li>
          <li>Type-erased pricer dispatch through <code>Evaluator</code></li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Curves</td>
      <td>
        <ul>
          <li>Flat and interpolated term structures</li>
          <li>Multi-curve bootstrapping</li>
          <li>Cross-curve dependencies</li>
          <li>FX-implied collateral curves</li>
          <li>CDS-based survival-curve bootstrapping</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Volatility</td>
      <td>
        <ul>
          <li>Interpolated volatility surfaces and cubes</li>
          <li>Black and normal volatility conventions</li>
          <li>FX surface orientation</li>
          <li>Constant, surface-, cube-, or calibration-driven volatility sources</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Models and simulation</td>
      <td>
        <ul>
          <li>Brownian motion, Hull-White, and LGM models</li>
          <li>Hull-White/LGM volatility calibration</li>
          <li>Seeded Monte Carlo path generation from serializable configurations</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Exposure and XVA</td>
      <td>
        <ul>
          <li>Contingent-claim decomposition</li>
          <li>Fixing preprocessing</li>
          <li>Claim compression</li>
          <li>Netting sets and CSA terms</li>
          <li>NPV cubes</li>
          <li>EPE/ENE/EE</li>
          <li>CVA, DVA, FVA, and parallel AAD sensitivities</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Scripting</td>
      <td>
        <ul>
          <li>Payoff scripting language: assignments, <code>if</code>/<code>else</code>, <code>for</code>, <code>pays</code>, <code>RateIndex</code>, <code>Df</code>, <code>Spot</code>, <code>cvg</code>, <code>fif</code>, and arrays</li>
          <li>Dated event streams</li>
          <li>Single-tape and Rayon-parallel Monte Carlo evaluation with AAD sensitivities and expected cashflows</li>
          <li>Smoothed conditionals for digital payoffs</li>
          <li>Scripted products as XVA contingent claims</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Market data</td>
      <td>
        <ul>
          <li>Quote, fixing, and FX stores</li>
          <li>Bid/mid/ask selection</li>
          <li>Absolute and relative quote scenarios that rebuild dependent curves, volatility objects, and simulations</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Conventions and numerics</td>
      <td>
        <ul>
          <li>Dates, periods, schedules, IMM dates, and calendars</li>
          <li>Business-day conventions</li>
          <li>Day counts, compounding, and interpolation</li>
          <li>Root solvers, FFT, and probability utilities</li>
        </ul>
      </td>
    </tr>
    <tr>
      <td>Languages</td>
      <td>
        <ul>
          <li>Native Rust API</li>
          <li>PyO3-based Python bindings with pandas result tables</li>
        </ul>
      </td>
    </tr>
  </tbody>
</table>

The Rust prelude re-exports the types used by the main workflows:

```rust
use quantsupport::prelude::*;
```

## Installation

Add the Rust crate to `Cargo.toml`:

```toml
[dependencies]
quantsupport = "0.1.6"
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

## Scripting

Bespoke payoffs can be described as dated scripts instead of new Rust instruments. A script is a list of `CodedEvent`s (date + source); the `ScriptEngine` parses and indexes them once, derives the discount factors, forward rates, FX rates, and spots it needs from the market model, and evaluates every Monte Carlo path in `DualFwd`, so NPV, pillar sensitivities, and expected cashflows come out of the same run.

The language supports `=`/`+=`/`-=`/`*=`/`/=`, arithmetic (`+ - * / **`), comparisons combined with `and`/`or`/`not`, `if { } else { }`, `for x in range(a, b) { }`, arrays (`[..]`, `.append`, `.mean`, `.std`, indexing), `exp`, `ln`, `pow`, `min`, `max`, `cvg(start, end, day_counter)`, the smoothed indicator `fif(x, a, b, eps)`, market observations `RateIndex("SOFR", start, end)`, `Df(date[, curve])`, `Spot("AAPL")` / `Spot("USD", "CLP")`, and payments `acc pays amount on "date" in "CCY";`. Conditionals are evaluated with scale-aware smoothing so digital payoffs keep finite AAD sensitivities.

```rust,ignore
use quantsupport::prelude::*;

// One event per accrual period: observe SOFR on the start date, pay the net coupon at the end.
let events: Vec<CodedEvent> = periods
    .iter()
    .enumerate()
    .map(|(i, (start, end))| {
        let init = if i == 0 { "swap = 0; fixed_rate = 0.035;" } else { "" };
        CodedEvent::new(*start, format!(r#"
            {init}
            accrual = cvg("{start}", "{end}", "Actual360");
            floating_rate = RateIndex("SOFR", "{start}", "{end}");
            swap pays 10000000 * (fixed_rate - floating_rate) * accrual on "{end}";
        "#))
    })
    .collect();

let engine = ScriptEngine::new(EventStream::try_from(events)?, ref_date, Currency::USD, MarketIndex::SOFR)?;

// Any MarketModel<DualFwd> works; here an LGM model whose curve pillars are on the AD tape.
let (values, cashflows) = engine.evaluate_with_cashflows(&mut lgm_model, Some("swap"))?;
println!("NPV = {}", values["swap"]);          // pillar.adjoint() now holds dNPV/dPillar
for cf in cashflows {
    println!("{} {} amount={:.2} pv={:.2}", cf.date, cf.currency, cf.amount, cf.present_value);
}

// Multi-threaded evaluation rebuilds the model per Rayon worker through `ScriptModelSetup`
// and returns values, labelled sensitivities, and cashflows.
let parallel: ParallelScriptEvaluation = engine.evaluate_parallel(&setup, Some("swap"))?;

// The same script enters the XVA engine as ordinary contingent claims.
let claims = ScriptedProduct::new("note", EventStream::try_from(events)?, ref_date, Currency::USD, MarketIndex::SOFR)?
    .contingent_claims()?;
```

`examples/scripting` prices a swap both natively and as a script and checks that NPV, pillar sensitivities, EPE, and CVA/FVA sensitivities agree. The [Scripting](book/src/scripting/overview.md) part of the book documents the full language and runtime.

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
| [`scripting`](examples/scripting) | Scripted swap vs native swap: NPV and pillar sensitivities through `ScriptEngine` | `cargo run -p scripting-examples --bin valuation` |
| [`scripting`](examples/scripting) | Scripted product as XVA contingent claims: EPE and CVA/FVA sensitivities vs native swap | `cargo run -p scripting-examples --bin xva` |

The `plot` Cargo feature enables the library's plotting helpers:

```toml
quantsupport = { version = "0.1.6", features = ["plot"] }
```

## Python bindings

The Python are under development, but a package exposes typed dates and enums, market-data/configuration objects, curve/volatility/simulation exploration, the supported trade specifications, pricing results as pandas tables, quote scenarios, and the high-level XVA workflow.

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

## Book

The [QuantSupport Book](https://jmelo11.github.io/quantsupport/) covers installation, market construction, pricing, risk, scripting, simulation, and XVA. It is published to GitHub Pages on every push to `main`; the sources live under [`book/src`](book/src/SUMMARY.md). To build it locally, install [mdBook](https://rust-lang.github.io/mdBook/), then from the repository root:

```bash
mdbook build
mdbook serve --open
```

Generated HTML is written to `book/html/`.

## Contributing

Contributions are welcome. For small fixes, feel free to open a pull request directly. For larger changes or design discussions, please open an issue first.

## License

QuantSupport is released under the [MIT License](LICENSE).

## Contact

For business inquiries, contact <jmelo@live.cl>.
