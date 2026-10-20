Building a reproducible pricing workbook
============================================

Install and load the add-in
-------------------------------

Use 64-bit Windows Excel and the matching `quantsupport.xll release <https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll>`_. Excel for macOS and
Excel in a browser cannot load this native XLL. Follow your organization's
policy for downloaded add-ins and trusted locations; if Windows marks the
downloaded file as blocked, review its source before unblocking it.

In Excel open **File → Options → Add-ins**, choose **Excel Add-ins**, select
**Go**, and browse to the XLL. Keep the file at a stable location. Load the
add-in before opening the pricing workbook, or press **Ctrl+Alt+F9** after
loading to rebuild all formula dependencies.

Download the `swap example workbook <https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx>`_
or the `historical pricing workbook <https://jmelo11.github.io/quantsupport/downloads/historical_pricing_example.xlsx>`_.
Further notebook-style studies cover
`loans, funding, and borrower credit <https://jmelo11.github.io/quantsupport/downloads/loan_pricing_example.xlsx>`_,
`bond anchors and risk <https://jmelo11.github.io/quantsupport/downloads/bond_pricing_example.xlsx>`_, and
`interpolation and schedules <https://jmelo11.github.io/quantsupport/downloads/interpolation_example.xlsx>`_.

Add-in loading validates every function and alias. A registration failure
displays the rejected function name and fails loading; a partially registered
catalog is rolled back.

To compile from the checkout, use a Windows PowerShell prompt with the
64-bit MSVC Rust toolchain and Visual Studio Build Tools:

.. code-block:: powershell

   rustup target add x86_64-pc-windows-msvc
   powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1

The build writes ``bindings/excel/dist/quantsupport.xll``. The portable Sphinx
build and metadata tests do not load Excel; actual worksheet execution requires
Windows Excel.

Check the loaded version and discover functions:

.. code-block:: text

   =QS.VERSION()
   =QS.FUNCTIONS()
   =QS.FUNCTIONS.DETAILS()
   =QS.HELP("QS.INSTRUMENT.SWAP")

The catalog includes aliases; ``QS.HELP`` accepts aliases case-insensitively.
Function Arguments provides parameter hints. Formula examples below use commas
as separators and decimal points; adapt separators to your Excel locale without
changing financial units.

A complete curve-to-swap chain
----------------------------------

Create a new worksheet. Set ``B1`` to the ISO text ``2026-02-24``; format it as
Text, or enter a leading apostrophe so Excel does not convert it to a serial
date. The apostrophe is not part of the function input. All dates in this
workflow are ISO text, not locale-dependent date strings.

Enter the following market observations:

.. list-table::
   :header-rows: 1
   :widths: 10 60 30

   * - Row
     - Column A: quote identifier
     - Column B: annual decimal rate
   * - 4
     - ``FixedRateDeposit_USD_SOFR_1D``
     - ``0.04500``
   * - 5
     - ``OIS_USD_SOFR_1Y``
     - ``0.04837``
   * - 6
     - ``OIS_USD_SOFR_2Y``
     - ``0.04528``
   * - 7
     - ``OIS_USD_SOFR_5Y``
     - ``0.04077``

The displayed number may be formatted as a percentage, but its stored value
must remain decimal: 4.5% means ``0.045``, not ``4.5``. Identifiers are native
instrument quote conventions, not free-form tenor labels.

Create the store and chain quote updates in column C:

.. code-block:: text
   :name: guide-excel-market

   H2 =QS.QUOTES.CREATE("guide.usd.quotes",$B$1)
   C4 =QS.QUOTES.SET($H$2,A4,B4,,)
   C5 =QS.QUOTES.SET(C4,A5,B5,,)
   C6 =QS.QUOTES.SET(C5,A6,B6,,)
   C7 =QS.QUOTES.SET(C6,A7,B7,,)
   H3 =QS.CURVE.CONFIGS.CREATE("guide.usd.curves")
   H4 =QS.CURVE.CONFIGS.ADD(H3,"SOFR","Actual360","LogLinear",FALSE,A4:A7)
   H5 =QS.CONTEXT.CREATE("guide.usd.context",C7,H4,,,,,,,,,,"USD","SOFR","Mid")

In :excel:func:`QS.CONTEXT.CREATE`, base currency and base index are arguments
**13 and 14**, followed by the optional quote level. Keep the nine intervening
optional component arguments empty, as shown in the formula.

The quote update chain supplies Mid values; blank Bid/Ask arguments remain
absent rather than copying Mid. The context constructs/calibrates the supplied
market. Construction reports an error for a missing consumed quote or selected side,
an unsupported convention, or failed calibration.

Create a five-year receive-fixed position and request value and par coupon:

.. code-block:: text
   :name: guide-excel-swap

   L2 =QS.INSTRUMENT.SWAP("guide.usd.instrument",$B$1,"2031-02-24","USD","SOFR",10000000,0.04,0,"Actual360","Simple","Annual","Semiannual","Quarterly")
   H6 =QS.TRADE.CREATE("guide.usd.swap",L2,$B$1,"Receive")
   L3 =QS.DISCOUNT.POLICY.CSA("guide.usd.policy","USD","SOFR")
   L4 =QS.PRICER.CREATE("guide.usd.pricer","DiscountedCashflowPricer",L3)
   M2 Value
   M3 Cashflows
   M4 Sensitivities
   M5 =QS.REQUEST.FAIR.RATE(0)
   L5 =QS.EVALUATE("guide.usd.results",L4,H6,H5,M2:M5)
   H7 =QS.RESULT.VALUE(L5)
   H8 =QS.RESULT.FAIR.RATE(L5)

The contract fixes dates, coupons and indices; the trade supplies direction and
position quantity. The pricer selects a native discount policy independently.
The receive-fixed position can have negative PV when its coupon is below par.
``H7`` is monetary PV and ``H8`` is an annual decimal fixed coupon.

``FairRate(0)`` selects the fixed leg; ``FairRate(1)`` instead solves the second
leg's floating spread so total trade PV is zero, including principal exchanges,
each leg's index and native policy discounting. Request one target per evaluation.
``QS.REQUEST.FAIR.RATE`` builds the request text from a numeric index; directly
entering ``FairRate(1)`` in a request cell has the same meaning. Invalid indices
fail explicitly. Other single-quote products support only index zero.

Use ``QS.INSTRUMENT.*`` and ``QS.TRADE.CREATE`` for every exposed product family.
Combined ``QS.TRADE.<product>`` and direct valuation functions remain compatibility
shortcuts. The separated object path makes position terms, chosen pricing engine,
and discount assumptions visible as independent worksheet dependencies.

Inspecting the results
----------------------

Extract the curve once and leave room for spilled results:

.. code-block:: text
   :name: guide-excel-results

   J2 =QS.CURVE.GET(H5,"SOFR")
   J3 =QS.CURVE.DISCOUNT(J2,"2028-02-24")
   J4 =QS.CURVE.FORWARD(J2,"2027-02-24","2028-02-24",,)
   J6 =QS.CURVE.TABLE(J2,,)
   P2 =QS.RESULT.CASHFLOWS(L5)
   Z2 =QS.RESULT.SENSITIVITIES(L5)

Blank scalar curve-rate query conventions default to Simple and Annual.
``QS.CURVE.TABLE`` reports Pillar, Date, Discount factor, Zero rate, and
Forward from prior node. Discount factors are dimensionless; rates are annual
decimals. A flat runtime curve need not expose nodes and can reject the table
request even when its scalar discount query succeeds.

The cashflow spill has Payment date, Type, Amount, Fixing, Accrual, Currency,
Floor, Cap, and Leg columns. For the generic discounted-cashflow pricer,
Amount includes leg direction and reversed disbursements: receipts are positive
and payments are negative.
Principal exchanges can appear on both legs. To reconcile the amounts to
price, apply discounting and any
required FX conversion. The spill itself has no PV column.

The generic discounted-cashflow value path does not itself remove past-dated
payments. This example uses inception-date valuation; a seasoned-trade workflow
must examine the actual product/pricer treatment rather than assuming
settled flows are filtered automatically.

Sensitivities are raw derivatives with respect to named risk factors. For a
decimal-rate quote, multiplying by ``0.0001`` estimates the value change for a
+1 bp move. This is not a universal conversion for futures prices, FX spots,
or volatility quotes. Keep risk labels and units with the table.

Handles, revisions, and calculation
-----------------------------------

Constructors return tokens such as ``QSObject:QuoteStore:guide.usd.quotes:3``.
The native object remains owned by the XLL registry. Handles are valid only in
the current Excel process/calculation thread and are not persisted market data.
Saving a workbook saves formulas and inputs, not the live Rust objects.

Unchanged inputs reuse current objects. Changed constructor/mutator inputs
advance revisions; a changed upstream token makes dependent context and value
formulas recalculate. Readers reject stale handles. Mutators accept older
revisions of the same identity because a worksheet mutation chain can retain
its original upstream token while another formula has advanced the store.
Do not reconstruct this behavior by editing handle text.

Object identity is shared by type and name. Use workbook-qualified naming when
several workbooks are open; ``guide.usd.*`` is an example namespace, not an
automatically isolated workbook registry. Registry/AD functions run on Excel's
main calculation thread. Thread-safe date/interpolation functions may return
scalars or spills; thread safety does not imply scalar output.

Deleting a quote row does not issue a removal operation on an already mutated
store. Introduce a keyed reset when the input set changes:

.. code-block:: text

   H10 =QS.QUOTES.CREATE("guide.reset.quotes",$B$1)
   H11 =QS.OBJECT.RESET(H10,$B$2)
   H12 =QS.QUOTES.SET(H11,A4,B4,,)

Changing ``B2`` clears the mutable store once and lets unchanged downstream
updates repopulate it. Reset works for supported mutable stores/configuration
collections, not immutable curves, contexts, or trades. :excel:func:`QS.STORE.RESET` is
an alias. Use the reset chain instead of retaining deleted observations by
accident.

Diagnosing errors
-----------------

``#VALUE!`` generally means invalid input, an invalid/stale handle, or failed
construction/evaluation. ``#N/A`` is used by particular lookups or when a
successful evaluation did not produce the requested measure; consult that
function's reference rather than treating all lookup failures alike.
``#SPILL!`` is Excel's obstruction of an array result: clear the destination
area instead of changing a pricing assumption.

Check dates, decimal units, exact index/calendar names, optional argument
positions, selected quote side, and handle revisions before changing numerical
parameters. ``QS.OBJECT.TYPE`` can confirm a live object's type;
``QS.OBJECT.COUNT`` counts identities, not historical revisions.
Proceed to :doc:`markets-and-products` for alternative construction and
quotation workflows, then :doc:`risk-and-history` for reproducible comparisons.
