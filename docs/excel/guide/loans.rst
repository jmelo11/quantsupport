Loan contracts, funding and fair rates
======================================

A loan contract contains principal, dates, coupon rules and repayment structure.
A trade adds position direction and trade date. The pricer selects discounting,
refinancing assumptions and fair-rate amortization behavior. This follows the
Rust library's separation of ``Loan``, ``LoanTrade`` and ``LoanPricer``.

Read the constructor arguments
------------------------------

All rates and spreads are annual decimals: ``0.05`` means 5%, ``0.01`` means
100 basis points. Principal uses currency units. A blank optional argument uses
the default below; zero is a supplied value. In Excel, type the function and
press **Ctrl+A** to open Function Arguments, or enter
``=QS.HELP("QS.INSTRUMENT.LOAN")`` for the same parameter help in cells.

.. list-table:: QS.INSTRUMENT.LOAN, in worksheet argument order
   :header-rows: 1
   :widths: 20 20 60

   * - Argument
     - Required / blank default
     - Meaning
   * - ``name``
     - Required
     - Your object name, e.g. ``"mortgage"``; the function returns a loan handle.
   * - ``start_date``
     - Required
     - Advance/accrual start, as ISO text ``"2026-02-24"``; distinct from trade date.
   * - ``maturity_date``
     - Required
     - Final accrual date, after start; payment-date adjustment is a separate setting.
   * - ``currency``
     - Required
     - Principal/payment currency, e.g. ``"EUR"``.
   * - ``notional``
     - Required
     - Positive original principal, e.g. ``10000`` means EUR 10,000 for EUR.
   * - ``rate``
     - Required
     - Fixed coupon if ``market_index`` is blank; spread over the index otherwise.
       Coupon arrays, when supplied, replace these scalar coupon terms.
   * - ``market_index``
     - Fixed coupon
     - Floating projection index, e.g. ``"EURIBOR12m"``; this is not the discount curve.
   * - ``repayment``
     - ``"French"``
     - French: level initial instalments. Bullet: interest with all principal at
       maturity. Balloon: amortization leaving a final fraction of original principal.
   * - ``payment_frequency``
     - ``"Monthly"``
     - Payment/accrual frequency, e.g. Monthly, Quarterly, Semiannual or Annual.
       Index resets and observation tenor are separate settings.
   * - ``day_counter``
     - ``"Actual365"``
     - Year fraction for interest and initial principal amortization.
   * - ``settings``
     - No optional overrides
     - Handle returned by ``QS.LOAN.SETTINGS``; see the defaults below.
   * - ``coupon_periods``
     - No segment schedule
     - Payment count for each successive coupon segment; ``12`` means 12 payments,
       not 12 years. Only the final count may be blank to fill maturity.
   * - ``coupon_rates``
     - Use scalar ``rate``
     - Annual coupon/spread for each segment. Supplied segments must cover the loan.
   * - ``coupon_indices``
     - All segments fixed
     - Projection index per segment; a blank entry means fixed. An omitted array
       makes every segment fixed even if scalar ``market_index`` is supplied.
   * - ``prepayment_dates``
     - No dated CPR model
     - Strictly increasing ISO observation dates, aligned with ``prepayment_cprs``.
   * - ``prepayment_cprs``
     - No dated CPR model
     - Annual conditional prepayment fractions in [0,1], e.g. ``0.10`` = 10% per year.

Arrays are independent row, column or spill ranges with equal entry counts
within each group. Do not include headers. The final coupon count can be blank;
other interior blanks are retained, not removed. Coupon arrays cannot be combined
with introductory terms or a floor/cap in Loan Settings.

Choose optional settings
-------------------------

``QS.LOAN.SETTINGS(name, ...)`` returns a settings handle. Its remaining arguments
are optional, in the order below. Leave unused argument positions empty or use
cell references so the formula remains readable.

.. list-table:: Loan Settings defaults and units
   :header-rows: 1
   :widths: 24 22 54

   * - Argument
     - Blank default
     - Meaning / when to supply
   * - ``balloon_fraction``
     - Unset
     - Residual fraction in [0,1] of original principal, e.g. ``0.25`` = 25%.
       Required for Balloon; invalid for French/Bullet.
   * - ``amortization_rate``
     - First fixed coupon
     - Annual all-in rate used to build principal instalments, e.g. ``0.05``.
       Required for floating-first French/Balloon; not needed for Bullet.
       It does not set the coupon spread or forecast market rates.
   * - ``credit_index``
     - No borrower default
     - Borrower survival curve, e.g. ``"Credit:BORROWER"``.
   * - ``recovery``
     - ``0.40``
     - Fraction of opening principal recovered on default; [0,1].
   * - ``fixing_frequency``
     - Index metadata
     - Floating reset frequency; daily indices use payment frequency by default.
       Each reset must fall on a payment accrual boundary.
   * - ``fixing_lag``
     - ``0``
     - Nonnegative whole business days backwards from reset to fixing.
   * - ``index_tenor``
     - Index metadata
     - Positive observation period, e.g. ``"3M"`` or ``"12M"``; separate from
       payment frequency and reset frequency.
   * - ``calendar``
     - ``NullCalendar``
     - Payment/fixing calendar, e.g. ``"TARGET"``. NullCalendar treats every day
       as a business day. Accrual boundaries stay unadjusted.
   * - ``business_day_convention``
     - ``"Unadjusted"``
     - Payment adjustment: Following, ModifiedFollowing, Preceding,
       ModifiedPreceding or Unadjusted.
   * - ``floor`` / ``cap``
     - No bounds
     - Annual decimal bounds on the all-in floating coupon (index plus spread),
       e.g. ``0`` / ``0.06``. Require scalar ``market_index``; incompatible with
       coupon arrays and FairRate solving.
   * - ``introductory_periods``
     - No introduction
     - Positive number of initial fixed payments; must leave at least one floating
       payment. Supply ``introductory_rate`` too on a floating loan.
   * - ``introductory_rate``
     - No introduction
     - Annual fixed coupon during the introduction; afterwards scalar ``rate``
       is a spread over ``market_index``. Incompatible with coupon arrays.
   * - ``annual_cpr``
     - No prepayment
     - Constant annual prepayment fraction, [0,1], e.g. ``0.10`` = 10% per year.
       Do not also supply dated CPRs or refinancing settings.
   * - ``base_cpr``
     - No refinancing model
     - Base annual CPR, [0,1]. Supply all three refinancing fields together and
       a benchmark index on ``QS.LOAN.PRICER``.
   * - ``incentive_multiplier``
     - No refinancing model
     - Nonnegative CPR increase per unit of positive coupon-minus-benchmark
       incentive; ``4`` adds 0.04 CPR for a 0.01 rate incentive.
   * - ``maximum_cpr``
     - No refinancing model
     - Annual CPR ceiling, [0,1], at least ``base_cpr``. Refinancing settings
       cannot be combined with constant or dated CPRs.

With no prepayment inputs, there is no prepayment model. Annual CPR is converted
to an interval probability as ``1 - (1 - CPR)^year_fraction``; it is not a
per-payment percentage. Dated CPRs use the latest observation at or before the
interval start, zero before the first date and the last rate thereafter.

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
A floating-first French/Balloon schedule requires a separate amortization rate;
projection and contractual amortization assumptions are distinct.

A minimal fixed French loan needs only the six required arguments. A floating
French loan supplies an all-in amortization rate through settings; its scalar
``rate`` is the spread. These contracts can be passed directly to ``QS.EVALUATE``
for Receive at the context date, or wrapped in ``QS.TRADE.CREATE`` as above.

.. code-block:: text

   F2 =QS.INSTRUMENT.LOAN("fixed","2026-02-24","2031-02-24","EUR",10000,0.05)
   F3 =QS.LOAN.SETTINGS("floating.options",,0.05)
   F4 =QS.INSTRUMENT.LOAN("floating","2026-02-24","2031-02-24","EUR",10000,0.01,"EURIBOR12m","French","Monthly","Actual365",F3)

For a stepped fixed coupon, put ``12`` and a blank in A40:A41, and ``0.03`` and
``0.05`` in F40:F41. The first 12 monthly payments use 3%; the remainder use 5%:

.. code-block:: text

   F5 =QS.INSTRUMENT.LOAN("stepped","2026-02-24","2031-02-24","EUR",10000,0.03,,,,,,A40:A41,F40:F41)

To make the second segment floating, put a blank and ``EURIBOR12m`` in J40:J41
and pass that range as ``coupon_indices``. Its ``0.05`` would then mean a 500 bp
spread; change it to the intended spread, for example ``0.01`` for 100 bp.

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
