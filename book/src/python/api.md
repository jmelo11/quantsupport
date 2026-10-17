# Python API by Domain

The reference is generated from the packaged type stubs, without importing a
platform-specific native extension. Public types are available both through
their domain package and as `quantsupport.Name`.

- [Time](reference/time.md): dates, tenors, calendars, and accrual conventions.
- [Market](reference/market.md): observations, configurations, curves, volatility, and simulations.
- [Instruments](reference/instruments.md): product terms, coupons, loans, and payoff scripts.
- [Pricing](reference/pricing.md): positions, contexts, policies, extension points, and results.
- [Risk](reference/risk.md): scenarios, collateral agreements, exposure, and XVA.

## QuantSupportError

`quantsupport.QuantSupportError` is the library exception for native failures.
Invalid Python argument types and values may also raise `TypeError` or `ValueError`.