# Hull-White Model

One-factor Gaussian short-rate model, `src/models/hullwhite/`:

\\[
dr_t = \bigl(\theta(t) - \alpha r_t\bigr)dt + \sigma(t)\\,dW_t .
\\]

## API

```rust,ignore
pub struct HullWhite<'a, T: Scalar> {
    alpha: T,
    curve: &'a dyn InterestRatesTermStructure<T>,
    calibration_quality: Option<HullWhiteCalibrationQuality>,
    vol_func: Option<HullWhiteTimeDependentVolatility<T>>,
}

let mut hw = HullWhite::new(alpha, &sofr_curve).with_constant_volatility(0.01);
```

| Method                                                                   | Formula                                                                                                  |
| ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| `B(t, T)`                                                                | \\(\frac{1-e^{-\alpha(T-t)}}{\alpha}\\)                                                                  |
| `A(t, T, sigma, curve)`                                                  | \\(\frac{P(0,T)}{P(0,t)}\exp\\!\bigl(B\\,f(0,t) - \frac{\sigma^2}{4\alpha}(1-e^{-2\alpha t})B^2\bigr)\\) |
| `zcb_price(r_t, t, T, sigma, curve)`                                     | \\(A(t,T)\\,e^{-B(t,T)r_t}\\)                                                                            |
| `zcb_price_volatility(sigma, t, T)`                                      | \\(\sigma B(t,T)\sqrt{\frac{1-e^{-2\alpha t}}{2\alpha}}\\)                                               |
| `theta(t, sigma, curve)`                                                 | drift fitted to the initial curve                                                                        |
| `caplet_price(strike, t, S, sigma, curve)`                               | \\((1+\tau K)\\,\text{BondPut}(t,S,\frac1{1+\tau K})\\)                                                  |
| `swaption_price(strike, t_option, &[(pay_time, accrual)], sigma, curve)` | Jamshidian decomposition                                                                                 |
| `bond_put_price`, `bond_call_price`                                      | zero-coupon bond options                                                                                 |

The closed forms are shared with `ClosedFormHullWhiteCapletPricer`, `ClosedFormHullWhiteCapPricer` and `ClosedFormHullWhiteSwaptionPricer` ([Caps and Floors](../pricing/caps-floors.md), [Swaptions](../pricing/swaptions.md)).

## Calibration

```rust,ignore
hw.calibrate(&quote_ids, &quote_store, &curve, Level::Mid)?;
hw.calibrate_with_configuration(&config, &constructed_store, &quote_store, &curve, Level::Mid)?;
```

`ModelCalibrationConfiguration` (JSON in `examples/hullwhite/data/hw_calibration.json`):

```json
{
  "source": { "Surface": { "market_index": "SOFR" } },
  "quote_ids": [
    "CapletFloorlet_USD_SOFR_3M_3M_Absolute_0.045_Straddle_Black",
    "CapletFloorlet_USD_SOFR_3M_6M_Absolute_0.045_Straddle_Black",
    "CapletFloorlet_USD_SOFR_3M_1Y_Absolute_0.045_Straddle_Black"
  ],
  "strike": "Atm",
  "alpha": 0.1
}
```

Algorithm, per calibration quote in expiry order:

1. Parse the identifier to get expiry \\(T_i\\), index tenor and strike; `strike: "Atm"` replaces the quoted strike with the forward.
2. Read the Black (or Normal) vol from the surface/cube and compute the market caplet/swaption price.
3. Solve by bisection for the piecewise-constant \\(\sigma*i\\) on \\([T*{i-1},T_i]\\) such that the Hull-White price matches, keeping earlier pillars fixed.

Results are kept in `HullWhiteCalibrationQuality { records: Vec<HullWhiteCalibrationRecord> }`, each record holding `identifier, expiry, t, big_t, market_vol, market_price, model_price, calibrated_sigma, forward_rate, effective_strike`. `HullWhiteTimeDependentVolatility::new(schedule).with_pillar_labels().with_ift_sensitivities()` exposes the sigma pillars as labelled AD leaves so downstream prices carry sensitivities to the calibration quotes.

`cargo run -p hullwhite` bootstraps SOFR, builds the caplet surface, calibrates and prints a quality table (expiry, t, market vol, model implied vol, market price, model price, error) followed by ATM cap prices built from the calibrated model, then simulates paths using `examples/hullwhite/data/simulation.json`.

## Simulation

```json
{
  "market_index": "SOFR",
  "model": {
    "HullWhite": {
      "alpha": 0.1,
      "volatility": {
        "Calibrated": {
          "source": { "Surface": { "market_index": "SOFR" } },
          "quote_ids": ["..."],
          "strike": "Atm",
          "alpha": 0.1
        }
      }
    }
  },
  "n_paths": 1000,
  "seed": 42,
  "horizon": "5Y",
  "frequency": "Monthly"
}
```

`SimulationBuilder` calibrates (if `Calibrated`), then evolves \\(r\\) exactly on the date grid with the Gaussian transition \\(r\_{t+\Delta} = r_t e^{-\alpha\Delta} + \int\theta + \sigma\sqrt{\frac{1-e^{-2\alpha\Delta}}{2\alpha}}Z\\). Discount factors along a path are `zcb_price(r_t, t, T)`. In the XVA engine the same calibrated schedule is transferred to an LGM model via `LgmRateModel::calibrated` ([LGM](lgm.md)).
