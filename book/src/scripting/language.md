# Script Language

The scripting language expresses payoff state, market observations, conditions, loops, and dated payments in a compact imperative form. Each event contains one program and runs on one simulation date. This chapter explains the grammar from basic statements through market access and smoothed conditionals, then combines those elements in representative payoffs.

Statements end with `;` and blocks use `{ }`. Descriptive comments belong in the surrounding Rust or JSON that owns the script. The formal grammar is implemented in `src/scripting/parsing/lexer.rs` and `parser.rs`.

## Statements

Statements update variables, record payments, and control execution. The core forms are:

```text
x = 0.035;
x += 1; x -= 1; x *= 2; x /= 2;
acc pays amount on "2027-06-09" in "USD";
if cond { ... } else { ... }
for i in range(1, 3) { ... }
for s in [1, 2, 3] { ... }
```

Assignment and compound assignment operate on numeric variables. `if` and `for` introduce blocks, and `pays` records a dated cashflow. Identifiers are case-sensitive and must be distinct from the reserved words `if else and or not for true false pays on in` and from built-in function names. Every read requires an assignment in the current event or an earlier one. Variable values persist across events on the same path.

## Literals and operators

Expressions combine literal values, variables, array operations, and the operators listed below. String literals carry typed names such as dates, currencies, day counters, and indices when passed to built-in functions.

| Category   | Syntax                                                                                |
| ---------- | ------------------------------------------------------------------------------------- |
| Numbers    | `0.035`, `10000000`, `1e-4`                                                           |
| Booleans   | `true`, `false`                                                                       |
| Strings    | `"2027-06-09"`, `"USD"`, `"Actual360"` (dates, currencies, day counters, index names) |
| Arithmetic | `+ - * /`, `**` (power), unary `+`/`-`                                                |
| Comparison | `== != < <= > >=`                                                                     |
| Logic      | `and`, `or`, `not`                                                                    |
| Arrays     | `[1, 2, 3]`, `range(1, 3)`, `vals[0]`, `vals.append(x)`, `vals.mean()`, `vals.std()`  |

Comparisons produce boolean conditions for `if` and for the logical operators `and`, `or`, and `not`. `fif`, described below, converts a condition-like numeric expression into a differentiable numeric blend.

## Built-in functions

Built-in functions cover elementary mathematics, date accruals, smoothing, and loop ranges. Their argument rules and numerical meaning are:

| Function                            | Meaning                                                                                                                                                                                               |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `exp(x)`, `ln(x)`, `pow(x, y)`      | Elementary functions                                                                                                                                                                                  |
| `min(a, b, ...)`, `max(a, b, ...)`  | Variadic (2 to 100 arguments) min/max, differentiable almost everywhere                                                                                                                               |
| `cvg("start", "end", "DayCounter")` | Year fraction between two dates. Day-counter names follow the `DayCounter` enum: `Actual360`, `Actual365`, `Thirty360`, `Thirty360US`, `ActualActual`, `Business252`                                  |
| `fif(x, a, b, eps)`                 | Smoothed "functional if": returns `a` where `x > eps/2`, `b` where `x < -eps/2`, and interpolates linearly in between (a call spread of width `eps`), so the derivative with respect to `x` is finite |
| `range(a, b)`                       | Integer range `a..b` for `for` loops                                                                                                                                                                  |

`cvg` delegates time measurement to the library's day-counter implementations. `fif` creates a finite-width transition around a boundary, which is important for pathwise sensitivity of digital-style payoffs.

## Market data access

Market functions connect expressions to simulated state. The event date is the default observation date, and functions with an explicit date request that point in time. Static analysis converts each call into an entry in the event's `SimulationDataRequest`, which the selected market model must be able to serve.

| Expression                          | Request created                                        | Notes                                                                                                   |
| ----------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| `RateIndex("SOFR", "start", "end")` | `ForwardRateRequest`                                   | Simple forward rate of `MarketIndex::SOFR` for the period. On the fixing date this is the realized rate |
| `Df("2027-06-09")`                  | `DiscountRequest` on the engine's local discount index | Discount factor from the event date to the given date                                                   |
| `Df("2027-06-09", "TermSOFR3m")`    | `DiscountRequest` on a named curve                     |                                                                                                         |
| `Spot("AAPL")`                      | `SpotRequest` for `MarketIndex::Equity("AAPL")`        | Equity spot on the event date                                                                           |
| `Spot("USD", "CLP")`                | `FxRequest`                                            | Price of one `USD` in `CLP`                                                                             |
| `Spot("USD", "CLP", "2024-12-31")`  | `FxRequest` with explicit observation date             |                                                                                                         |

Index names are parsed with `MarketIndex::from_str`, and currencies use `Currency::try_from`. Their spelling therefore follows the corresponding enum variants, including `SOFR`, `TermSOFR3m`, `ICP`, and `ESTR`. Validation catches an unknown name before or during request resolution.

## Payments

A `pays` expression creates a cashflow and adds its present-value contribution to an accumulator. The full syntax exposes optional payment date and currency clauses:

```text
acc pays <amount> [on "<date>"] [in "<CCY>"];
```

- `acc` is the accumulator variable. The engine adds the **discounted, numeraire-deflated** value of the payment to it, so the amount represents the contractual cashflow before discounting.
- `on` defaults to the event date, and `in` defaults to the engine's local currency. Payments in another currency are converted with the simulated FX rate on the payment date.
- Every `pays` statement gets a payment id during indexing. `ScriptedProduct` turns each id into one `ContingentClaim`, and `evaluate_with_cashflows` reports each id's expected amount and present value.
- `pays` may also appear inside an expression, e.g. `call = pays max(Spot("CLP", "USD") - 900.0, 0);`, which assigns the discounted payment to `call`.
- An `EventStream` must contain at least one event, and a `ScriptedProduct` must contain at least one `pays` expression with no payment before the reference date.

Payment identifiers connect direct script valuation to claim conversion and expected-cashflow reporting. The same source statement therefore has a stable identity across those workflows.

## Conditionals and smoothing

Conditionals describe barriers, digitals, callability, and other state-dependent behavior. This example pays an amount according to whether the simulated AAPL spot is below a trigger:

```text
if Spot("AAPL") <= trigger {
    deal pays 100000 on "2027-09-09";
} else {
    deal pays 0 on "2027-09-09";
}
```

A plain `if` is a discontinuous function of `Spot("AAPL")`, so its pathwise derivative is zero almost everywhere and infinite at the barrier. The `FuzzyEvaluator` (`visitors/fuzzyevaluator.rs`) first lets `IfConditionTransform` rewrite every comparison into the canonical form \\((\text{lhs}-\text{rhs}) > 0\\), then replaces the hard branch selection by a truth degree \\(d_t\in[0,1]\\) computed with a call spread of width \\(\varepsilon\\):

\\[
d_t(x) = \begin{cases} 0 & x < -\varepsilon/2 \\ \dfrac{x + \varepsilon/2}{\varepsilon} & |x| \le \varepsilon/2 \\ 1 & x > \varepsilon/2 \end{cases},
\qquad
\text{result} = d_t\cdot\text{then} + (1-d_t)\cdot\text{else}.
\\]

Equality tests use a butterfly profile. The logical operators `and`, `or`, and `not` combine truth degrees through products and complements. With automatic scaling, which is the default in `ScriptEngine`, each comparison chooses the width

\\[
\varepsilon = \max\bigl(0.02\cdot\max(|\text{lhs}|,|\text{rhs}|),\\;10^{-8}\bigr)
\\]

The constants `AUTO_SMOOTHING_RELATIVE_WIDTH` and `AUTO_SMOOTHING_MIN_WIDTH` define that rule. A rate barrier at 4 percent receives a width of roughly 8 basis points. An equity barrier at 150 receives a width of 3 price units. `FuzzyEvaluator::with_eps` supplies the fallback width when automatic scaling is disabled. For nested conditions, `IfProcessor` computes the maximum depth and allocates one variable snapshot per level so both branches can be evaluated and blended.

Use `fif` when a single expression is more readable than an `if` block, for example a digital coupon paying 5% when SOFR fixes above 4%, smoothed over 10 bp:

```text
coupon = fif(RateIndex("SOFR", "2027-03-09", "2027-06-09") - 0.04, 0.05, 0.0, 0.001);
```

Here the first argument is positive above the barrier, the next two arguments are the high and low coupon values, and `0.001` defines the transition width in rate units.

## Worked payoffs

The following examples show how the language primitives combine into complete payment definitions. Each snippet would be attached to a dated `CodedEvent`.

A capped floating coupon reads a forward rate, applies the cap with `min`, and pays the resulting accrual amount:

```text
a1 = cvg("2027-03-09", "2027-06-09", "Actual360");
r1 = RateIndex("SOFR", "2027-03-09", "2027-06-09");
deal pays 10000000 * (0.0385 - min(r1, 0.06)) * a1 on "2027-06-09";
```

A cash-settled European equity put reads spot on its event date and applies the terminal payoff:

```text
payoff pays 10000 * max(strike - Spot("AAPL"), 0) / strike on "2027-09-09";
```

A single-event fixed-rate note can state its redemption amount and currency directly:

```text
note = 0; note pays 1052500 on "2028-09-09" in "USD";
```

Together, these examples cover projected rates, simulated spots, payoff functions, accrual calculations, explicit dates, and currency conversion.

## Errors

Parsing, static analysis, and runtime evaluation return the library's `QSError` and `Result<T>`.

| Shared variant                | Raised when                                                                                  |
| ----------------------------- | -------------------------------------------------------------------------------------------- |
| `ParsingErr(String)`           | Invalid characters, malformed numeric literals, unterminated strings, or grammar violations   |
| `InvalidValueErr(String)`      | Structural rules fail, such as empty or unordered events or a model with zero paths           |
| `ValueNotSetErr(String)`       | A required scenario, variable index, or initialized variable is absent                        |
| `NotFoundErr(String)`          | A result variable or requested market response cannot be found                               |
| `EvaluationErr(String)`        | An expression cannot be evaluated, such as a nonnumeric payment or an invalid array index    |
| Other library variants        | Date parsing, solvers, AD operations, and market models retain their original errors          |

`QSError::context` attaches operation details while preserving the original error as its source. For example, a parsing error carries its event date, and a model failure carries its path index. Messages from quote, curve, volatility, and simulation construction also identify the operation and relevant resource. Use `std::error::Error::source` to inspect the cause chain and `error.root_cause()` to match the original library variant:

```rust
match error.root_cause() {
    QSError::ParsingErr(_) => report_invalid_script(&error),
    QSError::NotFoundErr(_) => report_missing_data(&error),
    _ => report_failure(&error),
}
```

Code that matched the former scripting variants should use these shared variants. Grammar and token errors use `ParsingErr`; `InvalidOperation` uses `InvalidValueErr`; `NotFoundError` uses `NotFoundErr`; and `EvaluationError` uses `EvaluationErr` or the more specific missing-value variants above. The former `QuantSupport` wrapper is no longer needed.

## What to remember

The language organizes a payoff as state updates and dated payments. Market functions declare the simulated observations required by each event. Condition smoothing gives discontinuous economic logic a finite pathwise derivative, and validation turns grammar or market mismatches into localized errors before a long simulation run.
