# Scripting language

A payoff script describes a contract as a list of dated events. Each event can
observe market data, update variables, and make payments. Use scripts for
contracts whose coupon or redemption rules are easy to write as a small
program.

The script engine parses the events, works out which market data they need,
and evaluates them on simulated market paths.

- Variables keep their values along a path, so a later payment can depend on
  earlier observations.
- Payments are discounted and converted to the right currency by the market
  model.

`CodedEvent` pairs a date with script text. Converting a list of events into an
`EventStream` parses each script. Rules:

- Events must not be empty, must be in date order, and must not be before the
  reference date.
- Events on the same date run in list order.
- A variable must be assigned before it is read, in the same event or an
  earlier one.

## State, observations and payments

This event describes one coupon of a swap that receives fixed and pays
floating:

```text
swap = 0;
fixed_rate = 0.035;
accrual = cvg("2027-03-09", "2027-06-09", "Actual360");
floating = RateIndex("SOFR", "2027-03-09", "2027-06-09");
swap pays 10000000 * (fixed_rate - floating) * accrual on "2027-06-09";
```

Attach the event to the fixing and start date, 9 March. It observes the rate
on that date and schedules the payment for 9 June.

Syntax:

- Statements end with a semicolon. Blocks use braces.
- You can use assignment, compound assignment, arithmetic, powers,
  comparisons, boolean logic, arrays, and bounded `for` loops.
- Identifiers are case-sensitive.
- Comments and expressions follow the payoff-language grammar.

Market functions:

- `RateIndex` asks for a forward rate over the given period.
- `Spot("AAPL")` asks for an equity spot price.
- `Spot("USD","CLP")` asks for an FX rate, here CLP per USD.
- `Df` asks for a discount factor.
- `cvg` computes a year fraction with a named day counter.

The model must support every observation the script asks for. Writing a name in
a script does not create a missing model factor.

A payment has the form

```text
accumulator pays amount on "2027-06-09" in "USD";
```

- If you leave out the date, the payment is on the event date.
- If you leave out the currency, the local currency is used.
- The amount is the contract amount, before discounting.

When the script runs, each payment is converted, discounted, and divided by the
numeraire before it is added to the variable. Each payment gets a stable
identifier, used in cashflow reports and in claim conversion.

## Conditional payoffs and smoothing

A hard digital payoff \\(\mathbf1_{\\{X>K\\}}\\) has zero pathwise derivative
away from its boundary, so recording it gives no useful Monte Carlo
sensitivity. To get one, scripts replace market-dependent conditions with a
smooth transition of finite width.

For a comparison written as \\(x=\text{lhs}-\text{rhs}\\), the degree to which
it is true is

\\[
d_\varepsilon(x)=
\begin{cases}
0,&x<-\varepsilon/2,\\
(x+\varepsilon/2)/\varepsilon,&|x|\leq\varepsilon/2,\\
1,&x>\varepsilon/2.
\end{cases}
\\]

The result mixes both branches:
\\(d_\varepsilon V_{\rm then}+(1-d_\varepsilon)V_{\rm else}\\).

- Equality uses a butterfly-shaped profile.
- Boolean operators combine truth degrees with products and complements.
- Nested conditions keep a separate copy of the state for each branch.

The default width is

\\[
\varepsilon=\max\left(
0.02\max(|\text{lhs}|,|\text{rhs}|),10^{-8}\right).
\\]

So a 4% rate barrier gets a width of about 8 bp, and an equity barrier at 150
gets a width of 3. This changes the payoff as well as the numerics. The price
and its derivatives are those of the smoothed payoff, which can differ from the
exact term sheet.

To set the width yourself, use `fif(x,a,b,eps)`, which blends between two
expressions:

```text
coupon = fif(
    RateIndex("SOFR", "2027-03-09", "2027-06-09") - 0.04,
    0.05, 0.0, 0.001
);
```

Here the coupon moves from zero to 5% over a 10 bp range of the rate.
`min` and `max` payoffs are differentiable only along the branch that is
active.

## Products and validation

`ScriptedProduct` compiles an event stream and finds every payment, including
those inside branches and loops.

- It needs at least one payment.
- Payments before the reference date are errors.
- All claims created from the product share one engine, so the script is
  parsed only once.

Errors use the usual `QSError` categories:

- `ParsingErr`: grammar errors;
- `InvalidValueErr`: structural rule violations;
- `ValueNotSetErr`: a variable read before it is set;
- `NotFoundErr`: a missing response or result;
- `EvaluationErr`: an invalid operation in an expression.

The error context names the event or path that failed, and `root_cause()`
returns the underlying category. Validate the event stream before you start a
long simulation, so you can fix grammar and structure errors first.
