# QuantSupport for Python

Python bindings for the native QuantSupport pricing and risk library. Contracts,
positions, market inputs, and valuation methods remain separate objects.

## Documentation and computational studies

The [Python user guide](https://jmelo11.github.io/quantsupport/api/python/guide/index.html)
is self-contained: installation, a complete valuation, financial conventions,
market construction, product/model selection, scenarios, and exposure/XVA.
Essential explanations do not require visiting the Book.

- [First valuation and lifecycle](https://jmelo11.github.io/quantsupport/api/python/guide/first-valuation.html)
- [Curves, market data, calibration, and history](https://jmelo11.github.io/quantsupport/api/python/guide/markets.html)
- [Bonds, loans, options, and scripting](https://jmelo11.github.io/quantsupport/api/python/guide/valuation.html)
- [Sensitivities, netting, XVA, and custom pricers](https://jmelo11.github.io/quantsupport/api/python/guide/risk.html)
- [Official computational studies](https://jmelo11.github.io/quantsupport/api/python/studies/index.html):
  rendered notebooks organized by financial topic
- [Exact API reference](https://jmelo11.github.io/quantsupport/api/autoapi/quantsupport/index.html):
  constructors/factories, parameters, defaults, properties, protocols, and errors
- [Interface capabilities and limitations](https://jmelo11.github.io/quantsupport/api/coverage.html)

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

This example needs no downloaded market files. Its curve is an explicit
continuous zero-rate assumption, not a bootstrapped market quote:

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

A 3% continuous zero curve does not imply a 3% contractual par coupon.
Rates/spreads are annual decimals unless explicitly stated otherwise. Risk
values are raw derivatives; multiply decimal-rate derivatives by `0.0001`
for an approximate +1 bp monetary change, not every sensitivity indiscriminately.
Generic swap cashflow tables report underlying amounts before leg-side
multiplication and contain no PV column.

## Important contracts

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
all fourteen studies:

```sh
python -m pip install -r bindings/python/examples/requirements-benchmark.txt
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
