Risk checks and historical repricing
====================================

Sensitivities estimate the effect of a small market move, while scenarios
rebuild and reprice the market under changed inputs. Historical snapshots
provide a third comparison: the same trade valued against recorded markets.
Each calculation needs consistent position, currency and quotation conventions.

The examples continue the handles from :doc:`workbook`: ``H5`` is the base
context, ``H6`` the swap, ``C7`` the final quote handle and ``H4`` the curve
configuration. Place additional formulas in clear cells and retain upstream
references when moving them to dedicated sheets.

From raw derivatives to a risk report
----------------------------------------

:excel:func:`QS.RESULT.SENSITIVITIES` spills Risk factor and Sensitivity columns.
The values are raw derivatives in the pricer's valuation units per unit
risk-factor change. A decimal interest-rate derivative becomes an approximate
+1 bp currency change after multiplication by ``0.0001``. A futures quote
derivative uses price points; a volatility quote and FX spot have their own
units. Preserve the factor label and scaling convention in any hedge report.

For a portfolio, sum only compatible values and risk factors under the same
market and currency assumptions. The signed Amount column in generic swap
cashflow reports contains undiscounted receipts and payments; discount and
convert currencies before reconciling it to PV.
Cashflows, quote sensitivities, and future-exposure profiles are different
objects; the worksheet interface does not expose the latter.

A scenario is a new constructed market
--------------------------------------

Construct the stressed market from visible bumped quotes. Put 0.0001 in
``BB1``, retaining the original quote identifiers in ``A4:A7`` and rates in
``B4:B7``. Each update below adds one basis point to a decimal-rate quote:

.. code-block:: text
   :name: guide-excel-scenario

   BB2 =QS.QUOTES.CREATE("guide.stressed.quotes",$B$1)
   BC4 =QS.QUOTES.SET(BB2,A4,B4+$BB$1,,)
   BC5 =QS.QUOTES.SET(BC4,A5,B5+$BB$1,,)
   BC6 =QS.QUOTES.SET(BC5,A6,B6+$BB$1,,)
   BC7 =QS.QUOTES.SET(BC6,A7,B7+$BB$1,,)
   BB3 =QS.CONTEXT.CREATE("guide.stressed.context",BC7,H4,,,,,,,,,,"USD","SOFR","Mid")
   BB5 =QS.EVALUATE("guide.stressed.results",L4,H6,BB3,"Value")
   BB4 =QS.RESULT.VALUE(BB5)

This workflow requires only ordinary numeric cells. The same instrument, trade,
pricer and explicit policy are reused against another constructed market.
Native scenario imports remain available for interoperability; when supplied,
their context position is argument 12. An Absolute shock adds in the input's
units; Relative multiplies by ``1 + value``. A selector such as ``SOFR``
changes matching market quotes and explicit pillars; reconstruction determines
the effect on projected payments and value. Excel materializes an imported
scenario in a separate context snapshot, unlike Python's scoped borrowed views.

Shock targets are case-sensitive underscore-token selectors, not a guaranteed
exact-match mode. After checking identifier equality, matching ignores token
order and multiplicity. A full caplet identifier with repeated tenor/expiry
tokens can select other expiries too; overlapping shocks compose without
deduplication. Use one family selector such as ``CapletFloorlet_USD_SOFR``
for one intended volatility-family shift, not one shock per full ID. Check
the affected inputs and confirm each intended quote changes once. The example
above has one SOFR shock against the first workbook's rate-only quote set.

Compare ``BB4-H7`` with the sum of the matching decimal-rate derivatives times
``0.0001``. The difference measures finite-shock nonlinearity and numerical
construction effects; options, exercise boundaries, and recalibration can make
the linear approximation less accurate. For a central-difference check, build
an otherwise identical scenario with value ``-0.0001`` and compare

.. math::

   \frac{V(q+h)-V(q-h)}{2h}
   \quad\hbox{with}\quad
   \sum_{k\ \mathrm{selected}}\frac{\partial V}{\partial q_k}.

Compare several step sizes to assess curvature and numerical noise. Inspect
the selected inputs and confirm that the base value remains unchanged.
Request support follows the selected product and pricer.

Inspecting simulations
----------------------

With a configured simulation in a context, ``QS.SIMULATION.NPATHS``, ``DT``,
``DATES``, ``PATH``, and ``CALIBRATION`` inspect its generated state.
Path indices are zero-based, and a path vector aligns with the spilled date
grid. ``DT`` is the reported year step, which can be an average for a generated
grid. Inspect diagnostics before interpreting a calibrated path distribution.

Generated paths use numeric inputs and expose no market risk pillars.
The Hull–White generator uses Euler evolution, so assess time-grid error and
sampling precision separately. These worksheet queries inspect the stored
simulation; portfolio exposure and XVA workflows are available through other
interfaces.

Archiving a coherent market
---------------------------

A snapshot records the observations and construction inputs needed to rebuild
one market. Capture a base context for each observation instant, using distinct
object names. This preserves the recorded inputs as other worksheet markets
change.

For a minimal example using the base market, place the first formula in ``BD2``:

.. code-block:: text
   :name: guide-excel-history

   BD2 =QS.SNAPSHOT.FROM.CONTEXT("guide.close.20260224","2026-02-24T16:00:00Z",H5)
   BD4 =QS.SNAPSHOTS.CREATE("guide.archive",BD2:BD2)
   BD5 =QS.SNAPSHOT.SELECTOR("guide.selection","2026-02-25T08:00:00Z","Previous","Mid")
   BD6 =QS.SNAPSHOTS.SELECT("guide.matched.snapshot",BD4,BD5)
   BD7 =QS.CONTEXT.FROM.SNAPSHOT("guide.historical.context",BD4,BD5,)
   BD8 =QS.CONTEXT.EVALUATION.DATE(BD7)
   BD9 =QS.CONTEXT.SNAPSHOT.TIMESTAMP(BD7)
   BD11 =QS.EVALUATE("guide.historical.results",L4,H6,BD7,"Value")
   BD10 =QS.RESULT.VALUE(BD11)

The requested timestamp is February 25, but ``BD8`` remains the selected
market's February 24 reference date. ``BD9`` reports the actual matched
observation instant. Repricing the unchanged trade should agree with the base
valuation because no economic input changed merely by requesting a later
selection timestamp.

For a longer archive, construct each additional market from its own recorded
quote/configuration inputs, capture it into another row, and expand the
``QS.SNAPSHOTS.CREATE`` range. That function rebuilds the archive from its full
input range, so removed rows disappear; duplicate normalized UTC timestamps are
errors. Reusing one mutable quote chain for all historical observations would
instead recapture its latest state and lose the original experiment.

``Previous`` is the default matching rule and never chooses a later observation.
``Exact`` requires equality; ``Closest`` can choose future data and breaks ties
toward the earlier instant. All selection inputs require timezone-aware text;
date-only or naive timestamps are rejected. A backtest should normally prefer
Previous and record the actual match, not silently accept look-ahead.

Quote sides, fixings, and snapshot limits
--------------------------------------------

The selector chooses Mid, Bid, or Ask for construction and model calibration.
Every consumed quote must contain that side; missing Bid/Ask does not fall back
to Mid. Explicit zero-rate inputs and FX spots are side-independent.

Snapshots exclude historical fixing stores. Pass a fixing handle separately
as the fourth argument to ``QS.CONTEXT.FROM.SNAPSHOT`` when needed. There is
no publication-vintage filtering: the workbook must supply observations known
at that historical instant, not today's complete fixing history by accident.

Snapshots preserve configuration inputs, including explicit zero/composite
curves, volatility, and model/simulation definitions. Supplied runtime curves
and materialized scenario states cannot be archived. Use construction
configurations and record the scenario separately instead.

``QS.SNAPSHOTS.TO.JSON`` exports the native archive array, and
``QS.SNAPSHOTS.FROM.JSON`` imports it. The schema is shared with Python/Rust,
but Excel exchanges cell text rather than filenames and rejects exports beyond
the 32767 UTF-16-unit cell limit. Saving only handles does not preserve a market
between Excel processes.

Interpolation as a controlled numerical operation
-------------------------------------------------

Interpolation utilities are numeric scalar functions, not market-object
constructors. On another clear input area set ``A20:A22`` to 0, 1, 2 and
``B20:B22`` to 1, 4, 1:

.. code-block:: text

   =QS.INTERPOLATE(0.5,A20:A22,B20:B22)
   =QS.INTERPOLATE.LOG.LINEAR(0.5,A20:A22,B20:B22)
   =QS.INTERPOLATE.CUBIC.SPLINE(0.5,A20:A22,B20:B22)

These produce 2.5, 2, and 3.0625. The generic function defaults to Linear.
Coordinates must be strictly increasing; values align point by point;
log-linear values must be positive. Blanks, text, booleans, duplicate
coordinates, nonfinite values, and mismatched ranges are errors rather than
missing-data imputation. Extrapolation is disabled unless explicitly enabled;
natural cubic extrapolation extends the endpoint polynomial, not a guaranteed
financially sensible tail.

Bilinear and trilinear functions accept point-coordinate ranges in arbitrary
order, including sparse slices and singleton axes. They interpolate innermost
axes first and do not complete a missing grid. Their default policy is Reject;
Flat clamps within each native slice and is not linear extrapolation.
These are general numerical operations. Their units and economic suitability
come from the values supplied by the workbook, not from the function name.

Use the :doc:`worksheet reference <../functions/index>` to check precise
signatures and errors.
