# Python Getting Started

Install a wheel for Python 3.9 or later:

```sh
python -m pip install quantsupport
```

The package includes pandas for tabular results. Install editor and type-checker
support with `pip install "quantsupport[typing]"`.

Import by domain, or use the equivalent package-root names:

```python
import quantsupport as qs
from quantsupport.market import CurveConfiguration, QuoteStore
from quantsupport.pricing import PricingContext, Trade

assert qs.Trade is Trade
```

## Price a Swap

This example builds an explicit flat zero-rate curve and values a receive-fixed
swap. Rates are annual decimals, not percentages.

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
with qs.PricingContext(quotes=quotes, curves=[curve]) as context:
    result = context.evaluate(position, [qs.Request.Value, qs.Request.Cashflows])
    print(result.price)
    print(result.cashflows)
```

An instrument contains contractual terms; a `Trade` adds the position's date,
side, and optional notional overrides. Bond positions also require an explicit
`BondPricingAnchor`. Market objects are constructed on demand inside the context
manager. Retain results rather than live market views after leaving it.

## Properties, Methods, and Mutability

- Contract terms and evaluation results are read-only properties, such as
  `swap.maturity_date` and `result.price`. Collection properties return copies.
- Queries and operations are methods, such as `curve.discount_factor(date)`,
  `context.evaluate(trade, requests)`, and `curve.nodes()`.
- Context inputs have setters. Reassigning an input invalidates the dependent
  market; assign modified collections back to the context.
- Dates accept `Date`, `datetime.date`, or ISO strings where indicated by the
  signature. Financial enums also accept their documented string forms.
- `__all__` defines public imports in every domain package. Underscore names
  and the native extension module are implementation details.

The [Python API](api.md) distinguishes properties, class attributes,
constructors, and methods, with argument and return types.

## More Examples

The [notebook collection](https://github.com/jmelo11/quantsupport/tree/main/bindings/python/examples)
covers bootstrapping, bonds, callable coupons, loans, swaps, volatility,
sensitivity, scripting, Hull-White, and XVA.