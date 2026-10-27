Markets, instruments and pricing conventions
============================================

Start every market with ``QS.CONTEXT.CREATE``. Its quote store reference date
is the valuation date. Base currency/index and quote side govern native market
construction. Additions belong to that stable root. Evaluation commits the complete
definition pass and lazily constructs only its required dependencies on workers.

Bootstrapped and explicit curves
--------------------------------

Use independent identifier and quote arrays for bootstrapping. Mid/Bid/Ask
fields may have different locations and orientations; they must align by entry
count. A missing selected side is an error. Construction uses quote identifier
conventions from the Rust library, including the product, index and tenor.

.. code-block:: text

   B2 =QS.CONTEXT.CREATE("usd","2026-01-15","USD","SOFR")
   B3 =QS.CONTEXT.ADD.CURVE(B2,"SOFR",A10:A20,F10:F20)
   B4 =B2

When identifiers and values are already supplied through ADD.QUOTES, omit them
in ADD.CURVE to select matching market-index quotes. Matching is deterministic.
Duplicate curve indices or quote sides fail explicitly rather than overwriting
an unrelated contribution.

Explicit zero curves accept separate ordered pillar and rate arrays:

.. code-block:: text

   =QS.CONTEXT.ADD.ZERO.CURVE(B2,"SOFR",A30:A34,F30:F34,"Actual360","Linear",TRUE,"Continuous","Annual")
   =QS.CONTEXT.ADD.COMPOSITE.CURVE(B2,"FUNDING","SOFR","FUNDINGSPREAD")

Pillars can be ISO date text or tenors such as 0D, 1Y and 10Y. Rates are annual
decimals under the specified compounding and frequency. The zero curve is a
construction configuration, so snapshots can rebuild it at a different market
reference date. A composite multiplies the discount factors of its base and
spread curves. FUNDING is an ordinary index, selected through a policy rather
than through contractual loan settings.

``QS.CURVE.GET`` obtains a curve from a committed context. Use DISCOUNT,
ZERO.RATE, FORWARD and TABLE under the same namespace to inspect it. The table
shows pillar dates, discount factors, zero rates and forwards. Default displayed
rates use Simple/Annual and the native curve day count. Extrapolation is explicit.

Fixings, FX, credit and volatility
----------------------------------

.. code-block:: text

   =QS.CONTEXT.ADD.FIXINGS(B2,A40:A50,F40:F50,J40:J50)
   =QS.CONTEXT.ADD.FX(B2,A60:A65,F60:F65,J60:J65)
   =QS.CONTEXT.ADD.CREDIT.CURVE(B2,"Credit:BORROWER","USD","SOFR",0.4,A70:A75,F70:F75)
   =QS.CONTEXT.ADD.VOL.SURFACE(B2,"SOFR","Black","Strike",A80:A90,F80:F90)
   =QS.CONTEXT.ADD.VOL.CUBE(B2,"SOFR","Black","Strike",A100:A110,F100:F110)

Fixing arrays are market index, ISO fixing date and finite value. Values retain
the index's native units. FX arrays are base currency, quote currency and
positive rate, expressed as quote units per base unit. Credit curves calibrate
survival from CDS identifiers with explicit recovery and discount assumptions.
Black volatility is annualized decimal volatility; Normal volatility uses rate
or price units per square-root year. The quote identifiers and configuration
must agree on that convention.

Volatility surface/cube requests use the corresponding context data. Model and
pricer configurations can be attached with ADD.SIMULATION.CONFIGURATION and
ADD.PRICER.CONFIGURATION using individual native JSON objects, preserving typed
Rust model assignments without a worksheet key/value table.

Keep contracts separate from positions
--------------------------------------

``QS.INSTRUMENT.*`` follows native product families. Fixed income uses
FIXED.RATE.BOND, FIXED.RATE.DEPOSIT and FLOATING.RATE.NOTE; rate futures use
RATE.FUTURES. Construct a position with ``QS.TRADE.CREATE``. Product-specific
trade shortcuts have been removed.

.. code-block:: text

   D2 =QS.INSTRUMENT.FX.FORWARD("fx",B2,"2027-01-15","EUR","USD",1.10)
   D3 =QS.TRADE.CREATE("position",B2,D2,"2026-01-15","Receive",1000000)
   D4 =QS.EVALUATE(B4,D3,"Value")

The native FX carry convention depends on the selected pricer. FX outrights are
quote currency per base unit; quote points are raw forward-minus-spot amounts,
not automatically pip-scaled. Equity indices use identifiers such as Equity:SPX.
Futures contract size is currency per price point, not per basis point.

Relative dates and pending trades
---------------------------------

``QS.INSTRUMENT.*`` functions have separate tenor arguments. Leave an absolute
date blank when supplying its tenor alternative. Instrument and trade handles
take a required construction context immediately after the name. Construction
calculates concrete dates before publishing a product handle.

.. code-block:: text

   D2 =QS.INSTRUMENT.FX.FORWARD("fx",B2,"","EUR","USD",1.10,"1Y")
   D3 =QS.TRADE.CREATE("position",B2,D2,"","Receive",1000000)
   D4 =QS.EVALUATE(B4,D3,"Value")

Scheduled products use ``start_tenor`` and ``tenor``; options use
``expiry_tenor`` and forwards use ``delivery_tenor``. ``QS.TRADE.CREATE`` also
accepts ``trade_tenor``. A blank execution date and blank tenor default to
``0D`` in the construction context. The resulting dates and initial principal
remain fixed during valuation; constructor recalculation creates a new contract.

``QS.CONTEXT.CREATE`` appends ``anchor_fixings`` (TRUE by default). TRUE
requires stored index fixings strictly before evaluation, raising an error
when one is missing; today and future observations project. FALSE projects
all index observations, subject to the curve's supported date range.

Policies and explicit model choices
-----------------------------------

.. code-block:: text

   L3 =QS.DISCOUNT.POLICY.CSA("csa","USD","SOFR")
   L4 =QS.PRICER.CREATE("dcf","DiscountedCashflowPricer",L3)
   L5 =QS.PRICER.CREATE("swaption","ClosedFormHullWhiteSwaptionPricer",L3,AC30:AC31,AD30:AD31)
   L6 =QS.DISCOUNT.POLICY.FIXED.INCOME("fixed.income",A120:A122,F120:F122)

Pricer settings are independent name/value arrays. For Hull-White alpha and
sigma, put setting names in AC30:AC31 and values in AD30:AD31. The explicit
swaption pricer requires its model parameters. RateFuturesPricer does not use a
discount policy and rejects one. Fixed-income policies map currencies to curve
indices and can optionally prefer an instrument's discount index.

Bonds require CleanPrice, DirtyPrice or ZSpread plus an anchor value on the
trade. Clean/dirty anchors use quotation units, while PV is monetary. Use native
requests YieldToMaturity, ModifiedDuration and ZSpread as supported by the
selected pricer. Straight exposed bond constructors do not supply callable
exercise terms for OAS, and BondPricer does not define FairRate.

The editable swap, bond and loan workbooks explain conventions next to each
input. The generated worksheet reference is authoritative for argument order,
optional defaults and native errors. Unsupported products and pricers fail
explicitly; the worksheet API does not register custom scripted products or an
exposure/XVA engine.
