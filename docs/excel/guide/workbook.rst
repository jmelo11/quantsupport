Install and build a pricing workbook
====================================

The add-in requires 64-bit Windows desktop Excel. Download the complete Excel
ZIP from the project's download page, extract it into a stable folder and load
``quantsupport.xll``, ``QuantSupport.Ribbon64.xll`` and
``ExcelDna.IntelliSense64.xll`` through Excel's Add-ins dialog. Keep
``quantsupport.intellisense.xml`` alongside the XLLs. The managed companions
require .NET Framework 4.6.2 or later. Restart Excel after loading them.

Use workbooks and add-ins from the same release. Old store constructors,
product-specific trade shortcuts and aliases have been removed. The repository's
seven generated examples use the context API. Downloading a released workbook
while testing an unreleased native build can produce unknown-function errors.

Start with a coherent dated market
----------------------------------

A context owns its stores and construction configuration. Put the valuation
date in B1, quote identifiers in A10:A20 and Mid levels in F10:F20:

.. code-block:: text

   B2 =QS.CONTEXT.CREATE("usd",B1,"USD","SOFR")
   B3 =QS.CONTEXT.ADD.QUOTES(B2,A10:A20,F10:F20)
   B4 =QS.CONTEXT.ADD.CURVE(B2,"SOFR")
   B5 =B2

Base currency and index describe collateral assumptions used for construction.
For example, EUR OIS in a USD collateral context needs the appropriate FX
collateral curve. Use a separate EUR/ESTR context for an independent EUR market;
the swap example demonstrates both markets without mixing their assumptions.

Quote identifiers encode instrument conventions and tenors. Decimal interest
rates use 0.04 for 4%; a futures price such as 95.25 is in price points.
Bid and Ask remain absent when omitted. Selecting a missing quote side is an
error rather than an implicit fallback or an average of other sides.

Define the contract, then its position
--------------------------------------

.. code-block:: text

   D2 =QS.INSTRUMENT.SWAP("swap",B2,"2026-01-15","2031-01-15","USD","SOFR",1000000,0.04,0,"Actual360","Simple","Annual","Semiannual","Quarterly")
   D3 =QS.TRADE.CREATE("position",B2,D2,B1,"Receive")
   H2 =QS.DISCOUNT.POLICY.CSA("csa","USD","SOFR")
   H3 =QS.PRICER.CREATE("pricer","DiscountedCashflowPricer",H2)
   H4 =QS.EVALUATE(B5,D3,"Value",H3)
   H5 =QS.EVALUATE(B5,D3,"FairRate(0)",H3)
   H6 =QS.EVALUATE.RESULTS(B5,D3,{"Value","Cashflows","Sensitivities"},H3)

The contract carries dates, rates, indices and notional. The position adds trade
date, direction, permitted quantity overrides and bond pricing anchors. A policy
chooses ordinary discount curves; the pricer chooses the native valuation model.
These are separate concerns in both the workbook and the Rust library.

Result views and financial units
--------------------------------

.. code-block:: text

   =QS.RESULT.VALUE(H6)
   =QS.RESULT.CASHFLOWS(H6)
   =QS.RESULT.COUPONS(H6)
   =QS.RESULT.SENSITIVITIES(H6)
   =QS.RESULT.CASHFLOW.SUMMARY(H6)
   =QS.CURVE.GET(B5,"SOFR")

Cashflows are signed undiscounted amounts in each row's currency. Positive
amounts are receipts and negative amounts payments. PV follows the native
pricer's valuation units. Sensitivities are raw derivatives per unit factor;
scale decimal-rate derivatives by 0.0001 to estimate a +1 bp change.
The coupon view preserves accrual dates, rate conventions and fixing indices.
The summary separates Interest, Capital, Other and Total by payment date and
currency; it neither allocates accrual-period income nor converts FX.

Tables accept trailing transpose and headers flags, defaulting to FALSE/TRUE.
Leave enough empty cells for dynamic arrays. An obstructed spill is an Excel
layout problem and does not require rebuilding native market data.

Use the ribbon and panel
------------------------

Market groups contexts and curves; Pricing opens an instrument or trade request.
Workbook provides object browsing, selection inspection and refresh. Calculation
contains Reset Memory & Rebuild. Setup & help links to the guide and reference.

The panel uses labeled form rows and resizable virtual grids. Ctrl+A selects all
rows and Ctrl+C copies values with headings. The copy menu also supports the
whole table. Numbers follow Excel's decimal separator, including custom settings.
Details show financial data; Copy handle and Go to cell provide explicit access
to the underlying worksheet without a method trace in the normal display.

Object, context and pricer selectors open searchable lists. Search by name,
type or valuation date, then click a row or press Enter. Escape closes the list
without changing the selection. These lists retain all objects in large books.
Refreshing shows a loading message while browser data is decoded in the
background; repeated refresh clicks share the pending load.

When a native worksheet function fails, the panel reports its source sheet and
cell with the library message. Correcting the input or clearing the formula
removes the message. Pricing queries require an explicit Run request; ordinary
browsing never recalculates the workbook. Changed dependencies require worksheet
recalculation before they can be priced safely.

Build and verification
----------------------

On Windows use stable Rust, Visual Studio Build Tools and .NET SDK 8 or later:

.. code-block:: powershell

   powershell -ExecutionPolicy Bypass -File tools/excel/build.ps1

The distribution includes the native XLL, ribbon, pinned IntelliSense companion,
XML metadata and licenses. On any platform regenerate examples with
``python tools/excel/examples/create_examples.py``. Portable tests validate
native markets, ownership and workbook formulas; live loading and panel
rendering require Windows Excel. See :doc:`risk-and-history` for historical
snapshot selection and batch evaluation.

Downloads
---------

* `Complete Excel ZIP <https://jmelo11.github.io/quantsupport/downloads/quantsupport-excel-windows-x64.zip>`_
* `Native XLL <https://jmelo11.github.io/quantsupport/downloads/quantsupport.xll>`_
* `Swap workbook <https://jmelo11.github.io/quantsupport/downloads/swap_pricing_example.xlsx>`_
* `Historical workbook <https://jmelo11.github.io/quantsupport/downloads/historical_pricing_example.xlsx>`_
* `10,000 valuations in one batch <https://jmelo11.github.io/quantsupport/downloads/origination_10000_batch.xlsx>`_

Development workbooks under ``bindings/excel/examples/workbooks`` should be used with a
locally built development add-in; these hosted downloads track published builds.
