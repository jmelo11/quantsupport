# Serializable configuration

Configuration holds the market data and build rules needed to rebuild a
market. The library types use Serde. Your application decides where files live
and how to load and save them.

## Observations and curve membership

The quote file has one reference date and a list of quotes, each with an
identifier and one or more levels:

```json
{
  "reference_date": "2026-02-24",
  "quotes": [
    {"identifier": "FixedRateDeposit_USD_SOFR_1D", "mid": 0.04},
    {"identifier": "OIS_USD_SOFR_1Y", "mid": 0.042, "bid": 0.0419, "ask": 0.0421}
  ]
}
```

Deserialize it as `QuoteStoreRecords` and convert it with
`QuoteStore::try_from`. Curve building and calibration use the selected
`Level` throughout.

Fixings are stored in a separate JSON file that maps index names to dated
rates. Each fixing keeps its own date; the reference date of the quote file is
not used for fixings.

The examples often wrap a list of curve configurations in
`{"curve_specs":[...]}`. That wrapper is part of the example loader. A single
curve configuration looks like this:

```json
{
  "market_index": "SOFR",
  "day_counter": "Actual360",
  "interpolator": "LogLinear",
  "enable_extrapolation": true,
  "quotes": ["FixedRateDeposit_USD_SOFR_1D", "OIS_USD_SOFR_1Y"]
}
```

Optional fields that are left out take their default values. The quote
identifiers decide whether the curve is calibrated to instrument prices or
built directly from zero rates or discount factors. All numbers go in the
quote store, not in the curve configuration:

```json
[
  {
    "market_index": {"Other": "FUNDINGSPREAD"},
    "day_counter": "Actual360",
    "interpolator": "Linear",
    "quotes": ["ZeroRate_EUR_OTHER:FUNDINGSPREAD_0D", "ZeroRate_EUR_OTHER:FUNDINGSPREAD_1Y", "ZeroRate_EUR_OTHER:FUNDINGSPREAD_5Y"]
  },
  {
    "market_index": {"Other": "COSTOFFUNDS"},
    "base_index": "EURIBOR12m",
    "spread_index": {"Other": "FUNDINGSPREAD"}
  }
]
```

Notes on this example:

- The quote store holds the funding spread values, such as `0.005`, `0.0075`,
  and `0.01`, under those identifiers.
- Pillars can be tenors or ISO dates.
- The benchmark curve (`EURIBOR12m`) must also be configured.
- The components of a composite must have the same reference date and day
  counter.
- The order of the list does not matter; dependencies are resolved
  automatically.
- A composite curve cannot also list quote identifiers.

Credit configurations also need a currency, discount index, recovery rate, and
the list of CDS quotes. They build survival probabilities, not discount
factors.

Volatility configurations set the market index, Black or Normal convention,
smile coordinates, source quotes, and extrapolation policy. `Reject` is the
default. Use `Flat` only if you want flat extrapolation.

## Typed models and calibration sources

Models are written as tagged variants, not as free-form strings. For example,
fixed Hull–White parameters are:

```json
{
  "HullWhite": {
    "alpha": 0.1,
    "parameter_source": {"Fixed": {"sigma": 0.01}}
  }
}
```

- LGM uses `lambda` and a Gaussian sigma.
- Brownian motion uses a lognormal `volatility` and an optional dividend rate.

A calibrated parameter source replaces `Fixed` with a source and a calibration
basket:

```json
{
  "Calibrated": {
    "source": {"Surface": {"market_index": "SOFR"}},
    "calibration_basket": {
      "expiries": ["1Y", "2Y"],
      "tenors": ["3M"],
      "strike": "Atm"
    }
  }
}
```

The source volatility configuration lists the quotes. The basket selects
contracts from that list. Asking for coordinates that are not there is an
error; the library does not create interpolated calibration contracts.

`PricerConfiguration` selects an instrument family, a pricing method, and model
assignments.

- Swaptions can use `HullWhiteClosedForm`.
- Callable bonds use `TreeBased` or `BackwardFourierEvolution`, with their
  numerical grids.

When you call the library directly, you create the pricer yourself. A
simulation configuration sets the index, model, number of paths, seed, horizon,
frequency, and time basis.

## XVA, CSA and payoff inputs

- `XvaEngineConfig` holds the rate models and driver mappings, the FX models,
  and the path and grid settings.
- `CsaTerms` holds the collateral, margin, credit, and funding terms of a legal
  agreement. Set these separately from the rate model.

See [Netting and collateral](../xva/netting-csa.md) for the order in which
fields are used and for margin timing limits.

A scripted product is a list of events, each with a date and a script:

```json
[
  {
    "event_date": "2027-03-09",
    "script": "deal = 0; deal pays 1000 on \"2027-06-09\" in \"USD\";"
  }
]
```

Deserialize it as `Vec<CodedEvent>` and convert it to an `EventStream`. Scripts
from JSON go through the same language and date checks as any other script.

Formats used by all these types:

- Dates use `YYYY-MM-DD`.
- Periods use forms such as `3M` and `5Y`.
- Enum values use the exact variant names.
- Standard `MarketIndex` values are strings. Structured ones include
  `{"Collateral":["CLP","USD"]}`, `{"Credit":"NAME"}`, and `{"Equity":"AAPL"}`.

Historical snapshots store the complete build inputs in these formats. They do
not store AD state, fixing histories, or anything your application registers
at run time. To reproduce a valuation, save those separately.
