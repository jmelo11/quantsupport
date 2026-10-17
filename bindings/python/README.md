# quantsupport (Python bindings)

PyO3 bindings for the [quantsupport](https://github.com/jmelo11/quantsupport) Rust
derivative pricing and risk analytics library.

## API reference

The [Python reference](https://jmelo11.github.io/quantsupport/api/autoapi/quantsupport/index.html)
has a page per class documenting constructors, methods, and properties by
domain: `time`, `market`, `instruments`, `pricing`, and `risk`.
Public classes remain available at the
package root, so `quantsupport.Swap` and `quantsupport.instruments.Swap` are
the same class. Contract inputs and results are read-only; context input
properties have explicit setters.

The [API index](https://jmelo11.github.io/quantsupport/api/index.html) also
includes the Excel reference. Use the
[Python walkthrough](https://jmelo11.github.io/quantsupport/python/getting-started.html)
for a guided introduction. See the [API build guide](../../docs/README.md)
for local Sphinx builds from the packaged stubs; no native extension is needed
to build the reference.

## Install (development)

Source builds require Python 3.9+ and the stable Rust toolchain. From the
repository root, create and activate a virtual environment, then build with
Maturin:

```bash
python -m venv .venv
source .venv/bin/activate
python -m pip install "maturin>=1.5,<2.0"
maturin develop -m bindings/python/Cargo.toml --release
```

On Windows, activate `.venv\Scripts\Activate.ps1` instead of using `source`.
Documentation builds separately require Python 3.11+ and
`docs/requirements.txt`, followed by `mdbook build` and
`sphinx-build -W -b html docs book/html/api` as described in the build guide.

## Guided examples

Start with the [examples guide](examples/README.md) for setup and a suggested
reading order. The notebooks explain the library to readers who know the
financial products, with typed Python contracts and local data-loading helpers.
They are cataloged under the same scopes as the Rust examples: `bootstrap`,
`valuation`, `volatilitysurface`, `hullwhite`, `sensitivity`, `scripting`, and `cva`.

| Notebook | Workflow |
| --- | --- |
| [Curve bootstrapping](examples/bootstrap/curves.ipynb) | Quote inputs, construction conventions, nodes, pillars, and forwards |
| [First swap](examples/valuation/swaps/tour.ipynb) | Market inputs, dates, context lifecycle, requests, and result units |
| [Par swap](examples/valuation/swaps/par_swap.ipynb) | Solve the fixed coupon and reprice at zero PV |
| [Fixed-rate bond](examples/valuation/bonds/fixed_rate_bond.ipynb) | Price/Z-spread anchors, yield, duration, cashflows and quote risk |
| [Caps and caplets](examples/valuation/caps/caps_and_caplets.ipynb) | Black option values and curve/volatility quote risk |
| [Loans](examples/valuation/loans/loans.ipynb) | Contractual principal, reset requests, prepayment functions, fair-rate modes, credit and quote risk |
| [Curves and volatility](examples/volatilitysurface/curves_and_volatility.ipynb) | Construction, inspection, and their impact on swap and option values |
| [Simulations](examples/hullwhite/simulations.ipynb) | Calibration, path retrieval, distributions, seeds and sampling error |
| [Portfolio risk](examples/sensitivity/portfolio_risk.ipynb) | Position aggregation, DV01/vega, cashflow reports, and full-revaluation checks |
| [Stepped-coupon bond](examples/valuation/bonds/stepped_coupon_bond.ipynb) | Callable terms, tree/Fourier engines, OAS and sensitivities |
| [Scripted cashflows](examples/scripting/scripted_cashflows.ipynb) | Dated payoffs and native/scripted representation checks |
| [XVA](examples/cva/xva.ipynb) | Netting sets, client terms, exposure profiles and adjustment risk |

Execute all notebooks with `python bindings/python/examples/run_notebooks.py`
after installing the example dependencies and building the binding. The runner
uses a fresh kernel per notebook and leaves the source notebooks unchanged.

## Usage

Market data and configurations are typed objects created from dicts
(`from_dict`) or JSON files (`from_json`), deserialized directly by the Rust
library's serde implementations. Create `PricingContext` and enter `with ctx:`
to bootstrap curves, volatility surfaces/cubes and simulations automatically
and start the AD tape. Exiting releases constructed market data and rewinds
the tape, including when the block raises an exception. Call `initialize()`
explicitly only when using the context without a `with` block.

```python
import quantsupport as qs

quotes = qs.QuoteStore.from_json("quotes.json")
curves = qs.CurveConfiguration.from_json("curve_specs.json")   # list
fx = qs.FxStore.from_dict([{"base": "CLP", "quote": "USD", "rate": 1 / 900}])
csa_policy = qs.SingleCurveCSADiscountPolicy(collateral_currency=qs.Currency.USD, collateral_index=qs.MarketIndex.SOFR)

ref = quotes.reference_date            # qs.Date
swap = qs.Swap(
    identifier="USD_IRS_5Y",
    start_date=ref,
    maturity_date=ref + "5Y",
    notional=10_000_000.0,
    fixed_rate=0.0378,
    currency=qs.Currency.USD,
    market_index=qs.MarketIndex.SOFR,
)
trade = qs.Trade(swap, ref, qs.Side.LongReceive)

ctx = qs.PricingContext(quotes=quotes, curves=curves, fx=fx, csa_policy=csa_policy)

with ctx:
    # explore the bootstrapped market data
    sofr = ctx.curve(qs.MarketIndex.SOFR)
    print(sofr.nodes())                 # DataFrame: date / discount_factor
    print(sofr.discount_factor(ref + "5Y"))
    print(sofr.forward_rate(ref + "1Y", ref + "2Y",
                            qs.Compounding.Simple, qs.Frequency.Annual))

    res = ctx.evaluate(trade, [qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities])
    print(res.price)            # NPV (float)
    print(res.sensitivities)    # pandas DataFrame: pillar / value
    print(res.cashflows)        # pandas DataFrame
```

Every enum argument also accepts its string name (`currency="USD"`,
`side="pay"`, `requests=["Value"]`, ...).

The [currency-conversion walkthrough](examples/valuation/swaps/currency_conversion.ipynb)
values a CLP swap in the USD base currency under a USD CSA, explains FX pair
direction, and reconciles the native value from converted, discounted cashflows.

### Instruments And Trades

Built-in products such as `Swap`, `FixedRateBond`, and `FxOption` are
`Instrument` subclasses containing contract terms, not positions. Both
`ctx.evaluate()` and `NettingSet` require explicit `Trade` objects:

```python
long = qs.Trade(swap, ref, qs.Side.LongReceive)
short = qs.Trade(swap, ref, qs.Side.PayShort, notional=500_000)
assert long.instrument is short.instrument
```

The execution date is required and is never inferred from maturity or expiry.
`side` defaults to `LongReceive`. An optional `notional` overrides the
instrument's contractual amount for that position without mutating it.
FX forwards, FX/equity options, CDS and caplets have no contractual notional
field: their position notional belongs exclusively to `Trade` (default `1.0`).
Cross-currency positions use `domestic_notional` and `foreign_notional`
overrides; futures positions use `num_contracts` (default `1.0`). Position
amounts must be finite and non-negative; direction belongs to `side`.
Bond positions additionally own `pricing_anchor`, `settlement_date`, and
`spread_day_counter`. Product constructors and product dictionaries no longer
accept these position fields, `side`, `trade_date`, or `num_contracts`.

Select the quote level during setup with
`qs.PricingContext(quotes=quotes, curves=curves, quote_level=qs.Level.Bid)`.
The setting accepts `qs.Level.Mid`, `qs.Level.Bid`, `qs.Level.Ask`, or their
case-insensitive names, such as `quote_level="ask"`. It defaults to mid and
applies to market construction and model calibration. `ctx.quote_level`
reports the selection. Every consumed quote must provide the requested level.

### European swaptions, equity forwards, and calibration reports

`EquityForward(identifier, market_index, delivery_date, strike, currency)` is a
native instrument. Its position size defaults to one in `Trade`. Pricing uses
the evaluation-date spot fixing, the index's forward curve, an optional dividend
curve (zero dividends when absent), and the context's CSA discount policy.
`Value`, `FairRate`, and `Sensitivities` use the same Rust pricer as Excel.

```python
forward = qs.EquityForward("SPX-FWD", "EQ:SPX", "2027-02-24", 5100.0, "USD")
position = qs.Trade(forward, "2026-02-24", notional=100.0)
```

`EuropeanSwaption(identifier, expiry_date, maturity_date, strike, notional,
currency, market_index, swaption_type="Payer")` builds the underlying swap at
expiry. `SwaptionType.Payer` and `.Receiver` are also accepted. Optional rate
conventions, payment frequencies, and a calendar configure the underlying
schedule. Product notional is the default position size; `Trade.notional`
overrides it. Both products support `from_dict` and `from_json`.

Swaptions require this explicit `PricerConfiguration` in
`PricingContext(pricer_configurations=[configuration], ...)`:

```python
configuration = qs.PricerConfiguration.from_dict({
    "instrument_type": "EuropeanSwaption",
    "pricer": "HullWhiteClosedForm",
    "models": [{"market_index": "SOFR", "model": {"HullWhite": {
        "alpha": 0.1, "parameter_source": {"Calibrated": {
            "source": {"Surface": {"market_index": "SOFR"}},
            "calibration_basket": {"strike": "Atm"}
        }}
    }}}],
})
```

This engine supports `Value` and curve and volatility `Sensitivities`. It uses
the same Hull–White calibration and piecewise volatility schedule as callable
bonds and simulations. Supply the source through `volatility_surfaces` or use
`"source": {"Cube": {"market_index": "SOFR"}}` with `volatility_cubes` and a
basket selecting one tenor per expiry. The source's curve and calibration quotes
are requested automatically. `{"Fixed": {"sigma": 0.01}}` remains available
when calibration is unnecessary.

After building a calibrated Hull–White simulation,
`ctx.simulation("SOFR").calibration_diagnostics()` returns a DataFrame with
`identifier`, `expiry`, `t`, `big_t`, `market_vol`, `market_price`, `model_price`,
`calibrated_sigma`, `forward_rate`, `effective_strike`, and `price_error` (model
minus market price). Fixed-parameter and other simulation models return `None`.

Calendar strings include `UnitedKingdom(Settlement)`, `Japan`, and
`JointCalendar(JoinHolidays,UnitedKingdom(Settlement),Japan)`. `JoinHolidays`
requires every market to be open; `JoinBusinessDays` requires at least one.
SONIA and TONAR use the local bank calendars in Rust index conventions.

Wheels include the public API stubs and `py.typed` marker. Install
`quantsupport[typing]` to include the pandas stubs and typing helpers. The bindings workflow
builds and tests CPython 3.9–3.14 wheels on Linux, Windows, and both Mac
architectures. Pushes to `main` publish the wheels and a source archive to PyPI
through the [release workflow](../../README.md#releases), using the same version
as the Rust crate. Wheels are also available as Actions artifacts and GitHub
release assets. To build one locally, run `maturin build --release` inside
`bindings/python`; no Rust compiler is needed when installing the resulting wheel.

Native enum members, dates, periods, calendars, and coupon/payoff expressions have
concrete types in the stubs. Editor colours depend on semantic highlighting and
the selected theme; hover information shows the inferred type. Select the Python
environment containing the rebuilt wheel in Jupyter to use its updated stubs.

### Calendars, schedules, and IMM

Use native calendar variants directly, such as `calendar = qs.Calendar.TARGET`.
Market-specific calendars accept enums:
`qs.Calendar.UnitedStates(market=qs.UnitedStatesMarket.Sofr)`.
`qs.Calendar.JointCalendar(calendars=[qs.Calendar.TARGET, qs.Calendar.Japan],
rule=qs.JointCalendarRule.JoinHolidays)` combines typed calendars.

The [calendar catalog](https://jmelo11.github.io/quantsupport/api/python/sections/time/calendars.html)
lists every native variant. Holiday overrides return independent calendars;
they never mutate shared values such as `Calendar.TARGET`.

```python
calendar = qs.Calendar.TARGET.with_added_holiday("2026-01-05")
month_end = calendar.end_of_month("2026-05-01")
schedule = qs.Schedule(
    "2026-01-15", "2027-01-15", "3M",
    calendar=calendar,
    convention=qs.BusinessDayConvention.ModifiedFollowing,
    termination_date_convention=qs.BusinessDayConvention.ModifiedFollowing,
)
dates = schedule.dates
next_roll = qs.IMM.next_date("2026-01-01")
```

`Schedule` exposes copied dates and interval-regularity flags, immutable generation
parameters, iteration, and indexing. Its constructor wraps native `MakeSchedule`.
`IMM` provides monthly or quarterly roll dates and code conversion. See the
[component coverage](https://jmelo11.github.io/quantsupport/api/coverage.html)
for configuration-based workflows and Rust components not independently bound.

### Temporary scenarios

Combine typed `Shock` objects in `Scenario` and enter them after the pricing context:

```python
scenario = qs.Scenario(shocks=[
    qs.Shock(target="EURIBOR12m", value=0.01, shock_type=qs.ScenarioType.Absolute),
    qs.Shock(target="FUNDINGSPREAD", value=0.005, shock_type=qs.ScenarioType.Absolute),
])
with context, scenario:
    stressed = pricer.evaluate(trade=trade, context=context, requests=[qs.Request.Value])
with context:
    base = pricer.evaluate(trade=trade, context=context, requests=[qs.Request.Value])
```

The scenario borrows inputs from the innermost active context. Pricing requests
construct the required market objects and dependencies in a separate cache;
unrelated curves and simulations remain untouched. Each requested curve has
independent AD nodes, while unchanged calibration results can be reused. Exiting
the scope releases its cache, including on exceptions. Nested scenarios compose
in entry order. The same scenario can be reused with another context.
Quote identifiers select calibrated market inputs. For explicit zero curves,
`FUNDINGSPREAD` selects every spread pillar and
`FUNDINGSPREAD_2031-02-24` selects one dated pillar. These are the same labels
returned by `Request.Sensitivities`. Multiplying their derivatives by `0.0001`
gives the approximate EUR value change for a +1 bp funding-spread move.

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
csa_policy = qs.SingleCurveCSADiscountPolicy.from_dict({"collateral_currency": "USD", "collateral_index": "SOFR"})
fx = qs.FxStore.from_dict([{"base": "EUR", "quote": "USD", "rate": 1.1}])
```

`from_dict_list` is available on `CurveConfiguration`,
`VolatilitySurfaceConfiguration`, `VolatilityCubeConfiguration`,
`PricerConfiguration`, and `SimulationConfiguration`. Like `from_json`, it
accepts a list, one configuration dict, or a single-key object wrapping a list,
such as `{"curve_specs": [...]}`. `from_dict` on those classes still returns
one configuration. `QuoteStore`, `FixingStore`, `FxStore`, and
`SingleCurveCSADiscountPolicy` keep their existing `from_dict` methods.

Market configs use the native Rust serde schemas. For product market-index
fields, rate indices are strings (`"SOFR"`); other variants use native tagged
dicts such as `{"Equity": "AAPL"}`, `{"Credit": "CORPORATE-A"}`,
`{"FxPair": ["EUR", "USD"]}`, and `{"Collateral": ["EUR", "USD"]}`.
Strikes accept a float, `"ATM"`, or a native tagged dict such as
`{"Absolute": 100.0}`.

### Bond pricing, Z-spread, and OAS

Straight fixed-rate bonds use `BondPricer`. Supply the required `pricing_anchor`
when constructing `qs.Trade(bond, ref, pricing_anchor=anchor)`. Use `trade.with_pricing_anchor(anchor)`
to create a copy with another quote; `PricingContext.evaluate(trade, requests)`
reads the quote from that trade.

`FixedRateBond.discount_index` is optional. Omit it to use the context's
fixed-income discount policy, which maps the base currency to the base index.
An explicit index takes precedence when `prefer_instrument_index=True`.
Pass `fixed_income_policy=qs.FixedIncomeDiscountPolicy({"USD": "SOFR"},
prefer_instrument_index=False)` to select the policy curve explicitly.
An unmapped currency produces an error rather than silently selecting a curve.

```python
anchor = qs.BondPricingAnchor.clean_price(99.25)
result = ctx.evaluate(
    qs.Trade(bond, ref, pricing_anchor=anchor),
    [qs.Request.Value, qs.Request.ModifiedDuration,
     qs.Request.ZSpread, qs.Request.Sensitivities],
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
        qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.045)),
        qs.OptionEmbeddedCoupon(
            index=qs.MarketIndex.SOFR,
            payoff=(qs.PayoffOps.Index + 0.006).max(0.0).min(0.06),
        ),
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

ctx = qs.PricingContext(quotes=quotes, curves=curves,
                       pricer_configurations=pricers,
                       csa_policy=csa_policy)

with ctx:
    result = ctx.evaluate(
        qs.Trade(callable_bond, ref, pricing_anchor=qs.BondPricingAnchor.dirty_price(101.10)),
        [qs.Request.Value, qs.Request.OAS, qs.Request.Sensitivities],
    )
    print(result.oas)
    print(result.sensitivities)  # curve rows plus <identifier>.OAS
```

The constructor generates coupon dates from `payment_frequency`. Both bond
wrappers accept `calendar`, `business_day_convention`, `date_generation_rule`,
`end_of_month`, and `first_coupon_date`. Stepped bonds additionally accept
`next_to_last_coupon_date` and `termination_date_convention` (defaulting to the
business-day convention). Calendar names or `qs.Calendar` objects and string or
enum conventions are accepted. These fields survive dictionary round trips.

`side`, `pricing_anchor`, `settlement_date`, and `spread_day_counter` belong to
the trade. The Rust instrument builder always builds the contractual cashflows
in the receiving direction; valuation applies trade side once. Defaults for
settlement and spread day count remain the context date and curve convention.

`csa_policy` configures collateral remuneration for curve construction and
native derivative pricing. A foreign-currency leg resolves the corresponding
collateral-adjusted curve. `CsaTerms` remains the netting/XVA input, including
credit, recovery and funding terms; it is distinct from the context's CSA policy.

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
        qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.045)).to_dict(),
        qs.OptionEmbeddedCoupon(
            index=qs.MarketIndex.SOFR,
            payoff=(qs.PayoffOps.Index + 0.006).max(0.0).min(0.06),
        ).to_dict(),
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

`FixedRateCoupon`, `FloatingRateCoupon`, and `OptionEmbeddedCoupon` wrap the
corresponding native cashflows. Each class provides `from_dict` and `to_dict`.
Serialized tags use those class names, and optional coupons preserve the full
`PayoffOps` expression tree. Legacy compact `Fixed`/`Floating` input remains
readable.
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

With no call entries, use an explicit zero Z-spread anchor on its trade.

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
ctx = qs.PricingContext(quotes=quotes, curves=curves, csa_policy=csa_policy)

with ctx:
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

See [examples/valuation/bonds/stepped_coupon_bond.ipynb](examples/valuation/bonds/stepped_coupon_bond.ipynb)
for tree/Fourier pricing and [examples/scripting/scripted_cashflows.ipynb](examples/scripting/scripted_cashflows.ipynb)
for the native-versus-scripted comparison.

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

Python registrations are resolved by exact trade type first and then by compatible
base type. They take precedence over built-in dispatch, so a trade holding a
native instrument can use a custom pricer without modifying the Rust binding. Use
`ctx.unregister_pricer(TradeType)` to restore built-in dispatch.
Native instruments and `Trade` are subclassable. An unregistered `Trade`
subclass holding a native instrument keeps native pricing. Register the trade
subclass, not the instrument class, to select a Python implementation.

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
funding_curve = qs.TenorBasedZeroRateTermStructure(
    ref, ["0D", "1Y", "3Y"], [0.004, 0.005, 0.006],
    enable_extrapolation=True,
)
csa_b = qs.CsaTerms(collateral_index="SOFR", collateral_currency="USD",
                    credit_spread=0.015, recovery=0.40,
                    funding_spread_curve=funding_curve,
                    variation_margin=True, counterparty_threshold=1000.0,
                    own_threshold=1000.0, minimum_transfer_amount=500.0)

ctx = qs.PricingContext(quotes=quotes, curves=curves, fx=fx, csa_policy=csa_policy)

with ctx:
    result = ctx.run_xva(config, netting_sets=[
        qs.NettingSet("clientA", [qs.Trade(swap, ref)], csa_a),
        qs.NettingSet("clientB", [qs.Trade(xccy, ref)], csa_b),
    ])
    print(result.xva_values)      # DataFrame: netting_set / measure / value
    print(result.sensitivities)   # DataFrame: parameter / value
    for p in result.exposures:    # per-netting-set EPE/ENE/EE profiles
        print(p.netting_set)
        print(p.to_dataframe())   # DataFrame: date / epe / ene / ee
```

See [examples/cva/xva.ipynb](examples/cva/xva.ipynb) for a complete netting comparison
and [the examples guide](examples/README.md) for the full learning sequence.

`ZeroRateTermStructure(ref, dates, rates)` and
`TenorBasedZeroRateTermStructure(ref, tenors, rates)` are reusable curve objects.
Both expose `zero_rate`, `discount_factor`, and `forward_rate`, plus their
time-based variants and a `to_dict()` export of the equivalent dated curve.
The dated class also supports `from_dict()` and `from_json()`.
Keyword options select `day_counter`, `compounding`, `frequency`, `interpolator`,
and `enable_extrapolation`. Defaults are Actual365, Continuous, Annual, Linear,
and disabled extrapolation.

`CsaTerms.funding_spread_curve` accepts either class (or a dated curve dictionary)
and returns a `ZeroRateTermStructure`. Its rates are annualized spread quotes;
FVA uses the curve's reference date, day counter, interpolation, and extrapolation
settings. `funding_index` can reuse a bootstrapped funding curve, with the
explicit spread curve as an optional additive overlay. There is no separate
`FundingSpreadCurve` type or list-of-pairs constructor. Serialized spread curves
now use the zero-curve schema (`reference_date`, `dates`, `rates`, `rate_definition`,
`interpolator`, `enable_extrapolation`), replacing the old `dates`/`spreads` schema.

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
- `Calendar.TARGET`, `Calendar.Japan`, `Calendar.WeekendsOnly`, and other native
  calendar variants — `is_business_day`, `adjust`, `advance`,
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
- `SingleCurveCSADiscountPolicy(collateral_currency, collateral_index)` — collateral policy for curve construction and derivative valuation
- `FixedIncomeDiscountPolicy(risk_free_indices, prefer_instrument_index=True)` — per-currency bond discount curves

Pricing:

- `Shock(target=..., value=..., shock_type=ScenarioType.Absolute)` — typed, immutable market shock
- `Scenario(shocks=[Shock(...)])` — temporary shocks in `with context, scenario:`
- `PricingContext(quotes, curves, fixings=None, fx=None, volatility_surfaces=None, volatility_cubes=None, pricer_configurations=None, simulations=None, csa_policy=None, fixed_income_policy=None)` — context manager
- inputs: `ctx.reference_date` / `ctx.evaluation_date`, `ctx.quotes`,
  `ctx.fixings`, `ctx.fx`, `ctx.base_currency`, `ctx.base_index`,
  `ctx.curve_configurations`,
  `ctx.volatility_surface_configurations`, `ctx.volatility_cube_configurations`,
  `ctx.simulation_configurations`
- constructed outputs (inside the `with` block, or after an explicit
  `initialize()` when using the context without one):
  - `ctx.discount_curves()` / `ctx.curve(index)` → constructed `DiscountCurve` objects
    (`nodes()`, `pillars()`, `discount_factor(date)`, `forward_rate(...)`)
  - `ctx.curves = new_curves` → replace curve configurations and rebuild initialized markets
  - `ctx.volatility_surfaces()` / `ctx.volatility_surface(index)` → `VolatilitySurface`
  - `ctx.volatility_cubes()` / `ctx.volatility_cube(index)` → `VolatilityCube`
  - `ctx.simulations()` / `ctx.simulation(index)` → `Simulation`
    (`dates()`, `n_paths`, `dt`, `paths()` and `calibration_diagnostics()` DataFrames)
- `Swap`, `CrossCurrencySwap`, `FixedRateBond`, `SteppedCouponBond` — instrument
    specifications with typed getters; callable stepped bonds use native coupon classes and
  explicit `CallScheduleEntry(date, strike)` terms
- `ctx.evaluate(trade, requests)` → `EvaluationResults` (`price`, `yield_to_maturity`, `modified_duration`, `z_spread`, `oas`, `fair_rate`, `sensitivities` DataFrame, `cashflows` DataFrame)

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

### Bond API migration

Bond `Trade` constructors require `pricing_anchor=qs.BondPricingAnchor.z_spread(0.0)`
for an explicitly unspread straight bond, or `qs.BondPricingAnchor.OAS(0.0)`
for an issuer-call bond. Pass a price anchor to imply the appropriate spread.
Remove position metadata from product constructors and wrap each product in
`qs.Trade(instrument, trade_date, side, pricing_anchor=...)` before valuation.
The `bond_anchor` evaluation argument has been removed. Replace the former
`discounting`/`DiscountingConfig` context input with `csa_policy` and
`SingleCurveCSADiscountPolicy(collateral_currency=..., collateral_index=...)`.

### Regional indices, calendars, and snapshots

Rate indices include Fed Funds, CDI, DESTR, KOFR, ZARONIA, EURIBOR 1W,
BBSW and BKBM (1/2/3/4/6 months), NIBOR (1/3/6/12 months), and SHIBOR
(1 day, 1/2 weeks, 1/3/6/12 months). Use typed constants such as
`qs.MarketIndex.FedFunds` and `qs.MarketIndex.NIBOR3m`, or their exact string
names in configuration dictionaries and quote identifiers.

Index metadata supplies currency, day count, reset frequency, fixing lag, and
fixing calendar. Instrument schedule conventions remain explicit; BBSW's
half-month date-roll rule is not inferred automatically.

Calendars include Australia, Canada, Switzerland, New Zealand, Norway, Sweden,
Denmark, South Korea, South Africa, and China. Select market variants explicitly:

```python
canada = qs.Calendar.Canada(market=qs.CanadaMarket.Settlement)
china_ib = qs.Calendar.China(market=qs.ChinaMarket.IB)
switzerland = qs.Calendar.Switzerland
```

`SnapshotSelector` chooses a complete historical market by timestamp and quote
side. Request an index from the resulting context; the selector itself does not
filter indices or choose a calendar:

```python
archive = qs.MarketSnapshotStore.from_json("markets.json")
selector = qs.SnapshotSelector(
    qs.DateTime(2026, 1, 15, 10),
    rule=qs.SnapshotSelectionRule.Previous,
    level=qs.Level.Mid,
)
selected = archive.select(selector)
with archive.pricing_context(selector) as historical:
    curve = historical.curve(qs.MarketIndex.NIBOR3m)
    discount = curve.discount_factor("2027-01-15")
```

The archive must contain a construction configuration for the requested index
and any required discount/collateral curves. New identifiers and calendar names
in OIS quote conventions survive snapshot JSON round-trips. Historical fixings
are supplied separately through `pricing_context(selector, fixings=...)`.

Calendar rules follow QuantLib commit `5f4811e7e03b`. China tables extend through
2026; South Korean lunar dates through 2050; New Zealand Matariki dates through
2052. New Zealand defaults to Wellington for QuantLib parity; regional variants
are explicit. Dates beyond finite tables require updated holiday announcements.
QuantLib calendar notices are included in `quantsupport/licenses/QuantLib.txt`.
