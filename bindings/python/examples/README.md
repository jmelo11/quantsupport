# Computational finance studies in Python

These notebooks develop quantitative finance calculations using QuantSupport.
Each study introduces a contract or market problem, states its assumptions and
uses Python calculations to interpret the native results. They assume
familiarity with the products and introduce the library as it is used.

Sixteen studies cover the Python binding. A separate historical benchmark
compares ORE/CurveEngine with recorded Rust results. Run cells in order in a
fresh kernel; saved outputs are retained for publication.

## Catalog by Rust example scope

The market and valuation folders follow the repository's Rust example scopes.
Each has its own catalog, notebooks and any scope-specific inputs. Most use an
adjacent `market_data.py` loader; portfolio and forecasting studies define their
small markets inline. The forecasting scope adds a Python orchestration layer.

| Scope | Catalog | Coverage |
| --- | --- | --- |
| `bootstrap` | [Curve construction](bootstrap/README.md) | Quotes, bootstrap configurations, nodes, pillars, and forwards |
| `valuation/swaps` | [Swaps](valuation/swaps/README.md) | First valuation, par coupon and base-currency conversion |
| `valuation/bonds` | [Bonds](valuation/bonds/README.md) | Straight-bond analytics and callable stepped bonds |
| `valuation/caps` | [Caps and caplets](valuation/caps/README.md) | Black option values and curve/volatility risk |
| `valuation/loans` | [Loans](valuation/loans/README.md) | Repayment, resets, prepayment, credit, valuation and risk |
| `volatilitysurface` | [Volatility markets](volatilitysurface/README.md) | Curves, surfaces, and independent rate/volatility valuation shocks |
| `hullwhite` | [Rate simulations](hullwhite/README.md) | Calibrated model setup and retrieving simulation results |
| `sensitivity` | [Portfolio and risk](sensitivity/README.md) | Portfolio basics, valuation requests, aggregation, quote risk, and scenarios |
| `nii_forecasting` | [Net interest income](nii_forecasting/README.md) | Accrued NII, prepayment, recursive rollover, new business and funding |
| `scripting` | [Payoff events](scripting/README.md) | Dated payoff scripts and representation checks |
| `cva` | [Exposure and XVA](cva/README.md) | Netting sets, credit/funding inputs, exposure profiles, and XVA |
| Historical external benchmark | [ORE/CurveEngine comparison](../../../examples/sensitivity/bechmark/sensitivity.ipynb) | Recorded 2025-11-11 Rust curves/PVs versus independently rebuilt markets and correctly scaled quote risk |

## Suggested reading order

| Order | Notebook | What you will learn |
| --- | --- | --- |
| 1 | [From quotes to a swap](valuation/swaps/tour.ipynb) | Dates and conventions, product/position terms, market indices, the context lifecycle, requests, and result units |
| 2 | [Par swap](valuation/swaps/par_swap.ipynb) | Solve the fixed coupon and verify zero PV |
| 3 | [Fixed-rate bond](valuation/bonds/fixed_rate_bond.ipynb) | Price/Z-spread anchors, yield, duration, payments and quote risk |
| 4 | [Caps and caplets](valuation/caps/caps_and_caplets.ipynb) | Black option values and separate rate/volatility risk |
| 5 | [Loans](valuation/loans/loans.ipynb) | Repayment, resets, prepayment, par coupon/spread, credit and risk |
| 6 | [Currency conversion](valuation/swaps/currency_conversion.ipynb) | Read FX direction, value a CLP swap under USD collateral, and reconcile its base-currency PV |
| 7 | [Curves and volatility](volatilitysurface/curves_and_volatility.ipynb) | Configure and inspect curves and surfaces, query forwards and smiles, and explain market-shock P&L |
| 8 | [Simulations and their results](hullwhite/simulations.ipynb) | Model calibration, path count/seed/grid, retrieving paths, seeds and sampling error |
| 9 | [How a portfolio works](sensitivity/portfolio.ipynb) | Build positions, use `qs.Portfolio`, reconcile PV/risk/payments, and understand membership and request rules |
| 10 | [Portfolio valuation and risk](sensitivity/portfolio_risk.ipynb) | Aggregate quote risk and payments, distinguish DV01 from vega, and check AAD against full revaluation |
| 11 | [Coupon and cashflow aggregation](sensitivity/portfolio_aggregation.ipynb) | Use `qs.Portfolio`, retain fixing indices, inspect weighted quotes, and separate interest/capital payments |
| 12 | [Callable stepped-coupon bond](valuation/bonds/stepped_coupon_bond.ipynb) | Typed terms, calibrated tree/Fourier engines, OAS and risk |
| 13 | [Scripted cashflows](scripting/scripted_cashflows.ipynb) | Dated observations/payments and the native-versus-scripted bond check |
| 14 | [Exposure and XVA](cva/xva.ipynb) | Netting sets, client terms, exposure profiles and adjustment risk |
| 15 | [Net interest income](nii_forecasting/nii_forecast.ipynb) | Monthly accrued income, prepayment, replacement cohorts, planned growth and funding scenarios |

Start with [How a portfolio works](sensitivity/portfolio.ipynb) for the
`qs.Portfolio` workflow, position sizes, common currency and request rules.
For weighted coupons and interest/capital payments, open
[`sensitivity/portfolio_aggregation.ipynb`](sensitivity/portfolio_aggregation.ipynb).
It has a [matching Excel workbook](../../excel/examples/workbooks/portfolio_aggregation_example.xlsx).

The introduction is the recommended starting point. For a closer look at curve
construction before pricing, read [bootstrap/curves.ipynb](bootstrap/curves.ipynb).
For loans, follow [the loan walkthrough](valuation/loans/loans.ipynb) from a fixed
payment schedule through floating resets, prepayment, valuation and risk.
The later notebooks repeat their own small setup, so you can also open the
subject you need directly.

## Install and run

Use a Python environment with the binding built from this checkout. From the
repository root, inside an activated virtual environment or Conda environment:

```bash
python -m pip install maturin -r bindings/python/examples/requirements.txt
maturin develop -m bindings/python/Cargo.toml --release
cd bindings/python/examples
python -m jupyterlab
```

If you need a virtual environment, create one first with `python -m venv .venv`
and activate it (`source .venv/bin/activate` on macOS/Linux, or
`.venv\Scripts\Activate.ps1` in PowerShell). Use either that environment or your
Conda environment. Select a notebook kernel using the same interpreter into
which `maturin develop` installed the extension.

Open any notebook and run its cells in order. Most studies import a local copy
of `market_data.py` with `from market_data import load_market`. That file resolves
market and configuration paths relative to its own location. The portfolio
introduction and aggregation notebooks define their small markets inline.
No helper-package installation or notebook magic is needed. The scripting
notebook also uses its adjacent event builder.

Jupyter starts each kernel in its notebook's scope folder. If your editor uses
a different working directory, select the folder containing the notebook as
the kernel working directory. The runner below sets this automatically. No
notebook invokes Cargo, fetches live data, or requires another notebook's
variables. If the native binding and the usual notebook/plotting dependencies
are already available, no installation step is needed to run the examples.

To execute the entire collection without a browser, run from the repository
root using the intended environment's Python:

```bash
python bindings/python/examples/run_notebooks.py
```

This runs the sixteen binding studies. The historical benchmark additionally
requires the optional `ORE` and `curveengine` packages; it uses its own adjacent
configuration and recorded-results JSON files. The benchmark uses ORE
1.8.14.1 and CurveEngine source revision
[`d7e000d`](https://github.com/jmelo11/curveengine/commit/d7e000da626e08ff2be5d34561597065340b017e),
which supports its `OvernightIborBasis` helpers. The published CurveEngine
1.1.6 package rejects that helper type despite sharing the source's version
number. Use Python 3.11 and install these optional dependencies into the study
environment:

```bash
python -m pip install "open-source-risk-engine==1.8.14.1" "curveengine @ git+https://github.com/jmelo11/curveengine.git@d7e000da626e08ff2be5d34561597065340b017e"
```

To run all seventeen, or only that benchmark:

```bash
python bindings/python/examples/run_notebooks.py --include-benchmark
python bindings/python/examples/run_notebooks.py benchmark
```

To execute selected notebooks and save results outside the source directory:

```bash
python bindings/python/examples/run_notebooks.py hullwhite/simulations.ipynb sensitivity/portfolio_risk.ipynb --output-dir artifacts/notebooks
```

The runner discovers notebooks in every scope and uses a fresh kernel with the
current interpreter for each notebook. It validates the notebook format and
fails on any cell error. Before executing cells, it checks the actual kernel's
Python executable, environment prefix, binding import path/version, and current
binding exports against the caller by default; a stale binding fails immediately. The
reported executable must point into your intended environment: do not resolve
a virtual environment's Python symlink to its base interpreter. Saved notebooks
record Python/binding versions and whether the kernel matched the runner,
without embedding machine-specific environment paths. A second live-kernel
probe after execution rejects any environment change before outputs are saved.
Executed copies preserve the scope folders under `--output-dir`; the external
benchmark is saved as `benchmark/sensitivity.ipynb`.
The runner leaves source notebooks unchanged by default. Pass `--write` to
refresh their stored outputs, or `--kernel NAME` to select a registered kernel
instead of the current interpreter. The callable-bond
example uses small teaching grids for its two calibrated numerical valuations
and OAS inversion; its narrative explains how to refine those grids. Build the
binding with `--release` as above for interactive pricing. The per-cell timeout
defaults to 600 seconds and can be changed with `--timeout`.

Runner regression checks use the standard-library test runner, the
checkout-built binding, and the existing notebook dependencies in
`requirements.txt` (`nbclient`, `nbformat`, and `ipykernel`, with
`jupyter_client` supplied by the Jupyter stack):

```bash
python -m unittest discover -s bindings/python/examples -p test_run_notebooks.py
```

These include two real-kernel integration checks: the default kernel must use
the invoking interpreter/binding rather than a conflicting registered
`python3` command, and an explicitly named kernel must retain its own launch
command. Isolated test kernelspecs/notebooks are created under
`artifacts/notebook-runner-tests` and removed after each test; no global kernel
registration or environment installation is performed.

## How the pieces fit

`QuoteStore` contains observations, `FixingStore` contains historical index
observations, and `FxStore` contains spot exchange rates. Most examples are
forward-looking USD products on one reference date. The currency-conversion
example also supplies USD/CLP spot FX and a CLP curve under USD collateral.
These examples do not require historical fixings. Curve and volatility
configurations describe how to construct market objects from quote identifiers.
A `MarketIndex` is the key
that connects those objects to products and models; it is not a market value.

Create a `PricingContext` and enter `with context:`. Entry validates inputs and
activates request-driven market construction and the differentiation scope;
curve, valuation, script, and XVA requests build their required dependencies.
`initialize()` explicitly constructs every configured object and is not
required for the context-manager workflow. Exiting clears constructed data
and closes the differentiation scope, including when an exception occurs. Product
specifications describe the contract and position; the native pricer supplies
the valuation method. `evaluate(trade, requests)` returns an `EvaluationResults` object with
scalar analytics and pandas tables. Save those results inside the context and
use them after it closes. Constructed curve, surface, and simulation views
should be used inside the active context. Keep contexts sequential because the
native automatic-differentiation tape is managed per thread.

Simulation configurations add a model and a path-generation setup. A raw rate
path is not a product value. Script evaluation and the XVA engine combine
models with future payments to answer different questions. Ordinary portfolio
valuation uses `context.evaluate(Portfolio("BOOK", trades), requests)` for streaming numeric aggregation;
the older risk study also shows labelled pandas reconciliation. Legal netting is
specified separately with `NettingSet`.

Python-defined instruments and pricers can also participate through
`register_pricer`; see the [binding extension guide](../README.md#extending-the-library-in-python).
The notebooks focus on native products and their existing Python entry points.

## Auxiliary files and market provenance

| File | Responsibility |
| --- | --- |
| `market_data.py` in each scope folder | Local loader imported directly by its notebooks; reads typed inputs/configurations without hidden pricing |
| [stepped_cashflows.py](scripting/stepped_cashflows.py) | Translate this tutorial's six-year semiannual note into repetitive script events; no optimal issuer exercise |
| [nii_forecast.py](nii_forecasting/nii_forecast.py) | Example classes for production rules, recursive rollover and accrued portfolio income |
| [valuation/bonds/data/bond_tree.json](valuation/bonds/data/bond_tree.json), [valuation/bonds/data/bond_fourier.json](valuation/bonds/data/bond_fourier.json) | Native pricer/model schema and numerical grids |
| [hullwhite/data/simulation.json](hullwhite/data/simulation.json) | Calibrated Hull–White simulation settings |
| [cva/data/xva.json](cva/data/xva.json) | Calibrated LGM exposure-engine settings |
| [run_notebooks.py](run_notebooks.py) | Execute the documentation against an installed binding |

Most notebooks use the **illustrative 2026-02-24** snapshot already maintained in
[`examples/hullwhite/data`](../../../examples/hullwhite/data) at the repository
root: `quotes.json`, `curve_specs.json`, and `vol_specs.json`. The
[currency-conversion notebook](valuation/swaps/currency_conversion.ipynb) uses the
USD/CLP quote and curve files in
[`examples/bootstrap/data`](../../../examples/bootstrap/data) on the same
reference date, with an explicit illustrative spot rate of 935 CLP per USD.
Keep the repository layout when running the tutorials; copying only the
notebooks omits their helpers and source market data. Nothing downloads or
updates these quotes automatically.
The [loan notebook](valuation/loans/loans.ipynb) reads a separate illustrative EUR
snapshot from [`examples/valuation/loans/data`](../../../examples/valuation/loans/data),
shared with `cargo run -p valuation-loans`.

The market contains a SOFR curve and a Black caplet surface with absolute-strike
smiles. The simulation and callable-bond examples calibrate a short-rate
volatility schedule from that surface; a Black quoted volatility and the
model's absolute sigma are different quantities.

## Reading the numbers

- Rates and spreads are decimals: `0.04` is 4%, and `1e-4` is 1 bp.
- A Black volatility of `0.30` is 30%; +1 volatility point is `0.01`.
- `result.price` is signed position PV. Bond clean/dirty anchors are quoted per
  configured units, usually per 100 principal.
- `sensitivities.value` is a derivative per unit input quote. Multiply by the
  appropriate move before reporting DV01 or vega. The notebooks retain signs.
- Cashflow amounts are undiscounted payments, with currency and leg labels.
  Native swap/bond and loan expected-receipt reports include position signs:
  receipts are positive and payments are negative. Discount these amounts and
  convert currencies as needed to reconcile PV; do not apply direction again.
- Missing/unrequested analytics are not zero and should not be silently filled
  when reporting product-level measures.

## Numerical checks and limitations

| Study | Independent numerical evidence |
| --- | --- |
| Bootstrap | Deposit repricing, forward/discount-factor identity, positive discount factors |
| Swap tour/par | Signed cashflow PV, reversed position, annuity/par formula and zero-PV repricing |
| Currency conversion | FX reciprocity, CLP/USD PV reconciliation, principal cancellation |
| Straight bond | Spread and yield PV reconstruction, duration bump, spread AAD/bump |
| Caps | Direct Black formula, put–call parity, explicit caplet-strip sum |
| Callable bond | OAS round trip, callable/underlying inequality, observed grid refinement |
| Volatility | Smile interpolation and isolation of rate versus volatility shocks |
| Simulation | Calibration residuals/domain, seeds, Gaussian Euler moment recursion |
| Portfolio introduction | Individual PV/risk/payment sums, zero-rate central differences, discounted payment PV, repeated/opposing positions |
| Portfolio risk | Rate/volatility AAD versus central differences; signed payment PVs |
| Scripting | Deterministic native representation check; repeated-seed stochastic estimates |
| Loans | Annuity payment, principal conservation, receipt PVs, par repricing, funding PV01 |
| NII forecast | Hand-calculated interest, native portfolio reconciliation, prepayment principal conservation, recursive replacement, growth/funding and balance identities |
| XVA | Signed profile identity, raw-exposure netting, margin/profile distinction |
| Historical benchmark | Date/quote-label alignment, curve/PV/rate-risk tolerances, restored inputs, explicit FX-risk-definition discrepancy |

The tolerances, seeds and grids are visible in the code. Exact algebraic
identities, numerical calibration derivatives, discretization diagnostics and
sampling checks answer different questions. In particular:

- Generated Hull–White paths use Euler steps and materialized `f64` values;
  exported paths do not carry quote-risk pillars.
- Scripted selected results may use a martingale control. Raw payment PVs need
  not sum to that control-adjusted result at finite paths.
- Exposure profiles are pointwise and pre-margin; ENE is signed negative.
  CVA/FVA use post-margin paths, and FVA integrates signed exposure.
- Scenario targets match sets of underscore-delimited segments, so even full
  identifiers can overlap. The volatility studies use one unambiguous family
  selector and verify each intended quote moves exactly once.
- The historical benchmark validates recorded results, not a newly executed
  current Rust binary. It exposes a discrepancy between recorded FX rescaling
  risk and independently propagated spot risk; those FX rows are not validated
  hedge ratios.

These studies explain and verify the implemented calculations; their teaching
settings and illustrative markets do not establish production model adequacy.
