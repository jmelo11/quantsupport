# Python API

The Python package exposes typed market data and configuration objects while returning tabular results as pandas `DataFrame` values.

```python
import quantsupport as qs

quotes = qs.QuoteStore.from_json("quotes.json")
curves = qs.CurveConfiguration.from_json("curve_specs.json")
discounting = qs.DiscountingConfig(
    currency=qs.Currency.USD,
    index=qs.MarketIndex.SOFR,
)

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

with qs.PricingContext(
    quotes=quotes,
    curves=curves,
    discounting=discounting,
) as ctx:
    result = ctx.evaluate(
        swap,
        [qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities],
    )
    print(result.price)
    print(result.cashflows)
    print(result.sensitivities)
```

Entering the context initializes market objects and starts the AD tape. Exiting releases constructed data and rewinds the tape. Keep evaluations inside the `with` block.

Configuration classes support `from_dict` and `from_json`; enums also accept string names. The guided notebook at `bindings/python/examples/tour.ipynb` covers dates, curves, pricing, and XVA.
