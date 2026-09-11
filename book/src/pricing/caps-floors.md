# Caps and Floors

A cap is a portfolio of caplets; a floor is a portfolio of floorlets. Each optionlet pays against an index fixing over an accrual period.

Build products with `MakeCapFloor` and choose `CapFloorType::Cap` or `CapFloorType::Floor`. Closed-form Black pricers are available for individual optionlets and full products:

- `ClosedFormBlackCapletPricer`
- `ClosedFormBlackCapPricer`

The pricer needs a forecast curve, discount curve, and volatility source. Ensure the market volatility convention matches `VolatilityType`; Black volatility assumes lognormal dynamics and normal volatility uses an absolute-rate convention.

Hull-White pricing is useful when the same calibrated short-rate model must support option valuation and simulation. Its volatility parameters can be calibrated to caplet quotes. Run `cargo run -p hullwhite` for curve setup, calibration diagnostics, and cap pricing.

Cap/floor vega is naturally distributed across the volatility quote pillars used by the surface or calibration source.
