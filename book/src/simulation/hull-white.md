# Hull-White Model

Hull-White is a one-factor Gaussian short-rate model that fits the initial discount curve through a time-dependent drift. It supports closed-form bond options, caplets, and swaptions, and it also generates rate paths for Monte Carlo pricing. This chapter explains the model API, its calibration to a constructed volatility market, and its simulation configuration. The implementation lives in `src/models/hullwhite/`.

The short rate follows

\\[
dr_t = \bigl(\theta(t) - \alpha r_t\bigr)dt + \sigma(t)\\,dW_t .
\\]

## API

The model owns mean reversion, a reference to the initial curve, and an optional volatility function and calibration report. The following definition and constructor show those responsibilities:

```rust,ignore
pub struct HullWhite<'a, T: Scalar> {
    alpha: T,
    curve: &'a dyn InterestRatesTermStructure<T>,
    calibration_quality: Option<HullWhiteCalibrationQuality>,
    vol_func: Option<HullWhiteTimeDependentVolatility<T>>,
}

let mut hw = HullWhite::new(alpha, &sofr_curve).with_constant_volatility(0.01);
```

Once volatility has been supplied, the model can evaluate its central affine quantities and closed-form option prices:

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

`B` measures how a future bond responds to the current short rate, and `A` fits that bond price to the initial curve. The option methods build on these affine bond prices. `ClosedFormHullWhiteCapletPricer`, `ClosedFormHullWhiteCapPricer`, and `ClosedFormHullWhiteSwaptionPricer` reuse the same formulas, as described in [Caps and Floors](../pricing/caps-floors.md) and [Swaptions](../pricing/swaptions.md).

## Calibration

Calibration converts market caplet or swaption volatilities into the model's piecewise-constant short-rate volatility schedule. The caller supplies a semantic calibration configuration together with the constructed market and curve:

```rust,ignore
hw.calibrate_with_configuration(&config, &constructed_store, &quote_store, &curve, Level::Mid)?;
```

`examples/hullwhite/data/vol_specs.json` defines the caplet quotes that form the surface. The `Calibrated` part of `simulation.json` references that surface and selects the instruments by expiry, index tenor, and strike rule:

```json
{
  "source": { "Surface": { "market_index": "SOFR" } },
  "calibration_basket": {
    "expiries": ["3M", "6M", "1Y"],
    "tenors": ["3M"],
    "strike": "Atm"
  }
}
```

The calibrator processes the selected instruments in expiry order:

1. Resolve the surface or cube instrument identifiers and select the configured expiries and tenors. `strike: "Atm"` sets each calibration strike to its forward rate.
2. Read the Black (or Normal) vol from the surface/cube and compute the market caplet/swaption price.
3. Solve by bisection for the piecewise-constant \\(\sigma_i\\) on \\([T_{i-1},T_i]\\) such that the Hull-White price matches, keeping earlier pillars fixed.

The result is a `HullWhiteCalibrationQuality` containing one `HullWhiteCalibrationRecord` per target. A record captures the instrument identifier, timing, market volatility, market and model prices, calibrated sigma, forward rate, and effective strike. This gives users both numerical parameters and an instrument-level quality report.

`HullWhiteTimeDependentVolatility::new(schedule).with_pillar_labels().with_ift_sensitivities()` equips the calibrated schedule with market labels and implicit-function derivatives. Downstream prices can then carry sensitivities to the option quotes used in calibration.

The `hullwhite` example bootstraps SOFR, builds the caplet surface, calibrates the schedule, and prints an instrument-level quality table. It then prices ATM caps with the calibrated model and simulates paths from `examples/hullwhite/data/simulation.json`. Run it with `cargo run -p hullwhite`.

## Simulation

Simulation can use the same calibrated source through `SimulationConfiguration`. The complete JSON below chooses the SOFR surface, selects ATM instruments, and defines a monthly five-year path grid:

```json
{
  "market_index": "SOFR",
  "model": {
    "HullWhite": {
      "alpha": 0.1,
      "parameter_source": {
        "Calibrated": {
          "source": { "Surface": { "market_index": "SOFR" } },
          "calibration_basket": { "strike": "Atm" }
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

`SimulationBuilder` resolves the parameter source before path generation. A calibrated source runs the procedure above, and a fixed source creates a constant schedule from its sigma. The model then evolves \\(r\\) exactly on the date grid with the Gaussian transition \\(r_{t+\Delta} = r_t e^{-\alpha\Delta} + \int\theta + \sigma\sqrt{\frac{1-e^{-2\alpha\Delta}}{2\alpha}}Z\\). Pathwise discount factors use `zcb_price(r_t, t, T)`. For XVA, `LgmRateModel::calibrated` accepts the same calibrated schedule, as explained in [LGM](lgm.md).

## What to remember

Hull-White combines an initial-curve fit, mean reversion, and a model volatility schedule. Fixed configuration supplies that schedule directly. Calibrated configuration derives it from selected caplet or swaption instruments and retains an instrument-level quality report together with quote sensitivities. The same resolved model supports closed-form pricing and Monte Carlo paths.
