Loans, prepayment, and funding
==============================

The `loan workbook <https://jmelo11.github.io/quantsupport/downloads/loan_pricing_example.xlsx>`_
develops the same questions as the :doc:`mortgage notebook <../../python/studies/notebooks/loans>`:
how principal returns, how floating resets change payments, and how prepayment,
borrower credit, and treasury funding change lender value. Its editable zero
curves are illustrative assumptions, while the notebook bootstraps its example
curves from market quotes. Both use native loan construction and pricing.

Construct a loan
----------------

:excel:func:`QS.TRADE.LOAN` takes name, trade date, start date, maturity date,
currency, notional, rate, then optional market index, repayment, payment
frequency, day counter, side, and options JSON text. An omitted or blank market
index makes ``rate`` a fixed annual coupon; otherwise it is the floating spread.
Rates are decimals: ``0.06`` means 6%, and ``0.01`` means a 100 bp margin.

For the USD/SOFR context in ``H5`` from :doc:`workbook`:

.. code-block:: text

   D20 =QS.TRADE.LOAN("guide.loan",$B$1,$B$1,"2031-02-24","USD",250000,0.06,,"French","Monthly","Actual360","Receive")
   D21 =QS.PRICE(D20,H5)
   D22 =QS.FAIR.RATE(D20,H5)
   D23 =D21-250000

Default terms are French amortization, monthly payments, Actual365 accrual,
and lender/Receive signs. The builder fixes contractual principal at construction;
changing projected market rates does not rebuild it. Floating-first French and
balloon loans require an initial all-in ``amortization_rate`` in the options.
Floating resets and observation tenor default to index metadata, independently
of payment frequency. A daily index defaults to resetting at payment boundaries.

Loan ``QS.PRICE`` is signed PV of future receipts strictly after the valuation
date. It excludes the advance, so at origination the lender's net value is
receipt PV less notional. ``QS.FAIR.RATE`` solves the fixed annual coupon or
floating margin that lends at par. It supports uniform fixed or unbounded
floating coupons at or before origination; mixed and bounded schedules are
unsupported. Its search interval is [-5%, 50%].

Advanced options
----------------

Pass a cell containing JSON text as the final argument; filenames are not read.
Unknown fields fail rather than being ignored. For example:

.. code-block:: json

   {"balloon_fraction": 0.25, "prepayment": {"Cpr": 0.15}}

Select ``Balloon`` repayment for this example. The balloon fraction is reserved
principal due at maturity; CPR is the annual conditional prepayment fraction.

.. list-table:: Contract and pricing settings
   :header-rows: 1
   :widths: 30 70

   * - JSON field
     - Meaning / default
   * - ``balloon_fraction``
     - Required for Balloon, in [0,1]; rejected for other repayment structures.
   * - ``amortization_rate``
     - Initial all-in annual decimal rate; fixed-first schedules otherwise use their first coupon.
   * - ``prepayment``
     - Native ``"None"`` (default), ``{"Cpr":0.15}``, ``{"CprCurve":[["2026-02-24",0.1]]}``, or ``{"Refinancing":{"base_cpr":0.03,"incentive_multiplier":5,"maximum_cpr":0.5}}``.
   * - ``credit_index``, ``recovery``
     - Borrower survival index such as ``{"Credit":"BORROWER"}``; no default risk when absent. Recovery defaults to 0.4 and must be in [0,1]. Supply the credit configuration to the pricing context.
   * - ``fixing_frequency``, ``fixing_lag``, ``index_tenor``
     - Reset frequency, nonnegative business-day fixing lag (default 0), and native period text such as ``"12M"``. Past observations require a fixing store in the context.
   * - ``calendar``, ``business_day_convention``
     - Native calendar and payment adjustment, for example ``"TARGET"`` and ``"Following"``. Defaults are NullCalendar and Unadjusted; accrual dates stay fixed.
   * - ``floor``, ``cap``
     - Optional all-in floating coupon bounds. The pricer resolves bounded rates without option time value.
   * - ``introductory_periods``, ``introductory_rate``
     - Supply both: a positive count of fixed payment periods and their annual coupon. The floating rate/margin then continues through maturity.
   * - ``refinancing_benchmark_index``
     - Required by Refinancing prepayment, e.g. ``"EURIBOR12m"``; this benchmark excludes replacement-loan margins and fees.
   * - ``discount_index``
     - Override the base-currency discount curve, e.g. ``{"Other":"FUNDING"}`` for a constructed composite funding curve. Absent means the context's base index.
   * - ``rebuild_amortization_for_fair_rate``
     - Default ``false`` preserves agreed principal during fair-rate trials. ``true`` rebuilds principal per trial; it leaves Value/Cashflows/Sensitivities principal unchanged.

Inspect receipts and risk
-------------------------

.. code-block:: text

   =QS.CASHFLOWS(D20,H5)
   =QS.CASHFLOWS(D20,H5,TRUE,FALSE)
   =QS.SENSITIVITIES(D20,H5)

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
