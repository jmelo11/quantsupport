# Events and Scripted Products

A scripted product is a **dated sequence of scripts**. Each script runs once per Monte Carlo path on its event date, with access to the simulated market state and the variables assigned by earlier events. This chapter follows a product from serializable source events through parsed event streams to the contingent claims consumed by XVA. The types live in `src/scripting/nodes/event.rs` and `src/scripting/product.rs`.

## `CodedEvent`

`CodedEvent` is the external representation of one dated program. It contains plain source text and a date, which makes it suitable for construction in Rust or deserialization from a product file:

```rust,ignore
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodedEvent { event_date: Date, script: String }

impl CodedEvent {
    pub fn new(event_date: Date, script: String) -> Self;
    pub fn event_date(&self) -> Date;
    pub fn script(&self) -> &String;
}
```

Deriving Serde allows a sequence of coded events to be persisted as JSON. This example stores the first two accrual periods of a scripted swap:

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

Each JSON object becomes one `CodedEvent`. Dates use the library's `YYYY-MM-DD` serialization, and the script remains a string until parsing begins.

## `Event` and `EventStream`

`Event::try_from(CodedEvent)` parses source text into a `Node` tree. A syntax error includes the event date in `ScriptingError::InvalidSyntax`, which identifies the affected point in the product timeline. `EventStream` then owns the ordered collection of parsed events:

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

Most applications build a stream with `EventStream::try_from(coded_events)`. The `scripted_swap_events()` helper in `examples/scripting/src/lib.rs` demonstrates how regular accrual periods can generate source events programmatically:

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

In this pattern, the **event date is the fixing date** given by `start`. The rate is observed on that event, and `on "{end}"` schedules payment at the accrual end. The engine discounts from the payment date to the reference date on every path.

### Validation performed by `ScriptEngine::new`

Engine construction validates the timeline before model paths are requested. The checks and their diagnostic messages are:

| Check                                   | Error                                                                        |
| --------------------------------------- | ---------------------------------------------------------------------------- |
| Stream has no events                    | `InvalidOperation("a script must contain at least one event")`               |
| An event date precedes `reference_date` | `InvalidOperation("scripted event dates cannot precede the reference date")` |
| Event dates are out of order            | `InvalidOperation("scripted events must be ordered by date")`                |

Two events may share a date. Their vector order determines their execution order and therefore the state visible to the later event.

## `ScriptedProduct`

`ScriptedProduct` is the bridge from a validated event stream to the product and claim interfaces. It owns a shared compiled engine and a catalog of every scripted payment:

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

`ScriptedProduct::new` compiles the stream through `ScriptEngine::new`. It walks every event's AST, including conditional branches, loop bodies, and indexed expressions, and records one `ScriptPayment { id, date, currency }` per `pays` node. Product validation requires at least one payment and requires every payment date to fall on or after the reference date. A missing payment date uses the event date, and a missing payment currency uses `local_currency`.

Each recorded payment becomes one `ContingentClaim` built with `MakeContingentClaim`. The field mapping preserves the script's identity and economic meaning:

| Claim field           | Value                                                          |
| --------------------- | -------------------------------------------------------------- |
| `trade_id`            | The id passed to `new` (or to `into_contingent_claims`)        |
| `leg_id`              | The payment id assigned during indexing                        |
| `payment_date`        | `on` date or event date                                        |
| `currency`            | `in` currency or `local_currency`                              |
| `notional`            | `1.0` (the script amount already includes the notional)        |
| `side`                | `Side::LongReceive` (sign lives in the script expression)      |
| `evaluation_strategy` | `ClaimEvaluationStrategy::Scripted { payoff: ScriptedPayoff }` |

`ScriptedPayoff` holds an `Arc<ScriptEngine>` and one payment id. At each XVA valuation date, the exposure evaluator calls `ScriptedPayoff::evaluate(valuation_date, responses)`. The method replays the program on the path's `SimulationResponse` sequence and returns the value associated with that payment. All claims share one compiled engine, so a product with 40 coupons performs parsing once.

## Reading scripts from files

Because `CodedEvent` implements `Deserialize`, an application can load the event list with `serde_json` and pass the parsed stream directly to `ScriptedProduct`:

```rust,ignore
let coded: Vec<CodedEvent> = serde_json::from_reader(File::open("product.json")?)?;
let product = ScriptedProduct::new("STRUCTURED_NOTE", EventStream::try_from(coded)?, ref_date, Currency::USD, MarketIndex::SOFR)?;
```

The resulting product can share a data-driven job with the JSON `QuoteStore`, `CurveConfiguration`, and `XvaEngineConfig` described in [Configuration](../reference/configuration.md). Product terms, market construction, and XVA settings can then be stored and versioned together.

## What to remember

`CodedEvent` is the serializable source form, `Event` is one parsed program, and `EventStream` is the validated timeline. `ScriptedProduct` discovers payments in that timeline and turns each one into a contingent claim backed by the shared compiled engine. These stages preserve dates, state, and payment identities from product data through pathwise exposure.
