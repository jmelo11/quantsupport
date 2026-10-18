Market assumptions and product workflows
========================================

The first :doc:`workbook` constructed a curve from visible quotes. This chapter
continues with its reference date in ``B1``, quote handle in ``C7``, context in
``H5``, and swap in ``H6``. Place additional spill formulas in clear worksheet
areas, or adjust references when separating inputs and outputs across sheets.

Dates, accrual and observations
-------------------------------

Use ISO date text for financial dates. ``QS.DATE.ADVANCE`` adds a period without
business-day adjustment; ``QS.CALENDAR.ADJUST`` applies a specified convention.
A rate's day counter determines year fractions independently of either operation.

The examples use Actual360/Actual365. The native ``ActualActual`` year fraction
omits one complete intervening year when endpoint years differ by at least two;
it is not a general ISDA implementation. ``Thirty360US`` adjusts 31sts before
independently mapping each February month-end to day 30; its ordering differs
from common US/NASD/BondBasis rules. Do not infer general conformity from a
recognized day-count name or successful parsing.

.. code-block:: text

   =QS.DATE.ADVANCE("2026-02-24",6,"Months")
   =QS.CALENDAR.ADJUST("2026-12-25","TARGET","Following")
   =QS.YEAR.FRACTION("2026-02-24","2027-02-24","Actual360")
   =QS.SCHEDULE.ADJUSTED("2026-02-24","2028-02-24","Quarterly","TARGET","ModifiedFollowing")

Calendars use exact native names, including market qualifiers such as
``UnitedStates(Sofr)``, ``Brazil(Exchange)``, ``Canada(Settlement)``, and
``China(IB)``. ``JointCalendar(JoinHolidays,TARGET,Japan)`` requires both markets
to be open; ``JoinBusinessDays`` requires at least one. Finite announced holiday
tables must be updated beyond their published ranges. Excel exposes calendar
queries, not independent holiday-override objects or IMM helpers.

Business month-end is not necessarily civil month-end.
``QS.CALENDAR.IS.END.OF.MONTH`` uses the native next-day Following predicate,
which can also be True for non-business days after the last business day.
Business-day counts use configurable endpoint inclusion; reversed bounds are
signed. Native ``WeekendsOnly`` and ``NullCalendar`` list methods return empty
lists even when their predicates classify days as open. Use the exact query
reference rather than extrapolating list behavior from a predicate.

Observation timestamps instead require offset-aware RFC 3339 text.
``QS.DATETIME.PARSE`` normalizes to UTC; ``QS.DATETIME.DATE`` returns the UTC
calendar day, not a market business date or a rolled valuation date.

Explicit zero rates and composite funding
-----------------------------------------

An explicit zero curve is useful for an assumption-driven valuation or a
controlled funding stress. Set ``A12:A13`` to text ``0D`` and ``10Y`` and
``B12:B13`` to numeric ``0.03`` and ``0.03``:

.. code-block:: text
   :name: guide-excel-zero

   H14 =QS.CURVE.CONFIGS.CREATE("guide.explicit.curves")
   H15 =QS.CURVE.CONFIGS.ADD.ZERO(H14,"SOFR","Actual360","Linear",TRUE,A12:A13,B12:B13,"Continuous","Annual")
   H16 =QS.CONTEXT.CREATE("guide.explicit.context",C7,H15,,,,,,,,,,"USD","SOFR","Mid")
   J14 =QS.CURVE.GET(H16,"SOFR")
   J15 =QS.CURVE.DISCOUNT(J14,"2031-02-24")
   J16 =QS.CURVE.ZERO.RATE(J14,"2031-02-24","Continuous","Annual")

The rate 0.03 is an annual decimal continuous zero rate on Actual360.
Consequently

.. math::

   D(t,T)=\exp[-0.03\,\tau_{\mathrm{ACT/360}}(t,T)].

It is not an observed par-swap quote. Pillars must resolve in increasing order,
and numeric ranges must align exactly with them; do not omit blanks from the
rate range and shift the remaining values onto different dates.

For a composite funding illustration, place text ``0D``, ``10Y`` in
``D12:D13`` and numeric spread rates ``0.005``, ``0.005`` in ``E12:E13``:

.. code-block:: text

   H17 =QS.CURVE.CONFIGS.ADD.ZERO(H15,"SPREAD","Actual360","Linear",TRUE,D12:D13,E12:E13,"Continuous","Annual")
   H18 =QS.CURVE.CONFIGS.ADD.COMPOSITE(H17,"FUNDING","SOFR","SPREAD")
   H19 =QS.CONTEXT.CREATE("guide.funding.context",C7,H18,,,,,,,,,,"USD","SOFR","Mid")

The composite satisfies :math:`D_F=D_{\mathrm{SOFR}}D_{\mathrm{SPREAD}}`.
Continuous rates add only when compared on a common time basis. All component
configurations must exist, and cycles are errors. Creating a funding curve
does not automatically make every pricer use it; selection is a discount-policy
or instrument choice, and Excel exposes fewer policy controls than Python.

``QS.CURVE.FLAT`` instead creates a runtime ``DiscountCurveElement``. It can be
queried directly and composed into an element store with ``QS.CURVE.ADD``.
It need not expose interpolation nodes and cannot be captured in a serializable
market snapshot. Prefer construction configurations for historical workflows;
a flat zero-rate configuration is an explicit, archivable alternative.

Bond anchors and monetary value
-------------------------------

The straight fixed-rate bond constructor takes a position and its quotation
anchor. The following bullet bond uses the bootstrapped context and asks for
the spread consistent with a clean quote of 99.25:

.. code-block:: text
   :name: guide-excel-bond

   H22 =QS.TRADE.FIXED.BOND("guide.usd.bond",$B$1,$B$1,"2031-02-24","USD","SOFR",1000000,0.04,"Receive","Actual360","Simple","Annual","Semiannual","CleanPrice",99.25)
   =QS.BOND.PRICE(H22,H5)
   =QS.BOND.MODIFIED.DURATION(H22,H5)
   =QS.BOND.Z.SPREAD(H22,H5)
   =QS.BOND.SENSITIVITIES(H22,H5)

``CleanPrice`` excludes accrued interest; ``DirtyPrice`` includes it.
``ZSpread`` instead supplies a continuously compounded annual decimal spread:
0.001 means 10 basis points. Anchor names are exact. ``QS.BOND.PRICE`` returns
monetary PV, not the clean/dirty quote per 100 units. Modified duration is
yield-modified duration in years; it is not a raw curve-quote sensitivity.
The risk spill can include both curve inputs and ``dPV/dZSpread``.

The worksheet constructor supports straight fixed-rate bonds. Callable
``SteppedCouponBond`` construction and its tree/Fourier workflow are available through
Rust and Python. Although :excel:func:`QS.BOND.OAS` is registered, its current path
accepts only straight fixed-rate bonds and rejects their OAS request. There is
no successful worksheet OAS workflow through the available constructors. Use a
supported callable-bond interface for issuer-call valuation.

Options and model configuration
-------------------------------

Caps/floors and caplets/floorlets use the native Black rate-option workflows.
An absolute strike is an annual decimal rate, not a volatility percentage.
The context needs the relevant curves, fixings, and constructed volatility
source. Equity and FX options have absolute price/FX strikes instead.
Worksheet option constructors take absolute numeric strikes. Do not transfer
relative-strike payloads from native configuration or another pricer into
those cells: native Black/Hull–White cap/caplet consumers use
:math:`K=F(1+\mathrm{pct})`, whereas the generic ``Strike.resolve`` helper
uses an additive offset. These are different conventions, not interchangeable
encodings of one rate.

European swaptions require an explicit ``EuropeanSwaption`` /
``HullWhiteClosedForm`` configuration with a Hull–White model assigned to the
underlying index. Put the following complete JSON array in a text cell such
as ``AA1``:

.. code-block:: json

   [{
     "instrument_type": "EuropeanSwaption",
     "pricer": "HullWhiteClosedForm",
     "models": [{
       "market_index": "SOFR",
       "model": {"HullWhite": {
         "alpha": 0.1,
         "parameter_source": {"Fixed": {"sigma": 0.01}}
       }}
     }]
   }]

Load it and build a context with the pricer handle in argument 9:

.. code-block:: text
   :name: guide-excel-model

   AA2 =QS.PRICER.CONFIGS.FROM.JSON("guide.pricers",AA1)
   AA3 =QS.CONTEXT.CREATE("guide.option.context",C7,H4,,,,,,AA2,,,,"USD","SOFR","Mid")
   AA4 =QS.TRADE.EUROPEAN.SWAPTION("guide.swaption",$B$1,"2027-02-24","2031-02-24","USD","SOFR",0.04,1000000,"Payer","Receive","Actual360","Simple","Annual","Semiannual","Quarterly","")
   =QS.PRICE(AA4,AA3)

``Payer``/``Receiver`` choose exercise direction; the separate side chooses
position direction. The final empty string leaves the underlying calendar
unset; do not assume every required text argument has a blank-cell default.
The fixed sigma above is an absolute short-rate volatility, not a Black
volatility quote or a calibrated market fit.

For calibrated parameters, use ``Calibrated`` with a ``Surface`` or ``Cube``
source and an explicit basket, and load the matching volatility configuration.
The selected Mid/Bid/Ask side applies to calibration. Default native surface
extrapolation is ``Reject``; ATM or cube-basket requests can be outside quoted
strike slices. Restrict the basket or explicitly document a supported Flat
policy instead of silently changing the numerical assumption.

``QS.SIMULATION.CALIBRATION`` reports available Hull–White calibration
diagnostics; the eleven columns include market/model prices, calibrated sigma,
forward, strike, and signed model-minus-market price error. A simulation
without those diagnostics returns an explanatory error.

FX, equity, and credit inputs
-----------------------------

Use the worksheet boundary's explicit parameterized index names:
``Equity:SPX``, ``Credit:ACME``, ``FX:EUR/USD``, and
``Collateral:EUR/USD``. Native display labels are not all accepted input
formats: in particular use ``Equity:SPX``, not ``EQ:SPX``, to request an equity
variant explicitly. Native configuration JSON instead uses tagged indices
such as ``{"Equity": "SPX"}``.

``QS.FX.SET(store,"EUR","USD",1.1)`` means 1 EUR = 1.1 USD.
Rate lookup supports inverse and triangulated spots. An equity forward needs
an evaluation-date spot fixing and its forward curve; an absent dividend
curve means zero dividends. A CDS needs credit/survival inputs and recovery.
Quote-store observations are not a replacement for historical index fixings.
Worksheet fixing queries return stored observations, not interpolated ones.
Supply verified historical observations or handle gaps through an explicitly
validated external preprocessing policy. The native missing-fixing fill
routine uses reversed interpolation coordinates and is unavailable as a
worksheet operation.

Do not assume identical FX carry formulas across product pricers. The current
FX-forward implementation uses spot times quote/base discount factors, while
the FX-option path implements its separate base/quote convention.
Similarly, accepting a context base currency does not promise automatic
reporting-currency conversion of every price or cashflow table.

JSON imports and capability boundaries
--------------------------------------

JSON import functions take **text**, not filenames. Quote stores use the native
quote-store object; configuration/scenario imports use the native collections
accepted by their specific function. Python's similarly named file loaders
have different boundary semantics. Excel cells also impose a 32767 UTF-16-unit
text limit, which matters for snapshot archives.

The worksheet interface supports vanilla rates, basis/cross-currency swaps,
straight fixed income, FX/equity forwards and options, CDS, caps/caplets,
swaptions, and rate futures. There are no worksheet constructors for loans,
callable stepped bonds, scripted products, or standalone OIS trades; no
custom-pricer registration; and no exposure/netting/XVA worksheet workflow.
Configuration-based access to native models is not a claim that every native
model object or product is directly constructible.
