Computational studies
=====================

These notebooks develop curve construction, valuation and risk calculations
using the Python binding. Each study introduces a financial problem, states
its assumptions and interprets the calculated results. Read the
:doc:`Python guide <../guide/index>` for the conventions shared by the studies,
and use the :doc:`reference <../../autoapi/quantsupport/index>` when a declaration
needs clarification. The pages include the code, tables and figures saved
in each canonical notebook.

The pages render saved notebook outputs without executing Python during a
documentation build. To reproduce the saved results, use the companion inputs and a binding built
from the same checkout. The notebooks record their numerical assumptions,
including grids, random seeds and comparison tolerances.

Curve construction
------------------

Construct a discount curve from market quotes and examine its forwards,
zero rates and repricing checks.

.. toctree::
   :maxdepth: 1

   notebooks/curves

Swaps and collateral currencies
-----------------------------------

These studies progress from a receive-fixed position to par-rate solving and
then to cashflows, FX direction, and collateral-dependent currency conversion.

.. toctree::
   :maxdepth: 1

   notebooks/first-swap
   notebooks/par-swap
   notebooks/currency-conversion

Fixed income and loans
--------------------------

Bond quotation anchors, issuer calls, contractual amortization, and borrower
prepayment are different economic problems. The studies keep their price and
cashflow conventions separate.

.. toctree::
   :maxdepth: 1

   notebooks/fixed-rate-bond
   notebooks/callable-bond
   notebooks/loans

Rate options and volatility
-------------------------------

Inspect option inputs before interpreting a price: a rate strike, a quoted
Black volatility, and a calibrated short-rate sigma are not interchangeable.

.. toctree::
   :maxdepth: 1

   notebooks/caps-and-caplets
   notebooks/curves-and-volatility

Short-rate simulation
---------------------

The simulation study examines calibration, stored paths, seeds, and sampling
uncertainty. Materialized simulation paths do not by themselves carry
calibration or volatility sensitivities.

.. toctree::
   :maxdepth: 1

   notebooks/simulations

Portfolio risk and benchmarking
-----------------------------------

Reconcile sensitivities with full revaluation before aggregation. The legacy
benchmark is an optional external comparison, not an additional pricing API:
its ORE/CurveEngine prerequisites are separate from QuantSupport and are not
required to build this documentation or run the main tutorial sequence.
Its reported agreements for curves, PV, and rate risk do not establish a
validated FX hedge: fully propagated FX spot risk differs from the historical
JSON's ``-V/S`` rescaling derivative. The study reports that disagreement
explicitly rather than hiding it with a comparison tolerance.

.. toctree::
   :maxdepth: 1

   notebooks/portfolio-risk
   notebooks/sensitivity-benchmark

Scripted payoffs
--------------------

The scripting study compares compatible native and scripted representations.
Distinguish deterministic checks from finite-path estimators and from
control-adjusted selected result variables.

.. toctree::
   :maxdepth: 1

   notebooks/scripted-cashflows

Exposure and XVA
----------------

Compare netting and collateral assumptions, signed exposure means, and CVA/FVA.
An exposure profile is neither a potential-future-exposure quantile nor a
time-averaged exposure measure.

.. toctree::
   :maxdepth: 1

   notebooks/xva

Reproduction
------------

From the repository root, install the example dependencies into the environment
containing the freshly built native package, then run:

.. code-block:: console

   python -m bindings.python.examples.run_notebooks
   python -m bindings.python.examples.run_notebooks valuation/swaps --write

The runner starts a clean kernel for each selected study and uses that
interpreter unless a kernelspec is explicitly selected. The first command
validates without modifying source notebooks; ``--write`` refreshes stored
outputs. Before running cells it probes the actual kernel and binding exports,
rejecting stale bindings. Saved ``execution_environment`` metadata records
sanitized Python/package versions and whether the kernel matched its caller;
inspect this provenance rather than inferring identity from a launch command.
The legacy external benchmark lives outside that runner's directory
but is available through its ``benchmark`` selector. To validate all fourteen,
install the benchmark's optional requirements and include it explicitly:

.. code-block:: console

   python -m pip install "open-source-risk-engine==1.8.14.1" "curveengine @ git+https://github.com/jmelo11/curveengine.git@d7e000da626e08ff2be5d34561597065340b017e"
   python -m bindings.python.examples.run_notebooks --include-benchmark

Every rendered study links its canonical source
and companion-file directory so a downloaded notebook is not mistaken for a
self-contained market-data bundle.
