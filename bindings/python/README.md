# quantsupport (Python bindings)

PyO3 bindings for the [quantsupport](https://github.com/jmelo11/quantsupport) Rust
derivative pricing and risk analytics library.

## Install (development)

```bash
pip install maturin
maturin develop -m bindings/python/Cargo.toml --release
```

## Usage

Market data and configurations are typed objects created from dicts
(`from_dict`) or JSON files (`from_json`), deserialized directly by the Rust
library's serde implementations. The `PricingContext` is a context manager:
entering the `with` block bootstraps curves, volatility surfaces/cubes and
simulations, and starts the AD tape; exiting releases all constructed market
data and rewinds the tape.

```python
import quantsupport as qs

quotes = qs.QuoteStore.from_json("quotes.json")
curves = qs.CurveConfiguration.from_json("curve_specs.json")   # list
fx = qs.FxStore.from_dict([{"base": "CLP", "quote": "USD", "rate": 1 / 900}])
discounting = qs.DiscountingConfig(currency=qs.Currency.USD, index=qs.MarketIndex.SOFR)

ref = quotes.reference_date            # qs.Date
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
    # explore the bootstrapped market data
    sofr = ctx.curve(qs.MarketIndex.SOFR)
    print(sofr.nodes())                 # DataFrame: date / discount_factor
    print(sofr.discount_factor(ref + "5Y"))
    print(sofr.forward_rate(ref + "1Y", ref + "2Y",
                            qs.Compounding.Simple, qs.Frequency.Annual))

    res = ctx.evaluate(swap, [qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities])
    print(res.price)            # NPV (float)
    print(res.sensitivities)    # pandas DataFrame: pillar / value
    print(res.cashflows)        # pandas DataFrame
```

Every enum argument also accepts its string name (`currency="USD"`,
`side="pay"`, `requests=["Value"]`, ...).

### In-memory product and market schemas

All built-in products support `Product.from_dict(spec)` and
`Product.from_json(path)`. Product dict keys are exactly the keyword constructor
arguments, with the same defaults and validation. Dates accept ISO strings;
enum arguments accept their names. No `qs.Date` or enum objects are required.
The classmethods also preserve subclasses.

```python
swap = qs.Swap.from_dict({
    "identifier": "USD-IRS", "start_date": "2026-02-24",
    "maturity_date": "2031-02-24", "notional": 1_000_000,
    "fixed_rate": 0.04, "currency": "USD", "market_index": "SOFR",
    "side": "LongReceive", "fixed_leg_frequency": "Semiannual",
})

quotes = qs.QuoteStore.from_dict(quote_data)
curves = qs.CurveConfiguration.from_dict_list(curve_data)
volatility_surfaces = qs.VolatilitySurfaceConfiguration.from_dict_list(volatility_data)
discounting = qs.DiscountingConfig.from_dict({"currency": "USD", "index": "SOFR"})
fx = qs.FxStore.from_dict([{"base": "EUR", "quote": "USD", "rate": 1.1}])
```

`from_dict_list` is available on `CurveConfiguration`,
`VolatilitySurfaceConfiguration`, `VolatilityCubeConfiguration`,
`PricerConfiguration`, and `SimulationConfiguration`. Like `from_json`, it
accepts a list, one configuration dict, or a single-key object wrapping a list,
such as `{"curve_specs": [...]}`. `from_dict` on those classes still returns
one configuration. `QuoteStore`, `FixingStore`, `FxStore`, `Scenario`, and
`DiscountingConfig` keep their existing `from_dict` methods.

Market configs use the native Rust serde schemas. For product market-index
fields, rate indices are strings (`"SOFR"`); other variants use native tagged
dicts such as `{"Equity": "AAPL"}`, `{"Credit": "CORPORATE-A"}`,
`{"FxPair": ["EUR", "USD"]}`, and `{"Collateral": ["EUR", "USD"]}`.
Strikes accept a float, `"ATM"`, or a native tagged dict such as
`{"Absolute": 100.0}`.

### Bond pricing, Z-spread, and OAS

Straight fixed-rate bonds use `BondPricer`. Pass a `BondPricingAnchor` to
`PricingContext.evaluate`; clean and dirty prices are explicit anchor
constructors, not values of a second price-type enum:

`FixedRateBond.discount_index` is optional. Omit it to use the context's
fixed-income discount policy, which maps the base currency to the base index.
An explicit index remains supported for compatibility and takes precedence.
An unmapped currency produces an error rather than silently selecting a curve.

```python
anchor = qs.BondPricingAnchor.clean_price(99.25)
result = ctx.evaluate(
    bond,
    [qs.Request.Value, qs.Request.ModifiedDuration,
     qs.Request.ZSpread, qs.Request.Sensitivities],
    bond_anchor=anchor,
)
print(result.z_spread)
print(result.modified_duration)
print(result.sensitivities)  # curve rows plus <identifier>.ZSpread
```

Callable debt uses the separate `SteppedCouponBond`. Coupon definitions and
outstanding-notional percentages are positional; the final entry in each list
repeats for all later generated payment periods. Calls always use explicit
dates and strikes:

```python
callable_bond = qs.SteppedCouponBond(
    identifier="USD-STEPPED-CALL-2031", start_date=ref, maturity_date=ref + "6Y",
    notional=1_000_000, currency=qs.Currency.USD,
    coupons=[
        qs.Coupon.fixed(0.045),
        qs.Coupon.floating(qs.MarketIndex.SOFR, spread=0.006,
                           floor=0.0, cap=0.06),
    ],
    notional_percentages=[1.0, 0.90, 0.75],
    call_schedule=[
        qs.CallScheduleEntry(qs.Date(2028, 1, 1), 101.0),
        qs.CallScheduleEntry(qs.Date(2029, 1, 1), 100.0),
    ],
)

# The JSON entry is keyed by instrument_type="SteppedCouponBond" and selects
# TreeBased or BackwardFourierEvolution plus a typed HullWhite model.
pricers = qs.PricerConfiguration.from_json("pricers.json")

with qs.PricingContext(quotes=quotes, curves=curves,
                       pricer_configurations=pricers,
                       discounting=discounting) as ctx:
    result = ctx.evaluate(
        callable_bond,
        [qs.Request.Value, qs.Request.OAS, qs.Request.Sensitivities],
        bond_anchor=qs.BondPricingAnchor.dirty_price(101.10),
    )
    print(result.oas)
    print(result.sensitivities)  # curve rows plus <identifier>.OAS
```

`SteppedCouponBond` has no contractual `discount_index` argument or dictionary
field. Discounting is selected by the context's fixed-income policy. Its nested
coupon and call entries may be plain native dicts or the existing wrappers:

```python
bond_spec = {
    "identifier": "STEP", "start_date": "2026-02-24",
    "maturity_date": "2031-02-24", "notional": 1_000_000,
    "currency": "USD", "units": 100.0,
    "payment_frequency": "Semiannual", "coupon_day_counter": "Actual365",
    "coupons": [
        {"Fixed": {"rate": 0.045}},
        {"Floating": {"index": "SOFR", "spread": 0.006,
                      "floor": 0.0, "cap": 0.06}},
    ],
    "notional_percentages": [1.0, 0.90, 0.75],
    "call_schedule": [
        {"date": "2028-02-24", "strike": 101.0},
        {"start_date": "2029-02-24", "end_date": "2029-08-24", "strike": 100.0},
    ],
}
bond = qs.SteppedCouponBond.from_dict(bond_spec)
restored = qs.SteppedCouponBond.from_dict(bond.to_dict())
```

`Coupon` wraps existing native `CashflowType<f64>` coupon variants; there is
no additional coupon-definition enum. `Coupon.from_dict` and `Coupon.to_dict`
retain compact `Fixed`/`Floating` constructor shapes. Input also accepts
`FixedRateCoupon`, `FloatingRateCoupon`, and `OptionEmbeddedCoupon` tags.
Python enums remain supported in direct constructors and dictionaries,
including nested configuration dicts. For example, use `qs.Currency.USD`,
`qs.MarketIndex.SOFR`, `qs.Frequency.Semiannual`, and `qs.Request.Value` in
Python-native code; strings remain suitable for data loaded from JSON files.
Calls serialize with `start_date`; `date` remains
an input alias. Omitting `end_date` creates a discrete call. Both window
boundaries are inclusive, and reversed windows or invalid strikes are rejected.
`CallScheduleEntry(date, strike)` remains supported, along with
`CallScheduleEntry.window(start_date, end_date, strike)`, keyword
`start_date`/`end_date` construction, `from_dict`, `to_dict`, `date`,
`start_date`, `end_date`, `strike`, and `contains(date)`.

A call strike excludes contractual cashflows on the same date. Same-day
coupon and amortization are paid, while the issuer call caps the remaining
bond value. There is no exercise-type enum because the product supports issuer
calls only. Yield to maturity and modified duration are intentionally not
defined for `SteppedCouponBond`; use price, OAS, and sensitivities.

With no call entries, a stepped bond defaults to zero Z-spread instead of OAS.

### In-process payoff scripting

`ScriptedProduct.from_dict` accepts an inline specification with `events` (a
list of `{"date": "YYYY-MM-DD", "code": "..."}`) and `currency`. Optional
fields are `name` (default `"script"`, with `identifier` as an alias),
`discount_index`, `result_variable`, and `models`. `from_json(path)` and
`to_dict()` use the same schema. Native event field names `event_date`/`script`
are also accepted as aliases for `date`/`code`.

```python
script_spec = {
    "name": "SCRIPTED-NOTE",
    "currency": "USD",
    "result_variable": "note",
    "events": [{
        "date": "2026-02-24",
        "code": 'note = 0; note pays 100 on "2028-02-24";',
    }],
    "models": [{
        "market_index": "SOFR",
        "model": {"Lgm": {
            "lambda": 0.05,
            "parameter_source": {"Fixed": {"sigma": 0.0}},
        }},
    }],
}
product = qs.ScriptedProduct.from_dict(script_spec)
with qs.PricingContext(quotes=quotes, curves=curves, discounting=discounting) as ctx:
    values = ctx.evaluate_script(product, n_paths=1000, seed=42)
    inline_values = ctx.evaluate_script(script_spec, n_paths=1000, seed=42)
    print(values["note"])
```

`evaluate_script` requires an initialized context and returns a plain
`dict[str, float]` of path-averaged numeric script variables. `n_paths` must be
positive; using the same market, path count, and seed reproduces results.
`result_variable`, when supplied, selects the native engine's sensitivity
target and must name a numeric variable; the returned dict still includes all
numeric variables. This API returns values, not sensitivity or cashflow tables.
It invokes the native `ScriptEngine` and `LgmMarketModel` directly, without
subprocesses or Cargo.

Omitting `discount_index` uses the context base index for the base currency;
another script currency requires an explicit index. Models resolve per index
from context pricer configurations, then simulation configurations, then
`script_spec["models"]`, with later assignments taking precedence. Models
must be configured for the discount and any requested forward curves. There
is no implicit zero-volatility fallback. This API currently assembles rate
models only, not FX/equity factors or custom correlations.

Both native `Lgm` (`lambda`) and `HullWhite` (`alpha`) schemas are accepted;
Hull-White uses its equivalent LGM representation. `Fixed` sigma is an absolute
short-rate volatility, not a Black quote. For calibration, supply the context's
volatility surfaces/cubes and replace the parameter source with, for example:

```python
parameter_source = {"Calibrated": {
    "source": {"Surface": {"market_index": "SOFR"}},
    "calibration_basket": {"expiries": ["1Y", "2Y"], "strike": "Atm"},
}}
```

Native calibration resolves option prices from the context's market and fits
the sigma schedule. Missing models, curves, calibration data, invalid event
order/dates, and script parse/runtime failures raise Python exceptions.

See [examples/stepped_coupon_bond.ipynb](examples/stepped_coupon_bond.ipynb)
for tree/Fourier pricing and the native-versus-scripted comparison.

### Extending the library in Python

The Python module keeps the same separation as the Rust API: an instrument
contains product terms, a trade represents a position, a pricer contains the
valuation method, a market-data provider supplies the market state, and an
evaluator dispatches by trade type. These are extension points, not only names
for built-in Rust implementations.

The base classes are optional conveniences. Registration uses structural
typing, so ordinary Python classes and dataclasses work too.

```python
class ZeroCoupon(qs.Instrument):
    def __init__(self, identifier, maturity, amount, discount_index):
        super().__init__(identifier)
        self.maturity = maturity
        self.amount = amount
        self.discount_index = discount_index


class ZeroCouponTrade(qs.Trade):
    pass


class ZeroCouponPricer(qs.Pricer):
    def evaluate(self, trade, requests, context):
        instrument = trade.instrument
        discount = context.curve(instrument.discount_index)
        value = instrument.amount * discount.discount_factor(instrument.maturity)
        return qs.EvaluationResults(
            context.evaluation_date,
            instrument.identifier,
            price=value if qs.Request.Value in requests else None,
        )


note = ZeroCoupon("NOTE-1Y", ref + "1Y", 1_000_000, qs.MarketIndex.SOFR)
trade = ZeroCouponTrade(note, ref)

# Context registration participates in the normal ctx.evaluate workflow.
ctx.register_pricer(ZeroCouponTrade, ZeroCouponPricer())
with ctx:
    result = ctx.evaluate(trade, [qs.Request.Value])
    print(result.price)
```

Python registrations are resolved by exact type first and then by compatible
base type. They take precedence over built-in dispatch, so a built-in class can
also be assigned a custom pricer without modifying the Rust binding. Use
`ctx.unregister_pricer(TradeType)` to restore built-in dispatch.
Native trade specification classes are subclassable as well: an unregistered
subclass keeps its native pricer, while registering the subclass selects the
Python implementation.

For a heterogeneous collection or a custom market-data provider, use the
standalone evaluator:

```python
evaluator = qs.Evaluator()
evaluator.register(ZeroCouponTrade, ZeroCouponPricer())
result = evaluator.evaluate(trade, provider, [qs.Request.Value])
```

`provider` may be an initialized `PricingContext`, a subclass of
`MarketDataProvider`, or any Python object offering the methods required by the
registered pricer. `PricingContext` exposes `quotes`, `fixings`, `fx`,
`base_currency`, `base_index`, curves, volatility objects, and simulations.
Raw stores support `QuoteStore.value`, `FixingStore.fixing`, and `FxStore.rate`.

### XVA

The engine configuration covers only the simulation setup (models, paths,
seed, frequency). Collateral treatment and credit/funding conditions are per
client: each `NettingSet` carries its own `CsaTerms`.

```python
config = qs.XvaConfig.from_json("xva_config.json")

csa_a = qs.CsaTerms(collateral_index="SOFR", collateral_currency="USD",
                    credit_spread=0.010, recovery=0.40, funding_spread=0.005)
csa_b = qs.CsaTerms.from_json("client_b_csa.json")

with qs.PricingContext(quotes=quotes, curves=curves, fx=fx, discounting=discounting) as ctx:
    result = ctx.run_xva(config, netting_sets=[
        qs.NettingSet("clientA", [swap], csa_a),
        qs.NettingSet("clientB", [xccy], csa_b),
    ])
    print(result.xva_values)      # DataFrame: netting_set / measure / value
    print(result.sensitivities)   # DataFrame: parameter / value
    for p in result.exposures:    # per-netting-set EPE/ENE/EE profiles
        print(p.netting_set)
        print(p.to_dataframe())   # DataFrame: date / epe / ene / ee
```

See [examples/tour.ipynb](examples/tour.ipynb) for a guided notebook covering
all components (dates, enums, market data, curve exploration, pricing, XVA).

## Focused validation

After rebuilding the wheel, run `python -m unittest discover -s
bindings/python/tests`. The in-process Rust harness also tests the freshly
compiled PyO3 module without relying on an installed wheel:

```text
cargo check -p quantsupport-py
cargo test -p quantsupport-py python_dict_and_script_contracts -- --nocapture
```

On Windows, the selected interpreter's directory and runtime DLL directories
must be on `PATH` for Rust test linking/loading. For Conda, this typically
includes the environment root and `Library/bin`. Set `PYO3_PYTHON` to the
selected interpreter when Cargo cannot discover it. Do not enable
`extension-module` for Rust tests; wheel builds enable it separately.

## API surface (v1)

Enums (typed classes; every argument also accepts the string name, and each
enum has a case-insensitive `parse`):

- `Currency` — with ISO metadata (`code`, `name`, `symbol`, `precision`, `numeric_code`)
- `MarketIndex` — rate-index constants (`MarketIndex.SOFR`, ...) plus factories
  `equity(name)`, `fx_pair(base, quote)`, `collateral(base, quote)`, `other(name)`
- `Side`, `Compounding`, `Frequency`, `DayCounter`, `TimeUnit`,
  `BusinessDayConvention`, `Request`, `VolatilityType`, `SmileType`

Time:

- `Date(y, m, d)` / `Date.parse(s)` — arithmetic with days, `Period` or period
  strings (`date + "6M"`), `weekday()`, `end_of_month()`, `to_datetime()`
- `Period(n, TimeUnit)` / `Period.parse("1Y6M")` / `Period.from_frequency(f)`
- `Calendar(name)` — `is_business_day`, `adjust`, `advance`,
  `business_days_between`, `holiday_list`, `business_day_list`
- `DayCounter.year_fraction(start, end)` / `day_count(start, end)`

Market data & configuration (each with `from_dict` / `from_json`, plus
`to_dict` on configurations):

- `QuoteStore` — quotes with a reference date (`reference_date`,
  `identifiers()`, `value()`, `levels()`, `to_dataframe()`)
- `FixingStore` — historical index fixings (`add()`, `fixing()`,
  `to_dataframe()`)
- `FxStore` — FX spot rates (`add()`, `rate()`)
- `CurveConfiguration`, `VolatilitySurfaceConfiguration`,
  `VolatilityCubeConfiguration`, `PricerConfiguration`,
  `SimulationConfiguration`
- `DiscountingConfig(currency, index)` — base-curve definition of the context

Pricing:

- `PricingContext(quotes, curves, fixings=None, fx=None, volatility_surfaces=None, volatility_cubes=None, pricer_configurations=None, simulations=None, discounting=None, scenarios=None)` — context manager
- inputs: `ctx.reference_date` / `ctx.evaluation_date`, `ctx.quotes`,
  `ctx.fixings`, `ctx.fx`, `ctx.base_currency`, `ctx.base_index`,
  `ctx.curve_configurations`,
  `ctx.volatility_surface_configurations`, `ctx.volatility_cube_configurations`,
  `ctx.simulation_configurations`
- constructed outputs (after `initialize()` / inside the `with` block):
  - `ctx.curves()` / `ctx.curve(index)` → `DiscountCurve`
    (`nodes()`, `pillars()`, `discount_factor(date)`, `forward_rate(...)`)
  - `ctx.volatility_surfaces()` / `ctx.volatility_surface(index)` → `VolatilitySurface`
  - `ctx.volatility_cubes()` / `ctx.volatility_cube(index)` → `VolatilityCube`
  - `ctx.simulations()` / `ctx.simulation(index)` → `Simulation`
    (`dates()`, `n_paths`, `dt`, `paths()` DataFrame)
- `Swap`, `CrossCurrencySwap`, `FixedRateBond`, `SteppedCouponBond` — trade
  specifications with typed getters; callable stepped bonds use `Coupon` and
  explicit `CallScheduleEntry(date, strike)` terms
- `ctx.evaluate(trade, requests, bond_anchor=None)` → `EvaluationResults` (`price`, `yield_to_maturity`, `modified_duration`, `z_spread`, `oas`, `fair_rate`, `sensitivities` DataFrame, `cashflows` DataFrame)

- extensibility: `Instrument`, `Trade`, `Pricer`, `MarketDataProvider`,
  `Evaluator`; `ctx.register_pricer(type, pricer)`,
  `ctx.unregister_pricer(type)`, `ctx.pricer_for(trade)`
- Python pricer output: construct `EvaluationResults(evaluation_date,
  identifier, price=..., sensitivities=..., cashflows=...)`

XVA:

- `XvaConfig` — simulation/model setup only
- `CsaTerms` — per-client collateral treatment + credit/funding parameters
- `NettingSet(name, trades, csa)` — one client's trades under its CSA
- `ctx.run_xva(config, netting_sets)` → `XvaResult` (`xva_values` DataFrame, `sensitivities` DataFrame, `exposures`)
- `QuantSupportError` — exception raised on library errors

All JSON schemas match the Rust examples in `examples/*/data/`. New products
and pricing methods can be implemented and registered entirely in Python;
adding a native binding in `bindings/python/src/trades.rs` remains available
when Rust-level performance or integration is required.
