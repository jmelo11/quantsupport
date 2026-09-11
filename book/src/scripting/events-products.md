# Events and Scripted Products

A scripted product is a **dated sequence of scripts**. Each script runs once per Monte Carlo path on its event date, with access to the market state simulated up to that date and to every variable assigned by earlier events. The types live in `src/scripting/nodes/event.rs` and `src/scripting/product.rs`.

## `CodedEvent`

```rust,ignore
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodedEvent { event_date: Date, script: String }

impl CodedEvent {
    pub fn new(event_date: Date, script: String) -> Self;
    pub fn event_date(&self) -> Date;
    pub fn script(&self) -> &String;
}
```

`CodedEvent` is the storage format: it is plain data and derives Serde, so a product can be persisted as JSON:

```json
[
  {
    "event_date": "2025-01-01",
    "script": "swap = 0; fixed_rate = 0.035; accrual = cvg(\"2025-01-01\", \"2025-04-01\", \"Actual360\"); floating_rate = RateIndex(\"SOFR\", \"2025-01-01\", \"2025-04-01\"); swap pays 10000000 * (fixed_rate - floating_rate) * accrual on \"2025-04-01\";"
  },
  {
    "event_date": "2025-04-01",
    "script": "accrual = cvg(\"2025-04-01\", \"2025-07-01\", \"Actual360\"); floating_rate = RateIndex(\"SOFR\", \"2025-04-01\", \"2025-07-01\"); swap pays 10000000 * (fixed_rate - floating_rate) * accrual on \"2025-07-01\";"
  }
]
```

Dates use the library's `Date` serialisation (`YYYY-MM-DD`).

## `Event` and `EventStream`

`Event::try_from(CodedEvent)` parses the source into a `Node` tree; a syntax error is reported as `ScriptingError::InvalidSyntax("<message> (event date: <date>)")`, so you always know which event failed.

```rust,ignore
pub struct Event { event_date: Date, expr: Node }
impl Event {
    pub fn new(event_date: Date, expr: Node) -> Self;
    pub fn event_date(&self) -> Date;
    pub fn expr(&self) -> &Node;
    pub fn mut_expr(&mut self) -> &mut Node;
}

#[derive(Default)]
pub struct EventStream { id: Option<usize>, events: Vec<Event> }
impl EventStream {
    pub fn new() -> Self;
    pub fn with_id(self, id: usize) -> Self;
    pub fn with_events(self, events: Vec<Event>) -> Self;
    pub fn add_event(&mut self, event: Event);
    pub fn events(&self) -> &[Event];
    pub fn mut_events(&mut self) -> &mut Vec<Event>;
    pub fn event_dates(&self) -> Vec<Date>;
}
impl TryFrom<Vec<CodedEvent>> for EventStream { type Error = ScriptingError; }
```

The usual way to build a stream is `EventStream::try_from(coded_events)`, exactly as `scripted_swap_events()` does in `examples/scripting/src/lib.rs`:

```rust,ignore
pub fn scripted_swap_events() -> Result<EventStream, ScriptingError> {
    let events: Vec<CodedEvent> = accrual_periods()
        .into_iter()
        .enumerate()
        .map(|(period, (start, end))| {
            let initialization = if period == 0 {
                format!("swap = 0; fixed_rate = {FIXED_RATE};")
            } else {
                String::new()
            };
            let source = format!(
                r#"
                {initialization}
                accrual = cvg("{start}", "{end}", "Actual360");
                floating_rate = RateIndex("SOFR", "{start}", "{end}");
                swap pays {NOTIONAL} * (fixed_rate - floating_rate) * accrual on "{end}";
                "#
            );
            CodedEvent::new(start, source)
        })
        .collect();
    EventStream::try_from(events)
}
```

Note the pattern: the **event date is the fixing date** (`start`), the rate is observed on that date, and the payment is deferred with `on "{end}"`. The engine discounts from the payment date back to the reference date on every path.

### Validation performed by `ScriptEngine::new`

| Check                                   | Error                                                                        |
| --------------------------------------- | ---------------------------------------------------------------------------- |
| Stream has no events                    | `InvalidOperation("a script must contain at least one event")`               |
| An event date precedes `reference_date` | `InvalidOperation("scripted event dates cannot precede the reference date")` |
| Events are not sorted by date           | `InvalidOperation("scripted events must be ordered by date")`                |

Two events may share a date; they are executed in order.

## `ScriptedProduct`

```rust,ignore
pub struct ScriptedProduct { id: String, engine: Arc<ScriptEngine>, payments: Vec<ScriptPayment> }

impl ScriptedProduct {
    pub fn new(
        id: impl Into<String>,
        events: EventStream,
        reference_date: Date,
        local_currency: Currency,
        local_discount_index: MarketIndex,
    ) -> Result<Self, ScriptingError>;
    pub fn id(&self) -> &str;
    pub fn maturity(&self) -> Date;                    // latest payment date
    pub fn contingent_claims(&self) -> QSResult<Vec<ContingentClaim>>;
}
impl IntoContingentClaims for ScriptedProduct {
    fn into_contingent_claims(&self, trade_id: &str) -> QSResult<Vec<ContingentClaim>>;
}
```

`ScriptedProduct::new` compiles the stream through `ScriptEngine::new`, walks every event's AST (including `if` branches, `for` bodies and indexed expressions) and records one `ScriptPayment { id, date, currency }` per `pays` node. Two extra checks apply: at least one `pays` must exist, and no payment date may precede the reference date. The default date of a payment is its event date; the default currency is `local_currency`.

Each payment becomes one `ContingentClaim` built with `MakeContingentClaim`:

| Claim field           | Value                                                          |
| --------------------- | -------------------------------------------------------------- |
| `trade_id`            | The id passed to `new` (or to `into_contingent_claims`)        |
| `leg_id`              | The payment id assigned during indexing                        |
| `payment_date`        | `on` date or event date                                        |
| `currency`            | `in` currency or `local_currency`                              |
| `notional`            | `1.0` (the script amount already includes the notional)        |
| `side`                | `Side::LongReceive` (sign lives in the script expression)      |
| `evaluation_strategy` | `ClaimEvaluationStrategy::Scripted { payoff: ScriptedPayoff }` |

`ScriptedPayoff` holds an `Arc<ScriptEngine>` and the payment id. When the XVA exposure evaluator reaches a valuation date it calls `ScriptedPayoff::evaluate(valuation_date, responses)`, which replays the script on the path's `SimulationResponse`s and returns only the value of that payment. The engine shares one compiled script between all claims, so a product with 40 coupons parses once.

## Reading scripts from files

Because `CodedEvent` is `Deserialize`, loading a product is a one-liner with `serde_json`:

```rust,ignore
let coded: Vec<CodedEvent> = serde_json::from_reader(File::open("product.json")?)?;
let product = ScriptedProduct::new("STRUCTURED_NOTE", EventStream::try_from(coded)?, ref_date, Currency::USD, MarketIndex::SOFR)?;
```

Pair this with the JSON `QuoteStore`, `CurveConfiguration` and `XvaEngineConfig` described in [Configuration](../reference/configuration.md) to keep an entire pricing job in data.
