# FX and Equity Options

QuantSupport supports European equity and FX options. Builders and trade types describe payoff economics; dedicated pricers supply closed-form or Monte Carlo valuation.

Equity choices include `EquityEuropeanOptionTrade`, `BlackEuropeanOptionPricer`, and `BlackMCEuropeanOptionPricer`. FX choices include `MakeFxOption`, `FxOptionTrade`, and `FxOptionPricer`.

Option valuation requires spot, strike, expiry, call/put orientation, discounting, carry or foreign discounting, and volatility. For FX, verify which currency is domestic and which is foreign; inverting the pair changes both strike and option orientation.

Closed-form Black pricing is efficient for European payoffs under compatible assumptions. Monte Carlo becomes useful when payoffs are path-dependent or share a simulation with exposure calculations. Compare Monte Carlo estimates with closed-form values before adding complexity.

Surface-backed volatility retains quote-level vega through AD. `OrientedFxVolSurface` adapts the stored FX surface to the requested pair orientation rather than requiring duplicate surfaces.
