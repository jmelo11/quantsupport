Risk, portfolios and historical snapshots
=========================================

Evaluation requests follow the Rust library's ``Request`` and
``EvaluationResults``. Direct evaluation returns one number or table. Use a
stored result when several requested measures share one evaluation:

.. code-block:: text

   H2 =QS.EVALUATE.RESULTS(context,trade,{"Value","Cashflows","Sensitivities"},pricer)
   H3 =QS.RESULT.VALUE(H2)
   H4 =QS.RESULT.SENSITIVITIES(H2)
   H5 =QS.RESULT.COUPONS(H2)
   H6 =QS.RESULT.CASHFLOW.SUMMARY(H2)

Sensitivities and cashflows
---------------------------

Sensitivities are raw derivatives in the pricer's value units per unit factor.
For a decimal-rate factor, multiply by 0.0001 for an approximate +1 bp value
change. Retain each factor's actual units; futures price points, currency spots,
volatility and rates are not interchangeable. Risk is calculated from the
original trade and model, not from a compressed cashflow report.

Cashflows contain signed undiscounted amounts in each payment currency, with
positive receipts and negative payments. The detailed coupon report retains
accrual dates, fixing and contractual indices, notional and rate conventions.
The summary groups Interest, Capital, Other and Total by payment date/currency.
It does not convert currencies or allocate accrual-period net interest income.

FairRate(index) requests one zero-based leg or loan coupon segment. Swap leg 0
solves the fixed coupon and leg 1 solves the floating spread, including all legs
and principal in the zero-PV condition. Loan segments are solved successively,
locking earlier rates. Bounded loan coupons remain unsupported for fair-rate
solving. Separate targets require separate evaluations.

.. code-block:: text

   =QS.EVALUATE(context,trade,"FairRate(0)",pricer)
   =QS.EVALUATE(context,trade,"FairRate(1)",pricer)
   =QS.REQUEST.FAIR.RATE(0)

Portfolio aggregation
---------------------

.. code-block:: text

   B2 =QS.PORTFOLIO.CREATE("book",A10:A100)
   B3 =QS.EVALUATE.PORTFOLIO(context,B2,pricer,requests,"USD")
   B4 =QS.RESULT.VALUE(B3)
   B5 =QS.RESULT.COUPONS(B3)
   B6 =QS.RESULT.CASHFLOW.SUMMARY(B3)

The portfolio holds an ordered trade-reference array. Repeated handles count as
repeated positions. Pass one pricer for all members or one per member. Value and
Sensitivities require a common monetary value currency; the currency argument
identifies those units and does not perform an FX conversion. Unsupported or
incomplete measures remain absent.

Evaluation streams one trade at a time and releases its intermediate result.
Compatible coupons are grouped using notional-weighted rates or spreads, while
pay/receive directions, nonlinear payoffs and distinct conventions remain
separate. The portfolio does not retain a combined AD graph. For individual
scalar values, use EVALUATE.RANGE with one shared committed context instead.

Archive complete dated markets
------------------------------

Prepare every root with its full contribution array. Put prepared-context
references in A2:A10 and corresponding observation timestamps in F2:F10:

.. code-block:: text

   C2 =first_root
   C3 =next_root
   H2 =QS.SNAPSHOT.FROM.RANGE("history",A2:A10,TRANSPOSE(F2:F10))
   H3 =QS.EVALUATE.FROM.SNAPSHOT(H2,trades,dates,pricer,"Value")

Observation timestamps require an explicit RFC 3339 offset, for example
2026-01-15T16:00:00Z. The selected quote-store reference date remains the actual
valuation date, even if an observation crosses a UTC day boundary. Dates and
trade references align one-for-one; blanks are rejected instead of shifting rows.
The output follows the original input order.

Defaults are Previous/Mid. Previous selects the most recent complete observation
at or before a request; Exact requires an exact match; Closest may select a future
observation and chooses the earlier one on a tie. Missing Bid/Ask never falls
back to Mid. Duplicate UTC observation timestamps fail explicitly.

For individual selection and inspection:

.. code-block:: text

   J2 =QS.SNAPSHOT.SELECTOR("selection","2026-01-16T17:00:00Z","Previous","Mid")
   J3 =QS.SNAPSHOT.SELECT("matched",H2,J2)
   J4 =QS.SNAPSHOT.TIMESTAMP(J3)
   J5 =QS.SNAPSHOT.REFERENCE.DATE(J3)
   J6 =QS.CONTEXT.FROM.SNAPSHOT("historical",H2,J2,fixings)

Snapshots store quote observations, FX and construction configurations, including
explicit zero curves, credit, volatility and simulation settings. Supplied
runtime curves cannot be captured. Fixings and their publication vintages are
separate; supply a fixing contribution or committed context to historical pricing.
No fixing-vintage filter is inferred. Native JSON import/export is available
under SNAPSHOT.FROM.JSON and SNAPSHOT.TO.JSON; Excel limits exported cell text
to 32767 UTF-16 units.

Historical batch performance
----------------------------

The native batch groups rows by matched snapshot and constructs each market
once. It returns scalar values in the original input order without creating
per-row context or evaluation-result objects. Evaluation completes synchronously
during Excel calculation.

The historical and context example workbooks show this batch API. Portable tests
verify snapshot selection, calibration reuse, input ordering and stale-handle
validation. Worksheet execution requires Windows Excel.
