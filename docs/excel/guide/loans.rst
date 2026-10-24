Loan contracts, funding and fair rates
======================================

A loan contract contains principal, dates, coupon rules and repayment structure.
A trade adds position direction and trade date. The pricer selects discounting,
refinancing assumptions and fair-rate amortization behavior. This follows the
Rust library's separation of ``Loan``, ``LoanTrade`` and ``LoanPricer``.

Build a market context
----------------------

For illustrative continuous zero rates, put ordered pillars in A10:A14 and rates
in F10:F14. A pillar can be ISO text or a tenor such as 0D or 10Y.

.. code-block:: text

   B2 =QS.CONTEXT.CREATE("eur","2026-02-24","EUR","ESTR")
   B3 =QS.CONTEXT.ADD.ZERO.CURVE(B2,"ESTR",A10:A14,F10:F14,"Actual360","Linear",TRUE,"Continuous","Annual")
   B4 =QS.CONTEXT.ADD.ZERO.CURVE(B2,"EURIBOR12m",A10:A14,J10:J14,"Actual360","Linear",TRUE,"Continuous","Annual")
   H2 =B2

ESTR discounts payments under the illustrated policy; EURIBOR12m projects the
floating coupon. Use appropriate calibrated inputs for production work. The
independent field arrays can have different orientations and need no table
headers. Each curve addition references B2, and H2 consumes the complete array.

Create fixed and floating contracts
-----------------------------------

.. code-block:: text

   D2 =QS.LOAN.SETTINGS("balloon.options",0.25)
   D3 =QS.INSTRUMENT.LOAN("balloon","2026-02-24","2031-02-24","EUR",10000,0.05,,"Balloon","Monthly","Actual365",D2)
   D4 =QS.TRADE.CREATE("receipt",D3,"2026-02-24","Receive")
   D5 =QS.LOAN.PRICER("loan.pricer","EUR","ESTR")
   D6 =QS.EVALUATE(H2,D4,"Value",D5)
   D7 =QS.EVALUATE(H2,D4,"FairRate(0)",D5)

The rate is an annual decimal fixed coupon when market_index is blank and a
floating spread otherwise. French, Bullet and Balloon repayment are supported.
Balloon repayment requires the residual principal fraction in Loan Settings.
A floating French schedule can use a separate amortization rate; projection and
contractual amortization assumptions are distinct.

``QS.LOAN.SETTINGS`` exposes named optional parameters in IntelliSense. Omitted
settings retain native defaults. These include balloon fraction, credit index,
recovery, fixing conventions, calendar, coupon bounds, introductory fixed terms
and prepayment assumptions. The loan constructor accepts the settings handle,
then independent coupon-period, rate/spread and index arrays, followed by
independent prepayment-date and annual-CPR arrays. Only the final coupon segment
may omit its period count to fill the remaining maturity.

Funding and borrower credit
---------------------------

Funding is an ordinary curve chosen by a discount policy. For a treasury spread,
construct its curve in the same root, then compose a FUNDING index:

.. code-block:: text

   B6 =QS.CONTEXT.ADD.ZERO.CURVE(B2,"FUNDINGSPREAD",A10:A14,N10:N14,"Actual360","Linear",TRUE,"Continuous","Annual")
   B7 =QS.CONTEXT.ADD.COMPOSITE.CURVE(B2,"FUNDING","EURIBOR12m","FUNDINGSPREAD")
   B8 =B2
   D8 =QS.LOAN.PRICER("funded.loan","EUR","FUNDING")
   D9 =QS.EVALUATE(B8,D4,"Value",D8)

Keep B3:B7 exclusively for addition handles or blanks. Prepared contexts belong
outside the contribution array. The full loan example separates those blocks.
A composite multiplies base and spread discount factors. Contract settings do
not select funding curves.

For borrower credit use an independent CDS identifier and spread array:

.. code-block:: text

   =QS.CONTEXT.ADD.CREDIT.CURVE(B2,"Credit:BORROWER","EUR","ESTR",0.4,A30:A36,F30:F36)
   =QS.DISCOUNT.POLICY.FIXED.INCOME("policy","EUR","FUNDING")

Attach the same credit index and compatible recovery assumption through Loan
Settings. Credit and prepayment can affect the future receipt schedule. Preserve
those model-dependent cashflows when comparing pricing and risk results.

Prepayment and fair-rate interpretation
---------------------------------------

Constant annual CPR, dated CPR observations and refinancing-based prepayment
models are supported through their named settings. Refinancing settings use an
explicit benchmark curve on LoanPricer. Incompatible prepayment specifications
fail validation instead of being silently combined.

A loan FairRate(index) solves earlier coupon segments in order, repays remaining
capital at each prefix end, locks each solved coupon/spread and then extends to
the requested segment. One evaluation returns one scalar fair_rate. Use separate
requests for each segment. Coupon bounds remain unsupported for this solve.
The pricer's rebuild_amortization option controls whether the contractual
amortization schedule is rebuilt during solving.

.. code-block:: text

   =QS.LOAN.PRICER("refinancing","EUR","FUNDING","EURIBOR12m",TRUE)
   =QS.EVALUATE.RESULTS(H2,D4,{"Value","Cashflows","Sensitivities"},D5)

Value is signed future receipt PV under the native loan conventions and excludes
the original principal advance. Cashflow tables contain signed undiscounted
payments; loan legs distinguish scheduled receipts, prepayment and recovery.
Sensitivities are raw per-unit derivatives. For decimal rates, multiply by
0.0001 for an approximate +1 bp change.

The editable loan study includes fixed, floating, balloon, stepped, mixed,
credit, bounded and prepayment cases. A selector drives its cashflow and risk
views. The context study demonstrates separate row/column inputs and historical
batch pricing; see :doc:`risk-and-history` for snapshot selection and batch evaluation.
