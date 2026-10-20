Loans, prepayment, and funding
==============================

The `loan workbook <https://jmelo11.github.io/quantsupport/downloads/loan_pricing_example.xlsx>`_
develops the same questions as the :doc:`mortgage notebook <../../python/studies/notebooks/loans>`:
how principal returns, how floating resets change payments, and how prepayment,
borrower credit, and treasury funding change lender value. Its editable zero
curves are illustrative assumptions, while the notebook bootstraps its example
curves from market quotes. Both use native loan construction and pricing.

Open **Start Here**, then follow the numbered tabs. **1 Inputs** contains shared
terms, **2 Cases** keeps each agreement's settings and coupon schedule together,
and **5 Results** compares the twelve cases. Use that tab's **Inspect one case**
dropdown to select the agreement shown on both Cashflows and Risk; each result
row also links to its inputs and build steps.

Construct a loan
----------------

:excel:func:`QS.INSTRUMENT.LOAN` defines the contract: name, start date,
maturity date, currency, notional, rate, optional market index, repayment,
payment frequency, day counter, then settings, coupons, and prepayments ranges.
A blank market index makes ``rate`` a fixed annual coupon; otherwise it is a
floating spread. Rates are decimals: 0.06 means 6%, 0.01 means a 100 bp margin.

Create the position, discount policy and pricer separately. With the USD/SOFR
context ``H5`` from :doc:`workbook`, enter USD in ``K20`` and SOFR in ``L20``;
put Value, Cashflows, and Sensitivities in ``K23:K25``:

.. code-block:: text

   D20 =QS.INSTRUMENT.LOAN("guide.loan",$B$1,"2031-02-24","USD",250000,0.06,,"French","Monthly","Actual360")
   D21 =QS.TRADE.CREATE("guide.loan.trade",D20,$B$1,"Receive")
   D22 =QS.DISCOUNT.POLICY.FIXED.INCOME("guide.loan.policy",K20:L20)
   D23 =QS.PRICER.CREATE("guide.loan.pricer","LoanPricer",D22)
   K26 =QS.REQUEST.FAIR.RATE(0)
   D25 =QS.EVALUATE("guide.loan.results",D23,D21,H5,K23:K26)
   D26 =QS.RESULT.VALUE(D25)
   D27 =QS.RESULT.FAIR.RATE(D25)
   D28 =D26-250000

Default contractual terms are French amortization, monthly payments and
Actual365 accrual. Position side is a trade input. Principal belongs to the
loan contract; a different position principal requires another contract.
Floating-first French and balloon contracts require an initial all-in
``Amortization rate``. Reset and observation tenor default to index metadata,
independently of payment frequency; daily indices reset at payment boundaries.

Loan value is signed PV of future receipts strictly after valuation, excluding
the advance. Net lender value at origination subtracts the original advance.
``FairRate(index)`` selects a **zero-based contiguous rate segment** and returns
one scalar ``fair_rate``. Changes in fixed rate, spread, coupon type or index
start a segment; projected index resets do not.

For target 1, the solver first shortens the loan to segment 0's end and repays
remaining capital there. It solves and locks that coupon/spread, extends to
segment 1's end, repays remaining capital, then solves segment 1. Only the
selected segment's scalar is returned. The original contract stays unchanged.
Use a separate evaluation for each target. Invalid indices fail explicitly;
bounded schedules remain unsupported and each search uses [-5%, 50%].

Worksheet settings
------------------

The optional 11th argument is a two-column ``Setting | Value`` range. A header
row is optional; empty rows and blank values retain defaults. Setting names
ignore capitalization, spaces and underscores. Numbers must be numeric cells;
flags use TRUE/FALSE or 0/1. Unknown or duplicate names fail explicitly.

For example, put these cells in ``A30:B32`` and pass that range as ``settings``:

.. list-table:: Example settings
   :header-rows: 1

   * - Setting
     - Value
   * - Balloon fraction
     - 0.25
   * - Annual CPR
     - 0.15

Select Balloon repayment for this example. The balloon fraction is reserved
principal due at maturity; Annual CPR is the conditional prepayment fraction.

.. list-table:: Available settings
   :header-rows: 1
   :widths: 35 65

   * - Setting
     - Meaning / default
   * - Balloon fraction
     - Required for Balloon, in [0,1]; rejected for other repayment structures.
   * - Amortization rate
     - Initial all-in annual decimal rate for agreed principal. Fixed-first schedules otherwise use their first coupon.
   * - Annual CPR
     - Constant annual conditional prepayment rate in [0,1]. Omit for no prepayment.
   * - Base CPR, Incentive multiplier, Maximum CPR
     - Supply all three for refinancing-incentive prepayment, plus a benchmark on the pricer. Cannot combine with Annual CPR or a dated CPR range.
   * - Prepayment model
     - Optional None, CPR, CPRCurve, or Refinancing; normally inferred from the supplied inputs. Conflicting inputs fail.
   * - Credit index, Recovery
     - Borrower index text such as ``Credit:BORROWER``. Recovery defaults to 0.4 and must be in [0,1]. Supply credit configurations to the context.
   * - Fixing frequency, Fixing lag, Index tenor
     - Reset frequency, nonnegative whole business-day lag (default 0), and period text such as 12M. Past observations require a fixing store.
   * - Calendar, Business day convention
     - For example TARGET and Following. Defaults are NullCalendar and Unadjusted; accrual dates stay fixed.
   * - Floor, Cap
     - Optional all-in floating coupon bounds; projection excludes option time value.
   * - Introductory periods, Introductory rate
     - Optional shortcut for fixed-then-floating loans: supply both a positive whole payment count and the annual introductory coupon.

Pricer settings and discounting
-------------------------------

The pricer's optional Setting/Value range accepts ``Refinancing benchmark index``
and ``Rebuild amortization for fair rate``. The former is required for
refinancing-incentive prepayment and excludes replacement-loan margins and fees.
The latter defaults to FALSE, preserving agreed principal during fair-rate
trials; TRUE rebuilds trial principal from the first trial coupon's all-in rate.
Neither setting changes the original loan.

Funding is an ordinary curve. Construct FUNDING with the same curve APIs used
for SOFR or an optional composite, as in :doc:`markets-and-products`. Set a
policy's USD mapping to FUNDING and pass that policy to another LoanPricer.
Reuse the same contract and trade to compare funding assumptions. Discount
selection is a native policy responsibility, with no Discount index loan setting.

Coupon schedule range
---------------------

The optional 12th argument is a three-column coupon table. Put the following
in ``E30:G33`` (including its optional header row):

.. list-table:: Coupon segments
   :header-rows: 1

   * - Periods
     - Rate / spread
     - Market index
   * - 12
     - 0.03
     -
   * - 12
     - 0.04
     -
   * -
     - 0.01
     - SOFR

This defines 12 fixed payments at 3%, 12 at 4%, then SOFR plus 1% through
maturity. A blank index means fixed; a blank final count fills the remaining
payments. All earlier counts must be positive whole numbers. Supplied counts
must cover the schedule. The coupon table replaces the scalar rate/index inputs
and cannot be combined with introductory settings or floor/cap.

.. code-block:: text

   D40 =QS.INSTRUMENT.LOAN("guide.stepped",$B$1,"2031-02-24","USD",250000,0.03,,"French","Monthly","Actual360",,E30:G33)
   D41 =QS.TRADE.CREATE("guide.stepped.trade",D40,$B$1,"Receive")
   D42 =QS.EVALUATE("guide.stepped.second",D23,D41,H5,"FairRate(1)")
   D43 =QS.RESULT.FAIR.RATE(D42)
   D44 =QS.EVALUATE("guide.stepped.third",D23,D41,H5,"FairRate(2)")
   D45 =QS.RESULT.FAIR.RATE(D44)

``D43`` returns the second segment's fixed coupon and ``D45`` the third segment's
floating spread. They are ordinary scalar cells and can be placed in any layout.
The USD/SOFR context supplies the projection curve for this schedule.

Dated prepayments
-----------------

The optional 13th argument is a two-column ``Date | CPR`` range. Dates are ISO
text, such as 2026-02-24; annual CPRs are numeric fractions in [0,1]. Dates must
increase strictly. Headers and blank rows are optional. Before the first date,
CPR is zero; after the last, it remains at the last supplied rate. This range
cannot be combined with Annual CPR or refinancing settings.

Borrower credit from cells
--------------------------

Create a configuration collection, then add a credit curve using a range of CDS
quote identifiers already present in the quote store:

.. code-block:: text

   D50 =QS.CREDIT.CURVE.CONFIGS.CREATE("guide.credit")
   D51 =QS.CREDIT.CURVE.CONFIGS.ADD(D50,"Credit:BORROWER","USD","SOFR",0.4,J30:J36)

Pass ``D51`` as the context's credit-configuration input and use
``Credit index | Credit:BORROWER`` in the loan settings. Optional premium
conventions default to Actual360 and Quarterly. The loan workbook demonstrates
this complete workflow using ordinary numeric and text cells.

Inspect receipts and risk
-------------------------

.. code-block:: text

   =QS.RESULT.CASHFLOWS(D25)
   =QS.RESULT.CASHFLOWS(D25,TRUE,FALSE)
   =QS.RESULT.SENSITIVITIES(D25)

Cashflows use leg 0 for scheduled receipts, 1 for prepayments, and 2 for
recovery. ``Pay`` reverses signs. Sensitivities are derivatives per unit market
pillar change; multiply decimal-rate derivatives by ``0.0001`` to estimate a
+1 bp move. The loan workbook compares funding-curve and benchmark exposures.

Table layout
------------

Table functions take trailing ``transpose`` and ``headers`` parameters.
Both accept TRUE/FALSE; omitted or blank inputs default to FALSE and TRUE,
respectively. ``headers=FALSE`` removes the header row first, then
``transpose=TRUE`` exchanges rows and columns while preserving cell types.
For a nine-field cashflow table, this produces nine rows with one column per
cashflow. An empty result returns a blank cell.

The controls apply to cashflows, sensitivities (including bond risk), curve
tables, simulation calibration diagnostics, the function catalog, and help.
Date lists, context index lists, historical timestamps, and simulation paths
have no headers and expose only ``transpose``.
