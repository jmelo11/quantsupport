# Dated payoff programs

Payoff scripts describe observations, state changes and payments in a dated
sequence of events. They are useful for contracts whose coupon or redemption
rules can be expressed directly as a small program. The native engine parses
the events, determines their market-data requirements and evaluates them
against simulated market states.

Variables persist along each path, allowing a later payment to depend on
earlier observations. Payment statements apply discounting and currency
conversion through the configured market model.

`CodedEvent` pairs a date with source text. Conversion into an `EventStream`
parses each program into an expression tree. Events must be nonempty, ordered
and not precede the reference date; equal-date events execute in vector order.
Every variable read must follow an assignment in the current or an earlier
event.

## State, observations and payments

The following event describes one receive-fixed/pay-floating coupon:

```text
swap = 0;
fixed_rate = 0.035;
accrual = cvg("2027-03-09", "2027-06-09", "Actual360");
floating = RateIndex("SOFR", "2027-03-09", "2027-06-09");
swap pays 10000000 * (fixed_rate - floating) * accrual on "2027-06-09";
```

Attach this event to the fixing/start date, 9 March. The event observes the
rate at that date and declares the payment due on 9 June.

Statements terminate with semicolons; blocks use braces. Assignment,
compound assignment, arithmetic, power, comparisons, boolean logic, arrays
and bounded `for` loops provide ordinary state manipulation. Identifiers are
case-sensitive. Script strings follow the native payoff-language grammar, including its
supported comment and expression syntax.

`RateIndex` requests a forward observation over the specified interval.
`Spot("AAPL")` requests an equity spot; a currency pair such as
`Spot("USD","CLP")` requests CLP per USD. `Df` requests a discount factor,
and `cvg` delegates year-fraction measurement to a named day counter.
The model must support the requested observations; parsing a name does not
create a missing factor.

A payment has the form

```text
accumulator pays amount on "2027-06-09" in "USD";
```

Date and currency clauses can be omitted, in which case they use the event
date and local currency. The amount is contractual before discounting.
Evaluation converts/discounts it and deflates by the appropriate numeraire
before adding to the accumulator. Every payment node receives a stable
identity for cashflow reporting and claim conversion.

## Conditional payoffs and smoothing

A hard digital payoff \\(\mathbf1_{\\{X>K\\}}\\) has zero pathwise derivative
away from its boundary. Merely recording it on a tape does not produce a
useful Monte Carlo digital sensitivity. Scripting deliberately smooths
market-dependent conditionals with a finite-width transition.

For a comparison reduced to \\(x=\text{lhs}-\text{rhs}\\), its truth degree is

\\[
d_\varepsilon(x)=
\begin{cases}
0,&x<-\varepsilon/2,\\
(x+\varepsilon/2)/\varepsilon,&|x|\leq\varepsilon/2,\\
1,&x>\varepsilon/2.
\end{cases}
\\]

The evaluated state blends both branches as
\\(d_\varepsilon V_{\rm then}+(1-d_\varepsilon)V_{\rm else}\\).
Equality uses a butterfly profile; boolean operations combine truth degrees
through products/complements. Nested conditionals need separate branch-state
snapshots.

The engine's automatic width is

\\[
\varepsilon=\max\left(
0.02\max(|\text{lhs}|,|\text{rhs}|),10^{-8}\right).
\\]

A 4% rate barrier therefore receives roughly 8 bp width, whereas an equity
barrier at 150 receives width 3. These are economic approximations as well as
numerical choices. Price and derivative belong to the smoothed program, not
necessarily the exact discontinuous term sheet.

For an explicitly chosen width, `fif(x,a,b,eps)` blends between two numerical
expressions:

```text
coupon = fif(
    RateIndex("SOFR", "2027-03-09", "2027-06-09") - 0.04,
    0.05, 0.0, 0.001
);
```

Here the digital-style coupon changes from zero to 5% across a 10 bp rate
interval. Piecewise `min`/`max` payoffs remain differentiable only along their
selected active branches.

## Products and validation

`ScriptedProduct` compiles an event stream and discovers payment nodes,
including those in branches and loops. It requires at least one payment and
rejects payments before the reference date. The same engine is shared by
claims, avoiding repeated parsing for each coupon.

Parsing and evaluation use the common `QSError` categories: `ParsingErr` for
grammar, `InvalidValueErr` for structural rules, `ValueNotSetErr` for
uninitialized state, `NotFoundErr` for unavailable responses/results and
`EvaluationErr` for invalid expression operations. Context preserves the
event/path that failed, and `root_cause()` exposes the underlying category.
Validate the event stream before starting a long simulation so structural and
grammar errors can be resolved first.