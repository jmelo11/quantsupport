# Scenario Analysis

Scenarios shock observable quotes before rebuilding dependent market objects. This captures nonlinear valuation effects and preserves consistency across curves, volatility, and simulations.

```rust
use std::str::FromStr;
use quantsupport::prelude::*;

let today = Date::new(2025, 11, 11);
let mut quotes = QuoteStore::new(today);
quotes.add_quote(Quote::new(
    QuoteDetails::from_str("OIS_USD_SOFR_1Y")?,
    QuoteLevels::with_mid(0.04),
));

let scenario = Scenario::new("SOFR", 0.0001, ScenarioType::Absolute);
let shocked = scenario.apply(&mut quotes)?;
assert_eq!(shocked, 1);
# Ok::<(), QSError>(())
```

An absolute scenario adds the shock to matching values. A relative scenario multiplies values by `1 + shock`. Targets can be exact quote identifiers or identifier segments such as `SOFR`.

Attach scenarios to `PricingContext` before `initialize()` so all dependent objects are rebuilt. For scenario P&L, compare shocked and base NPVs under otherwise identical inputs. Use small shocks to reconcile against AD, then larger shocks for stress testing.
