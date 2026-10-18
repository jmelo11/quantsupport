# Working in Python

The Python package provides access to QuantSupport's native instruments,
market construction, valuation and risk calculations. Build a contract and
trade, configure its market, then evaluate the position inside a pricing
context. Results are available as scalar properties and pandas tables.

Classes can be imported from domain packages or from the package root:

```python
import quantsupport as qs
from quantsupport.pricing import PricingContext, Trade

assert Trade is qs.Trade
```

## A complete market and valuation

An explicit zero-rate configuration makes a small example independent of
external data files. Its input rates are continuously compounded annual
decimals. Configuration remains serializable; the live curve is constructed
when pricing requests it.

```python
import quantsupport as qs

quotes = qs.QuoteStore.from_dict({
    "reference_date": "2026-01-02",
    "quotes": [],
})
curve = qs.CurveConfiguration(
    market_index=qs.MarketIndex.SOFR,
    construction=qs.CurveConstruction.zero_rate(
        pillars=["1Y", "5Y", "10Y"],
        rates=[0.03, 0.03, 0.03],
        compounding=qs.Compounding.Continuous,
    ),
    enable_extrapolation=True,
)
swap = qs.Swap(
    "USD_IRS_5Y", "2026-01-02", "2031-01-02",
    1_000_000.0, 0.035, "USD", "SOFR",
)
position = qs.Trade(swap, "2026-01-02", qs.Side.LongReceive)
context = qs.PricingContext(quotes=quotes, curves=[curve])
with context:
    result = context.evaluate(
        position, [qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities]
    )
    print(result.price)
    print(result.cashflows)
    print(result.sensitivities)
```

An instrument contains the contract; `Trade` supplies the position date,
direction and supported notional overrides. Bond trades additionally require
a `BondPricingAnchor`. Native dispatch chooses the compatible pricer, using
typed context-level pricer configurations for model-dependent choices.
Python-defined pricers can also be registered explicitly.

## Context lifetime and mutability

Entering `with context:` validates configuration and activates the pricing
scope. It does not eagerly build the entire market. Requests construct and
cache the dependencies they consume. Explicit `context.initialize()` performs
eager construction when that is the intended workflow.

Exiting the outer scope releases the context's constructed market and its AD
state, including on exceptions. Keep returned results for reporting, not live
market views whose validity depends on that scope. Inputs have explicit
setters: assigning a replacement collection invalidates dependent state.
Collection properties return copies, so edit-and-reassign rather than assume
an in-place change has reached the native context.

Contract and result properties are generally read-only. Operations such as
evaluation, curve queries and serialization are methods. Date-like inputs
accept the forms documented for their signatures, commonly `qs.Date`,
`datetime.date` and ISO date strings. Enum strings are conveniences at specific
conversion boundaries, not permission to invent new market identities.

## Scenarios and historical markets

Temporary shocks belong in an independent scenario scope:

```python
scenario = qs.Scenario(shocks=[
    qs.Shock(target="SOFR", value=0.0001, shock_type=qs.ScenarioType.Absolute),
])
with context, scenario:
    stressed = context.evaluate(position, [qs.Request.Value])
```

The base inputs remain unchanged. Scenarios compose in entry order and rebuild
only requested affected dependencies. To reprice against an observed historical
market, select a complete snapshot from a market archive:

```python
archive = qs.MarketSnapshotStore()
archive.insert(qs.MarketSnapshot.from_context(
    qs.DateTime(2026, 1, 2, 16, 30), context,
))
selector = qs.SnapshotSelector(
    qs.DateTime(2026, 1, 2, 17),
    rule=qs.SnapshotSelectionRule.Previous,
    level=qs.Level.Mid,
)
with archive.pricing_context(selector) as historical:
    print(historical.snapshot_timestamp, historical.evaluation_date)
    print(historical.evaluate(position, [qs.Request.Value]).price)
```

The observation instant and market business date are distinct. Selection does
not roll curves to the requested date. Archives retain construction inputs,
not runtime AD graphs, fixing publication vintages, custom Python pricers or
Python-only policy registrations. Supply those separately as appropriate.

## Scripts and portfolio workflows

`context.evaluate_script` evaluates a `ScriptedProduct` or corresponding
dictionary through native rate models and returns averaged script variables.
Call it inside an entered context or after explicit initialization, with a
positive path count. Required discount and forward indices need configured
rate models; absent models do not imply zero-volatility dynamics.
Its `result_variable` identifies the target for derivative/control processing;
it does not filter the returned dictionary. Models are resolved from pricer
configuration, then simulation configuration, with script model overrides.
Hull–White settings map to the equivalent LGM rate representation.

The discount index needs a configured model and constructed curves; a script
outside the base currency needs an explicit discount index. This service does
not expose arbitrary FX/equity factors, custom correlations, raw paths, or
scripted cashflow/sensitivity tables. Native-trade exposure/XVA is available
through the separate context XVA workflow. Use the Rust interfaces when the
workflow needs those additional facilities.

Native failures use `QuantSupportError`; invalid Python inputs can also raise
`TypeError` or `ValueError`. The [executable
notebooks](../reference/examples.md) develop complete applications. The optional
[Python
guide](https://jmelo11.github.io/quantsupport/api/python/guide/index.html)
develops binding-specific workflows; the [Python
reference](https://jmelo11.github.io/quantsupport/api/autoapi/quantsupport/index.html)
provides declaration-level details.