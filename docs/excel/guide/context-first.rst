Create a context and price directly
============================================================

Start with ``context_pricing_example.xlsx`` under ``bindings/excel/examples``.
It uses one root context, independent market inputs, and direct pricing.
Load the matching XLL in 64-bit Windows Excel first; see the
:doc:`installation instructions <workbook>`.

Put an ISO market date in B1 and Identifier/Mid quote rows in A3:B6. For example,
use ``FixedRateDeposit_USD_SOFR_1D`` and ``OIS_USD_SOFR_1Y``, ``_2Y``, ``_5Y``
with full identifiers and annual decimal rates.

.. code-block:: text

   H2 =QS.CONTEXT.CREATE("market",$B$1,"USD","SOFR")
   H3 =QS.CONTEXT.ADD.CURVE(H2,"SOFR",A3:B6)
   H5 =QS.INSTRUMENT.SWAP("swap",$B$1,"2031-02-24","USD","SOFR",10000000,0.04,0,"Actual360","Simple","Annual","Semiannual","Quarterly")
   H6 =QS.CONTEXT.PRICE(H2,H5,"Value",H3)
   H7 =QS.CONTEXT.PRICE(H2,H5,"FairRate(0)",H3)
   H8 =QS.CONTEXT.PRICE(H2,H5,"Cashflows",H3)

The curve input contains both its construction conventions and its quotes.
There is no separate curve-configuration collection to create or update.
``Cashflows`` and ``Sensitivities`` return tables directly; scalar measures
return numbers. No evaluation handle or result-reader formula is required.
Use separate areas for spilling tables.

Independent additions and Excel dependencies
------------------------------------------------------------

Each addition references the same root. A quote table can feed several curves:

.. code-block:: text

   H3 =QS.CONTEXT.ADD.QUOTES(H2,A3:B6)
   H4 =QS.CONTEXT.ADD.CURVE(H2,"SOFR")
   H6 =QS.CONTEXT.PRICE(H2,H5,"Value",H3:H4)

An omitted curve table selects the context quotes matching that curve's market
index, excluding volatility quotes. A supplied table defines the curve's exact
quote basket. Day count, interpolation and extrapolation are optional curve
arguments; defaults are Actual360, LogLinear and FALSE.

The complete additions range is an explicit argument to pricing. Excel must
see those cells as dependencies to order calculations and recalculate after an
edit. Additions describe inputs without changing the root as a hidden side
effect. Reordering them does not change the market; removing an addition from
the pricing range removes it from that calculation. Duplicate input handles,
quote identifiers, curve indices, or inputs belonging to another root fail
explicitly. Blank range cells are ignored.

Other input functions accept fixing histories, FX rows, credit curves, explicit
zero curves, composite curves, volatility surfaces and cubes. Use
``QS.CONTEXT.ADD.FIXINGS(root,range)`` with Market index/Date/Value columns.
Supply one complete fixing history per context. FX columns are Base/Quote/Rate.
Credit curves take an explicit currency and discount index with their CDS
quotes. Zero curves take ordered pillars and annual decimal rates; composite
curves identify their base and spread indices in the same context.

Contract terms and pricing choices
------------------------------------------------------------

Direct contract pricing uses Receive, the contractual notional and the context
date. Pass a trade when side, trade date, quantity or a bond pricing anchor must
be explicit. Default pricing uses the product's native pricer and the context
base discount curve. An optional pricer argument chooses another discount or
model policy. Swaptions require an explicit configured pricer.

``QS.LOAN.SETTINGS`` exposes optional contractual parameters in IntelliSense:
balloon and amortization rates, credit and recovery, fixing conventions,
calendar, coupon bounds, introductory periods and prepayment parameters.
Pass its handle to the loan constructor's settings argument. Existing
Setting/Value inputs remain compatible.

``QS.LOAN.PRICER(name,currency,discount_index,refinancing_index,rebuild_amortization)``
exposes the loan pricer's optional controls without a settings table or a
separate worksheet discount-policy object. Funding remains an ordinary market
curve selected by the pricer.

Historical studies with many instruments
------------------------------------------------------------

Capture a complete market with its additions range:

.. code-block:: text

   =QS.SNAPSHOT.FROM.CONTEXT("market.close","2026-02-24T16:00:00Z",H2,H3:H4)

Build an archive with ``QS.SNAPSHOTS.CREATE`` and price instrument ranges:

.. code-block:: text

   =QS.HISTORY.VALUES(archive,instruments,origination_timestamps,loan_pricer)

The instrument and timestamp ranges align one-for-one in row-major order.
Blank positions fail instead of shifting the correspondence. Timestamps require
an explicit UTC offset. The batch constructs each distinct matched market once
and spills scalar values in input order. Optional arguments select a scalar
request, fixing history, matching rule, quote side and transpose.

Previous/Mid are the defaults. Previous selects the latest market at or before
the timestamp. The selected market's actual reference date remains the valuation
date; curves are not silently rolled to the requested origination date.
Fixings are separate from snapshots and are not automatically filtered by
publication vintage.

Separately named ``QS.CONTEXT.FROM.SNAPSHOT`` handles share calibrated native
contexts when archive revision, matched snapshot, quote side and fixing revision
match. ``QS.CONTEXT.VALUES`` also prices an instrument range against one prepared
context. Neither batch retains evaluation objects or per-row preview trades.
Prepared declarative contexts use a bounded cache; historical shared contexts
are released when their worksheet owners and active batches release them.

These functions execute synchronously. RTD and background completion have not
been implemented. Worker isolation, recalculation cancellation and workbook
closure must be handled before moving registry or AD operations off the main
Excel calculation thread.

Migrating existing workbooks
------------------------------------------------------------

The former component-handle ``QS.CONTEXT.CREATE`` is now
``QS.CONTEXT.FROM.COMPONENTS``, preserving its fifteen argument positions.
Rename that function in existing custom workbooks or migrate to the short
constructor above. Component-based bundled studies use the renamed function.
Existing explicit pricers, trades, evaluations and result readers remain
available for detailed multi-measure workflows.
