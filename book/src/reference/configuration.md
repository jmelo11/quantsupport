# Serializable configuration

Configuration is the persistent description of a construction, not a dump of
runtime objects. Rust derives Serde support for native types; Python accepts
typed definitions and supported plain dictionaries; Excel constructors build
the corresponding native configurations. Applications remain responsible for
file I/O and example-specific envelopes.

## Observations and curve membership

The quote-record form combines one reference date with identifier/level
records:

```json
{
  "reference_date": "2026-02-24",
  "quotes": [
    {"identifier": "FixedRateDeposit_USD_SOFR_1D", "mid": 0.04},
    {"identifier": "OIS_USD_SOFR_1Y", "mid": 0.042, "bid": 0.0419, "ask": 0.0421}
  ]
}
```

Rust deserializes this as `QuoteStoreRecords` and converts with
`QuoteStore::try_from`. The selected `Level` is consumed consistently by
construction/calibration. Fixing JSON separately maps index names to dated
rate arrays. No file-level quote reference date substitutes for an observation's
actual fixing date.

Examples often wrap a vector of curve configurations in `{"curve_specs":[...]}`.
That wrapper belongs to their loader; the native configuration is one entry:

```json
{
  "market_index": "SOFR",
  "day_counter": "Actual360",
  "interpolator": "LogLinear",
  "enable_extrapolation": true,
  "quotes": ["FixedRateDeposit_USD_SOFR_1D", "OIS_USD_SOFR_1Y"]
}
```

Missing optional fields use native defaults. `construction` defaults to
bootstrapped construction. Explicit zero and composite curves instead leave
quote membership empty:

```json
[
  {
    "market_index": {"Other": "FUNDINGSPREAD"},
    "day_counter": "Actual360",
    "interpolator": "Linear",
    "construction": {
      "type": "ZeroRate",
      "pillars": ["0D", "1Y", "5Y"],
      "rates": [0.005, 0.0075, 0.01],
      "compounding": "Continuous",
      "frequency": "Annual"
    }
  },
  {
    "market_index": {"Other": "COSTOFFUNDS"},
    "construction": {
      "type": "Composite",
      "base_index": "EURIBOR12m",
      "spread_index": {"Other": "FUNDINGSPREAD"}
    }
  }
]
```

The benchmark curve must also be configured. Zero pillars can use tenors or
ISO dates; components must share their reference date and day counter.
Dependency resolution does not depend on list order.

Credit configurations additionally need currency, discount index, recovery and
CDS quote membership. They construct survival rather than discount factors.
Volatility configurations select market identity, Black/Normal convention,
smile coordinates, source quotes and extrapolation policy. `Reject` is the
default; use explicit `Flat` only when that continuation is intended.

## Typed models and calibration sources

Models use externally tagged variants rather than arbitrary strings. Fixed
Hull–White parameters, for example, are:

```json
{
  "HullWhite": {
    "alpha": 0.1,
    "parameter_source": {"Fixed": {"sigma": 0.01}}
  }
}
```

LGM uses `lambda` and Gaussian sigma. Brownian motion uses lognormal
`volatility` and optional dividend rate. A calibrated parameter source replaces
`Fixed` with a source and semantic basket:

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

The source volatility configuration owns quote membership. The basket selects
contracts already available there; absent requested coordinates are errors,
not instructions to fabricate an interpolated calibration contract.

`PricerConfiguration` selects an instrument family, implementation and model
assignments. Swaptions can select `HullWhiteClosedForm`; callable bonds select
`TreeBased` or `BackwardFourierEvolution` with numerical grids. Direct Rust
callers still construct the concrete pricer. A simulation configuration
separately supplies index, model, paths, seed, horizon, frequency and time basis.

## XVA, CSA and payoff inputs

`XvaEngineConfig` contains rate-model definitions/driver mappings, FX
definitions and path/grid controls. `CsaTerms` contains legal-group collateral,
margin, credit and funding assumptions. Those are different descriptions:
calibrating a rate model does not determine counterparty recovery or margin
thresholds. See [Netting and collateral](../xva/netting-csa.md) for field
precedence and timing limits.

A scripted-product input is an ordered array of date/source pairs:

```json
[
  {
    "event_date": "2027-03-09",
    "script": "deal = 0; deal pays 1000 on \"2027-06-09\" in \"USD\";"
  }
]
```

Deserialize it as `Vec<CodedEvent>` and parse an `EventStream`; source strings
do not bypass language or date validation.

Across these types, dates use `YYYY-MM-DD`, periods use forms such as `3M`
and `5Y`, and enum spelling follows canonical native variants. Standard
`MarketIndex` variants serialize as strings; structured forms include
`{"Collateral":["CLP","USD"]}`, `{"Credit":"NAME"}` and
`{"Equity":"AAPL"}`. A supported Python string conversion is not necessarily
an alias accepted by a serialized Rust enum.

Historical snapshot archives preserve complete native construction inputs
using this discipline. They exclude runtime AD state, fixing histories and
binding-only registrations. Reproducibility requires preserving those external
assumptions separately, not merely saving one JSON file.
