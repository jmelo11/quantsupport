# QuantSupport for Python

This Python package provides QuantSupport's native instruments, market
construction, valuation and risk calculations. Define a contract and trade,
configure its market, then evaluate the position inside a pricing context.
Results are available as scalar properties and pandas tables.

## Documentation and computational studies

The [Python user guide](https://jmelo11.github.io/quantsupport/api/python/guide/index.html)
is self-contained: installation, a complete valuation, financial conventions,
market construction, product/model selection, scenarios, and exposure/XVA.
Each workflow includes the financial conventions needed to interpret its
results.

- [First valuation and lifecycle](https://jmelo11.github.io/quantsupport/api/python/guide/first-valuation.html)
- [Curves, market data, calibration, and history](https://jmelo11.github.io/quantsupport/api/python/guide/markets.html)
- [Bonds, loans, options, and scripting](https://jmelo11.github.io/quantsupport/api/python/guide/valuation.html)
- [Sensitivities, netting, XVA, and custom pricers](https://jmelo11.github.io/quantsupport/api/python/guide/risk.html)
- [Official computational studies](https://jmelo11.github.io/quantsupport/api/python/studies/index.html):
  rendered notebooks organized by financial topic
- [Exact API reference](https://jmelo11.github.io/quantsupport/api/autoapi/quantsupport/index.html):
  constructors/factories, parameters, defaults, properties, protocols, and errors

Public classes are grouped in `quantsupport.time`, `.market`, `.instruments`,
`.pricing`, and `.risk`, and re-exported at the root for compatibility.
`quantsupport.Swap` and `quantsupport.instruments.Swap` are the same class.

## Install

Released wheels require Python 3.9 or later and install pandas as a dependency:

```sh
python -m pip install quantsupport
```

Use `quantsupport[typing]` for optional editor typing dependencies. A wheel does
not require Rust. For a development build from the repository root:

```sh
python -m venv .venv
. .venv/bin/activate
python -m pip install "maturin>=1.5,<2.0"
maturin develop -m bindings/python/Cargo.toml --release
```

On Windows activate `.venv\Scripts\Activate.ps1`. Source builds require the
stable Rust toolchain. Select this same interpreter in Jupyter; an older
installed wheel can lack classes present in the checkout.

## An in-memory valuation

This example defines a five-year receive-fixed swap and an explicit flat
continuous zero-rate curve using in-memory inputs:

```python
import quantsupport as qs

reference = qs.Date(2026, 2, 24)
quotes = qs.QuoteStore.from_dict({
    "reference_date": str(reference), "quotes": [],
})
curve = qs.CurveConfiguration(
    market_index="SOFR", day_counter="Actual360", interpolator="Linear",
    construction=qs.CurveConstruction.zero_rate(
        pillars=["0D", "10Y"], rates=[0.03, 0.03],
    ),
)
context = qs.PricingContext(
    quotes, [curve], csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
)
swap = qs.Swap(
    "USD-IRS", reference, reference + "5Y",
    10_000_000, 0.03, "USD", "SOFR",
    fixed_leg_frequency="Semiannual", floating_leg_frequency="Quarterly",
)
position = qs.Trade(swap, reference, qs.Side.LongReceive)
with context:
    result = context.evaluate(position, ["Value", "FairRate", "Sensitivities"])
    print(result.price, result.fair_rate)
    print(result.sensitivities)
```

The continuous curve implies simple forwards slightly above 3%, so the
receive-fixed swap has a small negative value. Rates and spreads use annual
decimals unless specified otherwise. Risk values are derivatives per unit
input; multiply decimal-rate derivatives by `0.0001` for the approximate
monetary change from a one-basis-point increase.
Generic swap cashflow tables report signed amounts, positive for receipts and
negative for payments, including initial advances. They contain no PV column.

## Interface conventions

For portfolio aggregation, use the same explicit trades and receive standard
`EvaluationResults`:

```python
book = qs.Portfolio([position, another_position])
with context:
    result = book.evaluate(
        context, ["Value", "Sensitivities", "Cashflows"], value_currency="USD",
    )
    print(result.price)
    print(result.sensitivities)
    print(result.coupons)  # type, market_index, fixing_index, dates, notional, rate/spread, conventions
    print(result.cashflow_summary)  # payment_date, currency, interest, capital, other, total
```

Open the complete [portfolio aggregation notebook](examples/sensitivity/portfolio_aggregation.ipynb)
for an editable deposit/swap book, weighted quotes, distinct fixing indices,
individual-position reconciliation and optional CSV exports.

The portfolio prices one member at a time, immediately summing completed numeric
results and duplicate risk factors. Built-in pricers reuse the per-trade AD tape;
no portfolio-wide AD graph or list of individual valuation results is retained.
`value_currency` identifies the common pricing currency of member PVs and raw
derivatives; it performs no FX conversion. Cashflow currencies remain separate.
Unsupported aggregate measures remain absent, as does any measure omitted by a
member. Repeated entries count as repeated positions. Cashflows preserve coupon
types, contractual and effective fixing indices (including leg overrides),
accrual dates, day counts, overnight conventions and payoffs.
Compatible fixed coupons use notional-weighted rates; compatible linear floating
coupons use notional-weighted spreads. Pay and receive directions remain separate;
daily-compounded spreads and different option payoffs are kept distinct. Coupons
without contractual metadata remain individual rows. `result.coupons` exposes
these contractual terms; historical payments are retained. The separate summary
view separates net coupon interest from signed redemptions and
advances, leaving unknown payment types in Other. It does not allocate accrual NII.

Fixed-rate deposits already use the shared Rust `MakeFixedRateDeposit` /
`FixedRateDeposit` instrument and are exposed as `qs.FixedRateDeposit` (also
`qs.instruments.FixedRateDeposit`). Create a `qs.Trade` to place one in a portfolio.

`BondPricer` leaves `FairRate` absent. For bond price/spread anchors, use
`YieldToMaturity`, `ModifiedDuration`, `ZSpread` or the selected model's `OAS`
request instead.

- Built-in instruments contain terms, not positions. `Trade` owns execution
  date, side, size overrides, and bond anchors. `evaluate()` and `NettingSet`
  require explicit trades. Do not put `side` or `trade_date` in product dicts.
- Context entry validates and enables lazy construction. `initialize()` eagerly
  constructs all configured objects. Final scope exit releases constructed
  state; do not retain AD-backed views after context/scenario exit.
- Context collection/store getters return copies. Assign changed inputs back
  to invalidate dependent construction.
- Many financial inputs accept enums or names, but typed shock/calendar/
  schedule parameters retain their documented restrictions. For equity use
  `qs.MarketIndex.equity("SPX")` or `{"Equity": "SPX"}`, not `"EQ:SPX"`.
- Python `from_json(path)` reads a file. Configuration `from_dict` returns one
  configuration; documented list loaders return lists. Product dictionaries
  instead follow constructor keywords.
- Bond trades require `pricing_anchor`: clean/dirty prices use quotation units;
  straight bonds use Z-spread, issuer-call bonds OAS. Discounting is a policy
  choice, not an automatic inference from currency.
- Loan Value is future expected receipt PV, excluding the initial advance.
  Loan fair-rate and prepayment choices are explained in the guide.
- Python scripting returns averaged numeric variables, not sensitivity/
  cashflow tables. Rate models are explicit; finite-path estimator comparisons
  need not reconcile raw cashflow PVs with a control-adjusted selected variable.
- High-level XVA produces CVA/FVA for the documented contingent-claim subset.
  ENE is signed; profiles are pre-margin pointwise means, not PFE quantiles.
  Curve-driven adjustment risk uses survival/derived funding-spread pillars,
  not original CDS/funding quotes; those aggregator paths do not replay
  the original bootstrap Jacobians.
- Historical selection preserves one complete market's valuation date.
  Fixings are separate and not filtered by publication vintage.

## Running the studies and tests

The [examples guide](examples/README.md) gives environment setup and reading
order. From the repository root, after building this checkout's binding:

```sh
python -m pip install -r bindings/python/examples/requirements.txt
python -m bindings.python.examples.run_notebooks
python -m bindings.python.examples.run_notebooks valuation/swaps --write
python -m unittest discover -s bindings/python/tests
```

The runner uses a fresh kernel per notebook, verifies the actual interpreter
and binding exports, and rejects stale bindings before executing cells.
Saved sanitized `execution_environment` metadata records Python/package
versions and whether the kernel matched its caller. Without `--write` it leaves
sources unchanged; Sphinx renders saved outputs without executing notebooks.
The legacy external sensitivity benchmark has additional ORE/CurveEngine
prerequisites and is not part of that runner's default directory. To validate
all fifteen studies:

```sh
python -m pip install "open-source-risk-engine==1.8.14.1" "curveengine @ git+https://github.com/jmelo11/curveengine.git@d7e000da626e08ff2be5d34561597065340b017e"
python -m bindings.python.examples.run_notebooks --include-benchmark
```

Use the `benchmark` selector to run it alone.

Native harness checks include:

```sh
cargo check -p quantsupport-py
cargo test -p quantsupport-py python_dict_and_script_contracts -- --nocapture
```

Do not enable `extension-module` for Rust tests. Set `PYO3_PYTHON` when Cargo
needs a particular interpreter; Windows linking/loading also requires its
runtime DLL directories on `PATH`. New native product implementations belong
under `bindings/python/src/instruments/` with dispatch in `src/pricing/`.

See the [documentation build guide](../../docs/README.md) for standalone
Sphinx builds, which require neither this native module nor notebook execution.
