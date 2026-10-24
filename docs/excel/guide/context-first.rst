Contexts, inputs and evaluation
===============================

A worksheet context corresponds to the Rust ``PricingContext``. It owns dated
quotes, historical fixings, FX observations and market construction settings.
Create one root per coherent market and use independent contributions to define
its inputs. The context's quote reference date is its valuation date.

Create the market
-----------------

With identifiers in A10:A20 and decimal Mid values in F10:F20:

.. code-block:: text

   B2 =QS.CONTEXT.CREATE("usd","2026-01-15","USD","SOFR","Mid")
   B3 =QS.CONTEXT.ADD.QUOTES(B2,A10:A20,TRANSPOSE(F10:F20))
   B4 =QS.CONTEXT.ADD.CURVE(B2,"SOFR")
   B5 =B2

Every addition references B2. B5 is an optional alias for the same stable root.
The engine commits the cell-owned contributions after the calculation pass and
constructs required market dependencies lazily on workers. Editing or clearing
an addition invalidates registered consumers of B2. Failed inputs produce a
source diagnostic; unchanged recalculation preserves contributions and results.

Both matching XLLs must be loaded. Evaluation and market views return ``#N/A``
while pending and update through RTD. In Manual mode use **Refresh Pricing**;
F9 and Excel's calculation status do not await background jobs. Automation should
use the explicit completion helper in ``bindings/excel/examples/scripts/async_excel.py``.

For an individual quote, specify identifier, value and level:

.. code-block:: text

   =QS.CONTEXT.ADD.QUOTE(B2,"FixedRateDeposit_USD_SOFR_6M",0.04,"Mid")
   =QS.CONTEXT.ADD.FIXINGS(B2,A30:A40,F30:F40,J30:J40)
   =QS.CONTEXT.ADD.FX(B2,A50:A55,F50:F55,J50:J55)

All field arrays can be scalar, row, column or spilled arrays. They may have
different orientations and locations, but supplied fields must have equal entry
counts. Interior blanks keep their positions. Optional quote sides remain absent
when omitted. Do not supply headers or rectangular record tables.

Duplicate observations fail explicitly. Separate quote levels for the same
identifier are combined; providing the same level twice is an error. An input
belongs to its root context and cannot be accidentally included in another root.
Shared fixing arrays can be reused across dated roots without copying unchanged
history. No fixing publication-vintage filtering is inferred.

Evaluate a contract or trade
----------------------------

.. code-block:: text

   D2 =QS.INSTRUMENT.FIXED.RATE.DEPOSIT("deposit","2026-01-15","2026-07-15","USD",,1000,0.05,"Actual360","Simple","Annual")
   D3 =QS.TRADE.CREATE("position",D2,"2026-01-15","Receive")
   D4 =QS.EVALUATE(B2,D3,"Value")
   D5 =QS.EVALUATE(B5,D3,"Cashflows")
   D6 =QS.EVALUATE(B5,D3,"Sensitivities")
   D7 =QS.EVALUATE.RANGE(B5,D10:D100,"Value")

For independent valuation dates, supply one context per trade:

.. code-block:: text

   G2 =QS.EVALUATE.RANGE(C2:C10001,F2:F10001,"Value")

The context and instrument/trade arrays must have the same number of entries.
Each dated context retains its own market and curve cache. The engine schedules
the valuations across workers and spills one scalar per input row in order.
Use this form for large dated books to reduce Excel calls and result notifications.

Direct evaluation takes context, instrument/trade, request, optional
pricer and optional discount policy. Its result is the requested scalar or table.
The default request is Value. A contract uses Receive, the context date and its
contractual notional. A trade preserves its position terms. Bond contracts need
a trade with an explicit pricing anchor before evaluation.

A blank pricer selects the native product implementation using the context's
base discount curve. A policy can override that default selection. For explicit
model or funding settings, create a configured pricer once and pass its handle;
leave the separate policy argument blank because the pricer already owns it.

.. code-block:: text

   H2 =QS.DISCOUNT.POLICY.CSA("csa","USD","SOFR")
   H3 =QS.PRICER.CREATE("pricer","DiscountedCashflowPricer",H2)
   H4 =QS.EVALUATE(B5,D3,"Value",H3)
   H5 =QS.EVALUATE.RESULTS(B5,D3,{"Value","Cashflows","Sensitivities"},H3)
   H6 =QS.RESULT.VALUE(H5)

``QS.EVALUATE.RESULTS`` stores one native ``EvaluationResults`` object when
several measures or repeated result views are needed. Direct evaluations avoid
retaining one result object per row. The prepared-market cache is bounded and
validates upstream revisions before reuse.

For complete editable formulas open ``context_pricing_example.xlsx``. Continue
with :doc:`workbook` for installation and debugging, :doc:`markets-and-products`
for construction conventions, or :doc:`risk-and-history` for snapshots and historical batches.
