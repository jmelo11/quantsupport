# Configuration Files

All configuration structs derive `serde::Deserialize`, so the same shapes work from JSON files or built in code. Dates are `YYYY-MM-DD`, periods are `1W`, `3M`, `5Y`, enums use their variant names.

## `quotes.json`

Array of quotes; identifiers encode instrument, currency, index and tenor (see [Market Data](../concepts/market-data.md)).

```json
[
  { "identifier": "OIS_USD_SOFR_1Y", "bid": 0.0421, "ask": 0.0423 },
  {
    "identifier": "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black",
    "bid": 0.31,
    "ask": 0.33
  },
  {
    "identifier": "Swaption_CLP_ICP_1Y_2Y_Absolute_0.045_Black",
    "bid": 0.25,
    "ask": 0.27
  },
  { "identifier": "Cds_USD_CLIENT_A_5Y", "bid": 0.0095, "ask": 0.0105 }
]
```

Loaded with `QuoteStore::from_json` / `serde_json`; values are read at `Level::Bid | Mid | Ask`.

## `fixings.json`

```json
{
  "SOFR": [{ "date": "2025-05-12", "rate": 0.0428 }],
  "ICP": [{ "date": "2025-05-12", "rate": 0.0575 }]
}
```

## `curve_specs.json` — `Vec<CurveConfiguration>`

```json
[
  {
    "market_index": "SOFR",
    "currency": "USD",
    "day_counter": "Actual360",
    "interpolator": "LogLinear",
    "quotes": ["Deposit_USD_SOFR_1W", "OIS_USD_SOFR_1Y", "OIS_USD_SOFR_5Y"]
  },
  {
    "market_index": "TermSOFR3m",
    "currency": "USD",
    "quotes": ["BasisSwap_USD_SOFR_TermSOFR3m_1Y"]
  },
  {
    "market_index": { "Collateral": ["CLP", "USD"] },
    "currency": "CLP",
    "quotes": ["FloatFloatCrossCurrencySwap_USD_SOFR_ICP_CLP_1Y"]
  },
  {
    "market_index": { "Credit": "CLIENT_A" },
    "currency": "USD",
    "quotes": ["Cds_USD_CLIENT_A_1Y", "Cds_USD_CLIENT_A_5Y"]
  }
]
```

Full field list and defaults in [Curve Bootstrapping](../curves/bootstrapping.md).

## `vol_specs.json`

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

## `hw_calibration.json` — `ModelCalibrationConfiguration`

```json
{
  "source": { "Surface": { "market_index": "SOFR" } },
  "quote_ids": ["CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black"],
  "strike": "Atm",
  "alpha": 0.1
}
```

## `simulation.json` — `SimulationConfiguration`

```json
{
  "market_index": "SOFR",
  "model": {
    "HullWhite": {
      "alpha": 0.1,
      "volatility": { "Constant": { "value": 0.01 } }
    }
  },
  "n_paths": 1000,
  "seed": 42,
  "horizon": "5Y",
  "frequency": "Monthly",
  "day_counter": "Actual365"
}
```

Model variants: `HullWhite { alpha, volatility }`, `BrownianMotion { volatility, dividend_rate? }`, `Lgm { lambda, volatility }`. Volatility sources: `Constant { value }`, `Surface { market_index, key }`, `Cube { market_index, tenor, key }`, `Calibrated { ... }`.

## `xva_config.json` — `XvaEngineConfig`

```json
{
  "model_configs": [
    { "market_index": "SOFR", "lambda": 0.05, "sigma": 0.01 },
    { "market_index": "TermSOFR3m", "driver": "SOFR" }
  ],
  "fx_configs": [{ "foreign_currency": "CLP", "fx_vol": 0.12, "rho": 0.0 }],
  "n_paths": 2000,
  "seed": 42,
  "frequency": "Monthly"
}
```

## `csa_terms.json` — `CsaTerms`

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

## Scripted products — `Vec<CodedEvent>`

```json
[
  {
    "id": "fix1",
    "date": "2026-06-15",
    "code": "libor = RateIndex(SOFR, 2026-06-15, 2026-12-15)"
  },
  {
    "id": "pay1",
    "date": "2026-12-15",
    "code": "coupon pays max(libor - 0.03, 0) * 0.5"
  }
]
```

Grammar and validation rules in [Events and Products](../scripting/events-products.md).

## `MarketIndex` spelling

Plain indices are bare strings (`"SOFR"`, `"ICP"`, `"TermSOFR3m"`, `"ESTR"`); structured ones are objects: `{"Collateral": ["CLP", "USD"]}` (curve of CLP under USD collateral), `{"Credit": "NAME"}`, `{"Equity": "AAPL"}`.
