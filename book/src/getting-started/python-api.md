# Python API

The `quantsupport` Python package (in `bindings/python`) wraps the Rust library with PyO3. It targets the configuration-driven workflow: load JSON or dict inputs, enter a `PricingContext`, explore the constructed market, price trades, and run XVA. Results are returned as floats or pandas `DataFrame`s.

Build it with `maturin develop -m bindings/python/Cargo.toml --release` (see [Installation](installation.md)).

## Mental model

The bindings follow the Rust API one-to-one, with three ergonomic additions:

1. Every enum argument also accepts its string name: `currency="USD"`, `side="LongReceive"`, `requests=["Value", "Sensitivities"]`. Each enum class also has a case-insensitive `parse()`.
2. Every configuration and store has `from_dict(...)` and `from_json(path)`; they are deserialised by the same Serde implementations as the Rust JSON files under `examples/*/data/`. Configurations also have `to_dict()`.
3. `PricingContext` is a context manager: entering the `with` block runs `initialize()` (bootstraps curves, builds surfaces/cubes/simulations) and starts the AD tape; exiting releases the constructed market and rewinds the tape.

## Exposed classes

| Group         | Classes                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Enums         | `Currency` (with `code`, `name`, `symbol`, `precision`, `numeric_code`), `MarketIndex` (constants plus `equity(name)`, `fx_pair(base, quote)`, `collateral(base, quote)`, `other(name)`), `Side`, `Compounding`, `Frequency`, `DayCounter`, `TimeUnit`, `BusinessDayConvention`, `Request`, `VolatilityType`, `SmileType`, `ScenarioType`, `OptionType`, `CapFloorType`, `CapletFloorletType`, `PaymentStructure` |
| Time          | `Date(y, m, d)`, `Date.parse("2025-01-01")`, `date + "6M"`, `weekday()`, `end_of_month()`, `to_datetime()`; `Period(n, TimeUnit)`, `Period.parse("1Y6M")`, `Period.from_frequency(f)`; `Calendar(name)` with `is_business_day`, `adjust`, `advance`, `business_days_between`, `holiday_list`; `DayCounter.year_fraction(start, end)`                                                                              |
| Market data   | `QuoteStore` (`reference_date`, `identifiers()`, `to_dataframe()`), `FixingStore`, `FxStore` (`FxStore()` + `.add(base, quote, rate)`), `Scenario`                                                                                                                                                                                                                                                                |
| Configuration | `CurveConfiguration`, `VolatilitySurfaceConfiguration`, `VolatilityCubeConfiguration`, `SimulationConfiguration`, `DiscountingConfig(currency, index)`                                                                                                                                                                                                                                                            |
| Constructed   | `DiscountCurve` (`nodes()`, `pillars()`, `discount_factor(date)`, `forward_rate(start, end, compounding, frequency)`), `VolatilitySurface`, `VolatilityCube`, `Simulation` (`dates()`, `n_paths`, `dt`, `paths()`)                                                                                                                                                                                                |
| Trades        | `Swap`, `BasisSwap`, `CrossCurrencySwap`, `FixFloatCrossCurrencySwap`, `FixedRateBond`, `FloatingRateNote`, `FixedRateDeposit`, `FxForward`, `FxOption`, `EquityOption`, `CreditDefaultSwap`, `CapFloor`, `CapletFloorlet`, `RateFutures`                                                                                                                                                                         |
| Results       | `EvaluationResults` (`price`, `fair_rate`, `sensitivities` DataFrame, `cashflows` DataFrame)                                                                                                                                                                                                                                                                                                                      |
| XVA           | `XvaConfig` (`from_dict`/`from_json`), `CsaTerms`, `NettingSet(name, trades, csa)`, `XvaResult` (`xva_values`, `sensitivities`, `exposures`), `ExposureProfile` (`netting_set`, `to_dataframe()` with `date/epe/ene/ee`)                                                                                                                                                                                          |
| Errors        | `QuantSupportError`                                                                                                                                                                                                                                                                                                                                                                                               |

Scripting (`ScriptEngine`, `ScriptedProduct`) is **not** exposed in Python at the moment.

## Price a swap

```python
import quantsupport as qs

quotes = qs.QuoteStore.from_json("examples/bootstrap/data/quotes.json")
curves = qs.CurveConfiguration.from_json("examples/bootstrap/data/curve_specs.json")   # list
fx = qs.FxStore.from_dict([{"base": "CLP", "quote": "USD", "rate": 1 / 900}])
discounting = qs.DiscountingConfig(currency=qs.Currency.USD, index=qs.MarketIndex.SOFR)

ref = quotes.reference_date
swap = qs.Swap(
    identifier="USD_IRS_5Y",
    start_date=ref,
    maturity_date=ref + "5Y",
    notional=10_000_000.0,
    fixed_rate=0.0378,
    currency=qs.Currency.USD,
    market_index=qs.MarketIndex.SOFR,
    side=qs.Side.LongReceive,
)

with qs.PricingContext(quotes=quotes, curves=curves, fx=fx, discounting=discounting) as ctx:
    sofr = ctx.curve(qs.MarketIndex.SOFR)
    print(sofr.nodes())                                   # DataFrame: date / discount_factor
    print(sofr.discount_factor(ref + "5Y"))
    print(sofr.forward_rate(ref + "1Y", ref + "2Y", qs.Compounding.Simple, qs.Frequency.Annual))

    res = ctx.evaluate(swap, [qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities])
    print(res.price)              # float
    print(res.sensitivities)      # DataFrame: pillar / value  (one row per quote identifier)
    print(res.cashflows)          # DataFrame: date / type / amount / currency / ...
```

`PricingContext(quotes, curves, fixings=None, fx=None, volatility_surfaces=None, volatility_cubes=None, simulations=None, discounting=None, scenarios=None)` mirrors the Rust builder methods. After `initialize()`/inside the `with` block: `ctx.curves()`, `ctx.curve(index)`, `ctx.volatility_surfaces()`, `ctx.volatility_surface(index)`, `ctx.volatility_cubes()`, `ctx.simulations()`, `ctx.simulation(index)`; inputs are available as `ctx.reference_date`, `ctx.quotes`, `ctx.curve_configurations`, etc.

`ctx.evaluate(trade, requests)` picks the pricer for the trade type internally (the equivalent of the Rust `Evaluator`).

## Scenarios

```python
bumped = qs.Scenario("SOFR", 0.0001, qs.ScenarioType.Absolute)   # +1 bp on every SOFR quote
with qs.PricingContext(quotes=quotes, curves=curves, fx=fx, discounting=discounting, scenarios=[bumped]) as ctx:
    print(ctx.evaluate(swap, ["Value"]).price)
```

The whole market is rebuilt from the shocked quotes, so the bumped NPV is consistent with the AAD sensitivities of the base context.

## XVA

```python
config = qs.XvaConfig.from_json("examples/cva/data/xva_config.json")

csa_a = qs.CsaTerms(collateral_index="SOFR", collateral_currency="USD",
                    credit_spread=0.010, recovery=0.40, funding_spread=0.005)
csa_b = qs.CsaTerms.from_json("client_b_csa.json")

with qs.PricingContext(quotes=quotes, curves=curves, fx=fx,
                       volatility_surfaces=surfaces, volatility_cubes=cubes,
                       discounting=discounting) as ctx:
    result = ctx.run_xva(config, netting_sets=[
        qs.NettingSet("clientA", [swap], csa_a),
        qs.NettingSet("clientB", [xccy], csa_b),
    ])
    print(result.xva_values)      # DataFrame: netting_set / measure / value   (CVA, DVA, FVA)
    print(result.sensitivities)   # DataFrame: parameter / value
    for profile in result.exposures:
        print(profile.netting_set)
        print(profile.to_dataframe())   # date / epe / ene / ee
```

`CsaTerms(collateral_index, collateral_currency, credit_spread, recovery, funding_spread=0.0, credit_index=None, funding_spread_curve=None, funding_index=None)` has the same fields as the Rust struct; `funding_spread_curve` is a dict `{"dates": [...], "spreads": [...]}`.

## Where to look

- [`bindings/python/README.md`](https://github.com/jmelo11/quantsupport/blob/main/bindings/python/README.md) – API surface list.
- [`bindings/python/examples/tour.ipynb`](https://github.com/jmelo11/quantsupport/blob/main/bindings/python/examples/tour.ipynb) – guided notebook covering dates, enums, market data, curve exploration, pricing, and XVA.
- `bindings/python/src/trades.rs` – add new trade wrappers by following the existing `#[pyclass]` pattern.
