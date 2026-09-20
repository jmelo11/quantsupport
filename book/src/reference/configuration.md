# Configuration Files

Configuration files separate changing market and model choices from the Rust code that executes them. QuantSupport derives `serde::Deserialize` for its configuration types, so applications can construct the same values in code or load them from JSON. This chapter introduces each file by its role in the market lifecycle and explains how its fields connect to the objects built by `PricingContext` and `XvaEngine`.

Across all files, dates use `YYYY-MM-DD`, periods use forms such as `1W`, `3M`, and `5Y`, and enums use their Rust variant names. Structured enum variants use the standard externally tagged Serde representation shown below.

## `quotes.json`

The quote file establishes the market reference date and the observable values available on that date. Each identifier encodes an instrument and its coordinates, as explained in [Market Data](../concepts/market-data.md). A quote may provide mid, bid, ask, or any useful combination of those levels:

```json
{
  "reference_date": "2026-02-24",
  "quotes": [
    { "identifier": "OIS_USD_SOFR_1Y", "mid": 0.0422,
      "bid": 0.0421, "ask": 0.0423 },
    { "identifier": "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black",
      "mid": 0.32 },
    { "identifier": "Swaption_CLP_ICP_1Y_2Y_Absolute_0.045_Black",
      "mid": 0.26 },
    { "identifier": "Cds_CLIENT_A_USD_5Y", "mid": 0.01 }
  ]
}
```

Deserialize this shape as `QuoteStoreRecords`, then convert it with `QuoteStore::try_from`. Example packages sometimes use an equivalent local loader. Builders read values at `Level::Bid`, `Level::Mid`, or `Level::Ask`, and a missing requested level produces an error.

## `fixings.json`

Fixings record historical index observations used by coupons whose fixing date has passed. The top-level keys are `MarketIndex` names and each value is a dated rate series:

```json
{
  "SOFR": [{ "date": "2025-05-12", "rate": 0.0428 }],
  "ICP": [{ "date": "2025-05-12", "rate": 0.0575 }]
}
```

Deserialize this file as `FixingStore`. Applications can call `fill_missing_fixings` under an explicit interpolation rule when their data policy permits gap filling.

## `curve_specs.json` — `Vec<CurveConfiguration>`

Curve specifications assign quote identifiers to rate-curve identities. The file format used by the examples wraps the vector in `curve_specs`. Currency and instrument conventions come from the structured quote identifiers, and interpolation choices belong to each curve configuration:

```json
{
  "curve_specs": [
    {
      "market_index": "SOFR",
      "day_counter": "Actual360",
      "interpolator": "LogLinear",
      "quotes": [
        "FixedRateDeposit_USD_SOFR_1D",
        "OIS_USD_SOFR_1Y",
        "OIS_USD_SOFR_5Y"
      ]
    },
    {
      "market_index": "TermSOFR3m",
      "quotes": ["BasisSwap_USD_SOFR_TermSOFR3m_1Y"]
    },
    {
      "market_index": { "Collateral": ["CLP", "USD"] },
      "quotes": ["FixFloatCrossCurrencySwap_CLP_SOFR_USD_1Y"]
    }
  ]
}
```

SOFR must be available before the dependent Term SOFR and collateral curves can be solved. The multi-curve bootstrapper derives that order from the instruments. [Curve Bootstrapping](../curves/bootstrapping.md) gives the full field list, defaults, and failure behavior.

Credit curves use `CreditCurveConfiguration` because CDS calibration also needs recovery, premium frequency, currency, and a discount index. A representative configuration is:

```json
{
  "market_index": { "Credit": "CLIENT_A" },
  "currency": "USD",
  "discount_index": "SOFR",
  "recovery": 0.4,
  "premium_frequency": "Quarterly",
  "quotes": ["Cds_CLIENT_A_USD_1Y", "Cds_CLIENT_A_USD_5Y"]
}
```

`CreditCurveBootstrapper` consumes a vector of these configurations after the rate curves have been constructed.

## `vol_specs.json`

Volatility specifications define the quote membership and conventions of surfaces and cubes. The wrapper used by the examples keeps the two collections together:

```json
{
  "volatility_surfaces": [
    {
      "market_index": "SOFR",
      "volatility_type": "Black",
      "smile_type": "Strike",
      "quotes": ["CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.035_Straddle_Black"]
    }
  ],
  "volatility_cubes": [
    {
      "market_index": "ICP",
      "volatility_type": "Black",
      "smile_type": "Strike",
      "quotes": ["Swaption_CLP_ICP_1Y_2Y_Absolute_0.045_Black"]
    }
  ]
}
```

The surface quote encodes expiry, index tenor, and strike. The cube quote adds the underlying swap tenor. Builders validate these coordinates and preserve the identifiers for interpolation risk and model calibration.

## `ModelCalibrationConfiguration`

Model calibration is configured inside the model's `parameter_source` section.
`vol_specs.json` defines the quote membership of each volatility market. The
model's `calibration_basket` selects instruments from that constructed market.
This fragment selects ATM caplets at three expiries with a three-month index
tenor:

```json
{
  "source": { "Surface": { "market_index": "SOFR" } },
  "calibration_basket": {
    "expiries": ["1Y", "2Y", "5Y"],
    "tenors": ["3M"],
    "strike": "Atm"
  }
}
```

Omitting `expiries` or `tenors` selects every available value on that axis. Every explicitly supplied value must occur in the source quote identifiers and participate in an instrument that also satisfies the other axis. Resolution returns `NotFoundErr` before calibration when an expiry or tenor is absent. On a surface, tenor means the floating-rate index tenor. On a cube, tenor means the underlying swap tenor.

The source object retains the option identifiers used for basket selection. Its AD pillars provide the labels used in sensitivity reports. An interpolated quote grid commonly uses the same identifiers for both roles. A parametrized volatility representation can retain option identifiers for calibration and expose fitted parameters as its AD pillars.

## `simulation.json` — `SimulationConfiguration`

A simulation configuration joins one market index to a dynamics model and a reproducible date grid. This example uses fixed Hull-White volatility and therefore requires no volatility surface during construction:

```json
{
  "market_index": "SOFR",
  "model": {
    "HullWhite": {
      "alpha": 0.1,
      "parameter_source": { "Fixed": { "sigma": 0.01 } }
    }
  },
  "n_paths": 1000,
  "seed": 42,
  "horizon": "5Y",
  "frequency": "Monthly",
  "day_counter": "Actual365"
}
```

Rate-model variants are `HullWhite { alpha, parameter_source }` and
`Lgm { lambda, parameter_source }`, where `parameter_source` accepts
`Fixed { sigma }` or `Calibrated { source, calibration_basket }`.
`BrownianMotion { parameter_source, dividend_rate? }` accepts
`Fixed { volatility }` or
`Calibrated { source, calibration_basket }`.

For rate models, sigma is an absolute short-rate volatility expressed in rate units per square-root year. A value of `0.01` means 100 basis points per square-root year. Calibrated configurations derive a model-specific parameter schedule from the selected option instruments.

## `xva_config.json` — `XvaEngineConfig`

XVA configuration describes all rate and FX factors simulated together. A driver entry reuses another rate factor for an additional curve identity. The example below gives Term SOFR the SOFR driver and adds a CLP FX factor:

```json
{
  "model_configs": [
    { "market_index": "SOFR", "lambda": 0.05,
      "parameter_source": { "Fixed": { "sigma": 0.01 } } },
    { "market_index": "TermSOFR3m", "driver": "SOFR" }
  ],
  "fx_configs": [{ "foreign_currency": "CLP", "fx_vol": 0.12, "rho": 0.0 }],
  "n_paths": 2000,
  "seed": 42,
  "frequency": "Monthly"
}
```

Every discount or projection index selected by claims and CSA policies needs a model configuration or a valid driver mapping. The path count must be even because the LGM market model uses antithetic pairing. The seed fixes the Sobol scrambling, and frequency defines the exposure dates.

## `csa_terms.json` — `CsaTerms`

CSA terms describe collateral discounting and the credit and funding inputs for one netting set. This example uses SOFR collateral, a credit curve, and a Term SOFR funding curve with an explicit spread term structure:

```json
{
  "collateral_index": "SOFR",
  "collateral_currency": "USD",
  "credit_spread": 0.01,
  "recovery": 0.4,
  "funding_spread": 0.0,
  "funding_index": "TermSOFR3m",
  "funding_spread_curve": {
    "dates": ["2026-11-11", "2028-11-11"],
    "spreads": [0.004, 0.005]
  },
  "credit_index": { "Credit": "CLIENT_A" }
}
```

When `credit_index` is present, CVA reads survival probabilities from that constructed credit curve. `funding_index` derives funding spreads relative to the system curve, and `funding_spread_curve` supplies dated spread values. Flat `credit_spread` and `funding_spread` fields support configurations that use scalar assumptions.

## Scripted products — `Vec<CodedEvent>`

A scripted-product file stores the event date and source program for each step in the payoff timeline. Variable state carries forward in array order. The following two events observe a rate and later pay a caplet-style amount:

```json
[
  {
    "event_date": "2026-06-15",
    "script": "libor = RateIndex(\"SOFR\", \"2026-06-15\", \"2026-12-15\");"
  },
  {
    "event_date": "2026-12-15",
    "script": "coupon = 0; coupon pays max(libor - 0.03, 0) * 0.5;"
  }
]
```

Deserialize the array as `Vec<CodedEvent>` and convert it with `EventStream::try_from`. [Events and Products](../scripting/events-products.md) explains ordering, payment discovery, and validation.

## `MarketIndex` spelling

Plain indices are bare strings such as `"SOFR"`, `"ICP"`, `"TermSOFR3m"`, and `"ESTR"`. Structured variants are tagged objects. `{"Collateral": ["CLP", "USD"]}` identifies a CLP curve under USD collateral, `{"Credit": "NAME"}` identifies a credit name, and `{"Equity": "AAPL"}` identifies an equity factor. The same spelling must be used in quotes, curve configurations, model configurations, and trades so their identities resolve consistently.

## What to remember

The files follow the order of market construction. Quotes and fixings provide observations, curve and volatility specifications construct queryable markets, model parameter sources resolve dynamics, and XVA plus CSA files define the exposure calculation. Stable `MarketIndex` values and quote identifiers connect every stage. Loading each file into its named Rust type keeps validation close to the component that understands its meaning.
