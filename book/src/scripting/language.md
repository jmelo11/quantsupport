# Script Language

Scripts are small imperative programs. Each event holds one script; statements are separated by `;`, blocks use `{ }`, and `#`-style comments are not supported (keep comments in the surrounding Rust or JSON). The grammar is defined in `src/scripting/parsing/lexer.rs` and `parser.rs`.

## Statements

```text
x = 0.035;                 # assignment
x += 1; x -= 1; x *= 2; x /= 2;
acc pays amount on "2027-06-09" in "USD";
if cond { ... } else { ... }
for i in range(1, 3) { ... }
for s in [1, 2, 3] { ... }
```

Identifiers are case-sensitive and must not collide with the reserved words `if else and or not for true false pays on in` or with the built-in function names below. Every variable that is read must have been assigned in the same or an earlier event; variables persist across events on the same path.

## Literals and operators

| Category   | Syntax                                                                                |
| ---------- | ------------------------------------------------------------------------------------- |
| Numbers    | `0.035`, `10000000`, `1e-4`                                                           |
| Booleans   | `true`, `false`                                                                       |
| Strings    | `"2027-06-09"`, `"USD"`, `"Actual360"` (dates, currencies, day counters, index names) |
| Arithmetic | `+ - * /`, `**` (power), unary `+`/`-`                                                |
| Comparison | `== != < <= > >=`                                                                     |
| Logic      | `and`, `or`, `not`                                                                    |
| Arrays     | `[1, 2, 3]`, `range(1, 3)`, `vals[0]`, `vals.append(x)`, `vals.mean()`, `vals.std()`  |

Comparisons produce booleans that can only be used in `if` conditions or combined with `and`/`or`/`not`; they cannot be assigned to numeric variables. Use `fif` (below) when you need a differentiable numeric indicator.

## Built-in functions

| Function                            | Meaning                                                                                                                                                                                               |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `exp(x)`, `ln(x)`, `pow(x, y)`      | Elementary functions                                                                                                                                                                                  |
| `min(a, b, ...)`, `max(a, b, ...)`  | Variadic (2 to 100 arguments) min/max, differentiable almost everywhere                                                                                                                               |
| `cvg("start", "end", "DayCounter")` | Year fraction between two dates; day counter names follow the `DayCounter` enum: `Actual360`, `Actual365`, `Thirty360`, `Thirty360US`, `ActualActual`, `Business252`                                  |
| `fif(x, a, b, eps)`                 | Smoothed "functional if": returns `a` where `x > eps/2`, `b` where `x < -eps/2`, and interpolates linearly in between (a call spread of width `eps`), so the derivative with respect to `x` is finite |
| `range(a, b)`                       | Integer range `a..b` for `for` loops                                                                                                                                                                  |

## Market data access

Market observations are resolved on the **event date** unless a date argument is given. Each call becomes an entry in that event's `SimulationDataRequest`; the market model must be able to serve it.

| Expression                          | Request created                                        | Notes                                                                                                   |
| ----------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| `RateIndex("SOFR", "start", "end")` | `ForwardRateRequest`                                   | Simple forward rate of `MarketIndex::SOFR` for the period; on the fixing date this is the realised rate |
| `Df("2027-06-09")`                  | `DiscountRequest` on the engine's local discount index | Discount factor from the event date to the given date                                                   |
| `Df("2027-06-09", "TermSOFR3m")`    | `DiscountRequest` on a named curve                     |                                                                                                         |
| `Spot("AAPL")`                      | `SpotRequest` for `MarketIndex::Equity("AAPL")`        | Equity spot on the event date                                                                           |
| `Spot("USD", "CLP")`                | `FxRequest`                                            | Price of one `USD` in `CLP`                                                                             |
| `Spot("USD", "CLP", "2024-12-31")`  | `FxRequest` with explicit observation date             |                                                                                                         |

Index names are parsed with `MarketIndex::from_str`, currencies with `Currency::try_from`, so the spelling must match the enum variants (`SOFR`, `TermSOFR3m`, `ICP`, `ESTR`, …).

## Payments

```text
acc pays <amount> [on "<date>"] [in "<CCY>"];
```

- `acc` is the accumulator variable; the engine adds the **discounted, numeraire-deflated** value of the payment to it. Never multiply the amount by a discount factor yourself.
- `on` defaults to the event date; `in` defaults to the engine's local currency. Payments in another currency are converted with the simulated FX rate on the payment date.
- Every `pays` statement gets a payment id during indexing. `ScriptedProduct` turns each id into one `ContingentClaim`, and `evaluate_with_cashflows` reports each id's expected amount and present value.
- `pays` may also appear inside an expression, e.g. `call = pays max(Spot("CLP", "USD") - 900.0, 0);`, which assigns the discounted payment to `call`.
- An `EventStream` must contain at least one event, and a `ScriptedProduct` must contain at least one `pays` expression with no payment before the reference date.

## Conditionals and smoothing

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

Equality tests use a butterfly instead of a call spread, and `and`/`or`/`not` are implemented as products and complements of truth degrees. The width is chosen per comparison: with automatic scaling (the default in `ScriptEngine`) it is

\\[
\varepsilon = \max\bigl(0.02\cdot\max(|\text{lhs}|,|\text{rhs}|),\\;10^{-8}\bigr)
\\]

(constants `AUTO_SMOOTHING_RELATIVE_WIDTH` and `AUTO_SMOOTHING_MIN_WIDTH`), so a rate barrier at 4% is smoothed over roughly 8 bp while an equity barrier at 150 is smoothed over 3 price units. `FuzzyEvaluator::with_eps` overrides the fallback width used when auto scaling is off. Nested `if`s are supported; the `IfProcessor` pass computes the maximum nesting depth and pre-allocates one variable snapshot per level so both branches can be evaluated and blended.

Use `fif` when a single expression is more readable than an `if` block, for example a digital coupon paying 5% when SOFR fixes above 4%, smoothed over 10 bp:

```text
coupon = fif(RateIndex("SOFR", "2027-03-09", "2027-06-09") - 0.04, 0.05, 0.0, 0.001);
```

## Worked payoffs

Capped floating coupon (per accrual period event):

```text
a1 = cvg("2027-03-09", "2027-06-09", "Actual360");
r1 = RateIndex("SOFR", "2027-03-09", "2027-06-09");
deal pays 10000000 * (0.0385 - min(r1, 0.06)) * a1 on "2027-06-09";
```

European equity put settled in cash:

```text
payoff pays 10000 * max(strike - Spot("AAPL"), 0) / strike on "2027-09-09";
```

Fixed-rate note (single event, explicit currency):

```text
note = 0; note pays 1052500 on "2028-09-09" in "USD";
```

## Errors

Parsing and evaluation return `ScriptingError`:

| Variant                         | Raised when                                                                                 |
| ------------------------------- | ------------------------------------------------------------------------------------------- |
| `InvalidSyntax(String)`         | Grammar violation or reserved-word misuse; the message includes line and column             |
| `UnexpectedToken(String)`       | Token sequence does not match the expected production                                       |
| `InvalidToken(String)`          | Lexer could not classify a character sequence                                               |
| `ParsingError(ParseFloatError)` | Malformed numeric literal                                                                   |
| `EvaluationError(String)`       | Runtime failure, e.g. the requested result variable is not defined                          |
| `NotFoundError(String)`         | A market response slot was missing                                                          |
| `InvalidOperation(String)`      | Structural rule broken, e.g. no events, unordered events, payment before the reference date |
| `QuantSupport(QSError)`         | Wrapped library error (date parsing, unknown index or currency, …)                          |
