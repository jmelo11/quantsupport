# Market Data

Observable data is held in three primary stores.

- `QuoteStore` contains instrument quotes at bid, mid, and ask levels.
- `FixingStore` contains historical index observations needed by seasoned cashflows.
- `FxStore` contains spot FX rates and supports currency conversion requests.

Quote identifiers encode product and pillar information, for example `OIS_USD_SOFR_5Y`. Parse identifiers through `QuoteDetails` rather than splitting strings in application code:

```rust
use std::str::FromStr;
use quantsupport::prelude::*;

let today = Date::new(2025, 11, 11);
let mut quotes = QuoteStore::new(today);
let details = QuoteDetails::from_str("OIS_USD_SOFR_5Y")?;
quotes.add_quote(Quote::new(details, QuoteLevels::with_mid(0.04)));
# Ok::<(), QSError>(())
```

Builders select a `Level` such as `Level::Mid`. Keeping bid, mid, and ask together makes the selection explicit and prevents preprocessing from silently discarding market information.

The store reference date anchors curve construction. Fixing and quote dates must be coherent with the pricing date. Prefer loading the JSON schemas used by `examples/*/data/` and validate inputs at ingestion boundaries.
