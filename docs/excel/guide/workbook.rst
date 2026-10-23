Building a reproducible pricing workbook
============================================

Install and load the add-in
-------------------------------

Download the `Excel add-in and IntelliSense ZIP <https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip>`_
and extract all files into one stable local folder. It contains
``quantsupport.xll``, its matching ``quantsupport.intellisense.xml``,
``ExcelDna.IntelliSense64.xll``, ``QuantSupport.Ribbon64.xll``, installation
instructions and license notices.
The `standalone XLL <https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll>`_
is also available at the same download location.

Use 64-bit Windows Excel. Excel for macOS and
Excel in a browser cannot load this native XLL. Follow your organization's
policy for downloaded add-ins and trusted locations; if Windows marks the
downloaded file as blocked, review its source before unblocking it.

In Excel open **File → Options → Add-ins**, choose **Excel Add-ins**, select
**Go**, and browse to each of the three XLLs. Keep the XML and companions beside
``quantsupport.xll``, then restart Excel with all three add-ins enabled. Load the
add-in before opening the pricing workbook, or press **Ctrl+Alt+F9** after
loading to rebuild all formula dependencies.

The **QuantSupport** ribbon tab opens **Pricing Contexts**, **Instruments & Trades**
and **All Objects** in a side panel for the active workbook. Contexts show their
valuation date, historical snapshot timestamp in UTC, and quote side. Select an
object to see its terms, source formulas and dependencies; **Go to cell** opens
its source definition for editing. **Inspect Selection** finds the live handle
in the selected cell. The panel refreshes while Excel is ready and never starts
calculation. These dates describe the market and contract, not object creation
times. **Setup Guide** opens the worksheet construction examples.

To price an existing instrument or trade, select its handle cell and click
**Price Selection**, or select it in **Instruments & Trades** and open **Pricing**.
Choose an existing context, explicit pricer and PV, Cashflows, Fair rate or
Sensitivities, then
click **Run request**. Instrument previews use the displayed trade date and
Receive/Pay side with the instrument notional; the date initially follows the
context's valuation date. A bond preview also needs its CleanPrice, DirtyPrice
or ZSpread anchor and value. Existing trades retain their stored position terms.
Results appear in the panel without creating worksheet trade or result handles.
Sensitivities displays risk-factor labels and raw derivatives per unit of each
factor. For a decimal-rate factor, multiply by ``0.0001`` to estimate a +1 bp move.

The ribbon is optional and requires .NET Framework 4.6.2 or later. Extract the
native XLL and ribbon from the same ZIP into the same folder. Worksheet functions
also work with the native XLL alone.

Download the `swap example workbook <https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx>`_
or the `historical pricing workbook <https://jmelo11.github.io/quantsupport/downloads/historical_pricing_example.xlsx>`_.
Further notebook-style studies cover
`portfolio aggregation <https://jmelo11.github.io/quantsupport/downloads/portfolio_aggregation_example.xlsx>`_,
`loans, funding, and borrower credit <https://jmelo11.github.io/quantsupport/downloads/loan_pricing_example.xlsx>`_,
`bond anchors and risk <https://jmelo11.github.io/quantsupport/downloads/bond_pricing_example.xlsx>`_, and
`interpolation and schedules <https://jmelo11.github.io/quantsupport/downloads/interpolation_example.xlsx>`_.

Each example opens on **Start Here**, which links to the numbered tabs and
suggests three changes to try. Follow Inputs, Market, Build, then Results.
Blue cells hold editable assumptions; green cells show financial measures.
Construction sheets keep object references beside their purpose and show each
formula immediately below its step. Loan and bond Results tabs link directly
to each case's inputs and construction steps; their **Inspect one case**
dropdown controls both Cashflows and Risk. Generated results stay blank until
Excel calculates them.

Add-in loading validates every function and alias. A registration failure
displays the rejected function name and fails loading; a partially registered
catalog is rolled back.

To compile from the checkout, use a Windows PowerShell prompt with the
64-bit MSVC Rust toolchain, Visual Studio Build Tools and .NET 8 SDK or later:

.. code-block:: powershell

   rustup target add x86_64-pc-windows-msvc
   powershell -ExecutionPolicy Bypass -File bindings/excel/scripts/build.ps1

The build writes ``bindings/excel/dist/quantsupport-excel-windows-x64.zip``
and the individual XLL, XML and companion files. Packaging downloads a pinned
Excel-DNA IntelliSense release and verifies its SHA-256. Keep the files together.
The portable Sphinx
build and metadata tests do not load Excel; actual worksheet execution requires
Windows Excel.

Check the loaded version and discover functions:

.. code-block:: text

   =QS.VERSION()
   =QS.FUNCTIONS()
   =QS.FUNCTIONS.DETAILS()
   =QS.HELP("QS.INSTRUMENT.SWAP")

The catalog includes aliases; ``QS.HELP`` accepts aliases case-insensitively.
The Function Arguments dialog shows parameter names and descriptions: type a
function name and press **Ctrl+A**, or click **fx**. **Ctrl+Shift+A** inserts its
argument names into the formula.

Parameter hints while typing
----------------------------

Inline signatures and active-parameter help use the optional
`Excel-DNA IntelliSense companion <https://github.com/Excel-DNA/IntelliSense/releases>`_.
The QuantSupport ZIP includes ``ExcelDna.IntelliSense64.xll``; load it through
the same Add-ins dialog as QuantSupport. It requires .NET Framework 4.6.2 or later. Keep the
generated ``quantsupport.intellisense.xml`` beside ``quantsupport.xll``, restart
Excel with both add-ins enabled, then type ``=QS.EVALUATE`` followed by an opening parenthesis. The popup shows
the signature and highlights the argument being entered.

The build generates descriptions for every registered function and alias from
the same catalog used by Function Arguments and ``QS.HELP``. Download the ZIP
from the site or a versioned `release <https://github.com/jmelo11/quantsupport/releases>`_
to keep the XLL and XML matched. Renaming the XLL also requires renaming the XML's base name.
The companion's
`usage guide <https://github.com/Excel-DNA/IntelliSense/wiki/Usage-Instructions>`_
documents this integration. Function Arguments and its shortcuts work without
the companion.

Formula examples below use commas as separators and decimal points; adapt
separators to your Excel locale without changing financial units.

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
   H5 =QS.CONTEXT.FROM.COMPONENTS("guide.usd.context",C7,H4,,,,,,,,,,"USD","SOFR","Mid")

In :excel:func:`QS.CONTEXT.FROM.COMPONENTS`, base currency and base index are arguments
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

Aggregate a trade range
-----------------------

The portfolio workbook uses two deposits, three receive-fixed swaps and one
short swap under USD/SOFR discounting. It builds both SOFR and TermSOFR3m
projection curves from visible zero-rate inputs. Follow **3 Build** to the
contiguous ``Portfolio_Members`` range and its two aggregation formulas:

.. code-block:: text

   =QS.PORTFOLIO.CREATE("Portfolio.Book",Portfolio_Members)
   =QS.PORTFOLIO.EVALUATE("Portfolio.Result",Portfolio_Book,Portfolio_Pricer,Portfolio_Context,Portfolio_Requests,"USD")
   =QS.RESULT.VALUE(Portfolio_Result)
   =QS.RESULT.COUPONS(Portfolio_Result)
   =QS.RESULT.CASHFLOW.SUMMARY(Portfolio_Result)
   =QS.RESULT.SENSITIVITIES(Portfolio_Result)

These use the workbook's defined names; its Build tab shows the actual formulas
and object names. One pricer handle applies to every member. Alternatively,
pass a range with one pricer per member. The returned object is standard
``EvaluationResults``. PV and sensitivities require one common pricing currency;
the currency argument declares that unit and performs no FX conversion.

The deposits' default 100,000 at 4% and 300,000 at 6% combine to a 400,000 coupon
at 5.5%. Paid SOFR coupons combine at a 25 bp spread on -400,000 principal.
TermSOFR3m coupons remain distinct, as do opposing directions and incompatible
conventions. **5 Coupons** retains types, accrual dates, both contractual and
effective fixing indices, principal and weighted quotes. **6 Cashflows** is a
separate payment-date view of signed interest, capital and other payments;
it does not allocate accrual NII. **4 Results** checks PV against individually
evaluated positions, and **7 Risk** keeps the original sensitivity labels.

Handles, revisions, and calculation
-----------------------------------

Constructors return tokens such as ``QSObject:QuoteStore:w7~guide.usd.quotes:3``.
Each workbook has an isolated registry; the same names can be used in different
workbooks. Handles are runtime references, not persisted market data. Saving a
workbook saves formulas and inputs. Closing it releases its registry even if
Excel remains running.

Unchanged inputs reuse objects and results. Changed inputs advance revisions;
Excel's dependency graph recalculates dependent formulas. Readers reject stale
handles and cached objects whose inputs have changed. Mutators accept older
revisions of the same identity so a worksheet update chain can retain its
original constructor token. Always pass the final update handle to the context.
Do not edit handle text.

A worksheet cell owns its outputs and observes its input objects. Replacing or
clearing the formula releases previous outputs once no other cell references
them. Store-update cells own individual contributions: changing the quote or
curve key removes its previous entry; deleting the formula or a failed edit
removes the contribution and restores any imported baseline. Two cells defining
the same store entry fail explicitly. Keep ``Application.EnableEvents`` enabled
for edit notifications; workbook closure is also checked independently.

Object and valuation functions are nonvolatile. Manual calculation defers
ordinary input-driven recalculation; F9/Calculate explicitly runs it. Excel may
also evaluate a newly entered formula in Manual mode. On opening a saved workbook, the observer marks QuantSupport formula sheets
dirty so handles are reconstructed under the current calculation mode. It never
calls Calculate. Registry/AD operations run on
Excel's main calculation thread; pure date/interpolation functions can run on
worker threads.

For large portfolios needing only PV, use
``QS.EVALUATE.VALUE(pricer,trade,context)`` or
``QS.EVALUATE.VALUES(pricer,A2:A10001,context)``. The latter spills a numeric
column, or a row with a final TRUE argument. These functions retain no
``EvaluationResults`` objects. Share the context and pricer across positions;
use ``QS.EVALUATE`` when other requested measures must be retained.

An explicit keyed reset is also available:

.. code-block:: text

   H10 =QS.QUOTES.CREATE("guide.reset.quotes",$B$1)
   H11 =QS.OBJECT.RESET(H10,$B$2)
   H12 =QS.QUOTES.SET(H11,A4,B4,,)

Changing ``B2`` clears the mutable collection once, then downstream formulas
repopulate it. Reset supports mutable stores/configuration collections;
:excel:func:`QS.STORE.RESET` is an alias.

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
``QS.OBJECT.COUNT()`` counts identities in this workbook, while
``QS.OBJECT.COUNT(TRUE)`` counts across the Excel instance. Revisions are not
counted as separate objects.
Proceed to :doc:`markets-and-products` for alternative construction and
quotation workflows, then :doc:`risk-and-history` for reproducible comparisons.
