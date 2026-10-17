# Python API by Domain

The [Python package reference](../api/autoapi/quantsupport/index.html)
has a separate page for each public class, with constructors, methods, and
properties. Sphinx generates it from the packaged type stubs without importing
the native extension. Public types are available both through their domain
package and as `quantsupport.Name`.

- [Time](../api/autoapi/quantsupport/time/index.html): dates, tenors, calendars, and accrual conventions.
- [Market](../api/autoapi/quantsupport/market/index.html): observations, configurations, curves, volatility, and simulations.
- [Instruments](../api/autoapi/quantsupport/instruments/index.html): product terms, coupons, loans, and payoff scripts.
- [Pricing](../api/autoapi/quantsupport/pricing/index.html): positions, contexts, policies, extension points, and results.
- [Risk](../api/autoapi/quantsupport/risk/index.html): scenarios, collateral agreements, exposure, and XVA.

Continue with the [Python walkthrough](getting-started.md) for a guided example,
or browse the [API reference index](../api/index.html)
for both bindings.

## QuantSupportError

`quantsupport.QuantSupportError` is the library exception for native failures.
Invalid Python argument types and values may also raise `TypeError` or `ValueError`.
