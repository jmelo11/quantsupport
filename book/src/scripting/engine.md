# Script engine and exposure claims

`ScriptEngine` prepares a payoff script and evaluates it on market paths. It
parses the script once and works out which observations the model must supply.
On each path, the model supplies those observations, the script updates its
variables, and payments are added up.

For the result variable you choose, the engine also computes derivatives with
respect to the registered market and model inputs. Memory used for derivatives
is freed after each path.

## Preparing the program

`ScriptEngine::new` takes an `EventStream`, a reference date, a local currency,
and a local discount index. During preparation the engine:

- assigns storage and payment identifiers to the variables;
- turns market calls into `SimulationDataRequest` values for each event, with
  their observation dates;
- analyses conditions and branches, and removes branches that can never run.

The script's horizon covers every observation and payment it asks for, not
only the event dates. For example, a fixing event can schedule a payment much
later. Configure the model for the whole period.

The following fragment assumes an existing event stream and a differentiable
market model:

```rust,ignore
let engine = ScriptEngine::new(
    events, reference_date, Currency::USD, MarketIndex::SOFR,
)?;
let values = engine.evaluate(&mut model, Some("swap"))?;
let value = values.get("swap");
```

- `evaluate` returns the path average of the variables. Derivatives are
  computed only for the variable you select.
- Scripts without conditions are evaluated exactly. Scripts with conditions use
  the smoothing described in the [language chapter](language.md).

## Pathwise AD and reporting

Curve pillars and model inputs are registered as AD inputs.

The parallel API compiles supported model and script layouts into one path
program that all paths share. Each path supplies its random draws, and the
compiled program computes values, payments, and first derivatives. Later
calls reuse the compiled code with the current market coefficients and risk
map, when the layout still matches.

For other layouts, the engine runs a shared setup step once. It then records,
compiles, and differentiates each path separately and discards the recording
afterward. The gradients include the setup's dependencies and are summed over
the paths. This keeps memory use bounded. `ScriptModelSetup` creates the inputs
for each worker's recording.

Results:

- Averages are divided by the total number of paths.
- Path results are reproducible by path index, but sums over paths may differ
  in the last bits with different thread layouts.
- The compiled path program and the per-path route have different costs for
  the first call and for repeated calls. They are benchmarked separately from
  the stored-path equity pricer.

`evaluate_with_cashflows` also collects the payments made. Each
`ExpectedCashflow` reports:

- the mean undiscounted amount, in the payment currency;
- the mean present value, discounted and divided by the numeraire, in the
  local currency.

These are plain path averages of the payments.

## Martingale controls and reconciliation

The engine reduces variance with martingale control variates when:

- you selected a result variable;
- there are at least 16 paths;
- the script makes eligible discount or forward requests.

It fits the control coefficients on a separate pilot sample of up to 64 paths.
The controls are discounted bonds and forwards, whose expected values are known
from today's curves. The estimator is

\\[
\widehat V_{\rm controlled}
=\frac1n\sum_p\left[Y_p-
\widehat\beta^\mathsf T(C_p-E[C])\right].
\\]

The fitted coefficients are held fixed for differentiation. The estimate has
lower variance, but it still has sampling error; measure it.

Only the selected result variable is adjusted. Expected cashflow present values
are not. With a finite number of paths, their sum can therefore differ from the
reported result. They can match only if the selected variable is the total of
all payments. When you reconcile the two, allow for sampling error; a
deterministic comparison needs a tighter tolerance.

## From payments to exposure claims

`ScriptedProduct::contingent_claims` creates one claim per payment identifier.
Each claim has unit notional and `LongReceive`, because the script expression
already contains the amount and direction. All claims share one compiled
engine.

At an exposure date, a scripted claim reruns the script on the market data for
that date and returns its payment's value. The live-claim schedule decides
which payments are still outstanding. The market model must support every
discount, rate, FX, and spot request; claims cannot add missing dynamics.

Scripted and native claims can be in the same netting set. If the product has
an exercise rule, write it into the script. Scripted claims do not solve for
optimal exercise of callable products.

The examples in [`examples/scripting`](../../../examples/scripting) compare
native and scripted swaps on matching setups.
