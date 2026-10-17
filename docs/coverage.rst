Rust Component Coverage
=======================

The bindings are task-oriented interfaces to the Rust library, not a one-to-one
export of every public Rust struct or trait. A component can be available through
configuration and valuation even when its implementation type is not a binding
class. The following distinctions describe the current public surfaces.

Time and Calendars
------------------

Python exposes :py:class:`~quantsupport.time.Date`,
:py:class:`~quantsupport.time.Period`, :py:class:`~quantsupport.time.Calendar`,
:py:class:`~quantsupport.time.Schedule`, :py:class:`~quantsupport.time.IMM`,
day counters, and schedule/accrual conventions. The
:doc:`calendar catalog <python/sections/time/calendars>` lists every native
calendar variant and its Python constructor. There are no separate Python
``TARGET`` or ``Japan`` classes: these are typed ``Calendar`` values.

Python calendar holiday overrides return independent copies. Month-end queries,
holiday/business-day lists, adjustment, and advancement operate on those copies.
The native list-method limitations for ``WeekendsOnly`` and ``NullCalendar``
are documented on the corresponding methods rather than silently changed.

Excel exposes :doc:`calendar queries <excel/functions/section-time-calendars>`,
schedule generation, and date/accrual queries. Calendar arguments use canonical
native names, such as ``UnitedStates(Sofr)``, ``Brazil(Exchange)``,
``UnitedKingdom(Settlement)``, or ``JointCalendar(JoinHolidays,TARGET,Japan)``.
Excel does not expose independent holiday-override objects or IMM helpers.

The Python ``Schedule`` constructor wraps native ``MakeSchedule``; a second
fluent builder is not required. Low-level Rust ``Weekday``, ``Month``,
``IMMMonth``, and ``easter_monday`` do not have independent Python interfaces.

Market Data and Construction
----------------------------

.. list-table::
   :header-rows: 1
   :widths: 22 39 39

   * - Rust component
     - Python access
     - Excel access
   * - Currencies and market indices
     - Typed ``Currency`` and ``MarketIndex`` values/factories.
     - Canonical names in worksheet parameters and native configuration files.
   * - Quotes, historical fixings, FX
     - ``QuoteStore``, ``FixingStore``, ``FxStore``.
     - ``QS.QUOTES.*``, ``QS.FIXINGS.*``, ``QS.FX.*`` constructors and updates.
   * - Yield and credit bootstrapping
     - ``CurveConfiguration`` / ``CreditCurveConfiguration`` with ``PricingContext``.
     - Configuration handles with ``QS.CONTEXT.CREATE``.
   * - Explicit yield curves
     - ``ZeroRateTermStructure``, ``TenorBasedZeroRateTermStructure`` and curve configurations.
     - ``QS.CURVE.FLAT``, curve-configuration functions and constructed curve queries.
   * - Volatility construction
     - Surface/cube configurations and read-only constructed views.
     - Surface/cube configuration handles and ``QS.VOL.SURFACE`` / ``QS.VOL.CUBE`` queries.

``MultiCurveBootstrapper``, ``CreditCurveBootstrapper``, interpolation engines,
and volatility builder implementation types are not separately constructible
binding classes. Their standard workflows are available through configurations;
custom low-level calibration callbacks are not a binding API.

Instruments and Pricing
-----------------------

Python groups instruments into rates, rate options, fixed income, loans,
coupons/payoffs, FX, equity/credit, and scripted products. Positions use
``Trade`` separately from contractual instrument terms. Excel exposes its
supported constructors under ``QS.TRADE.*`` and valuations under pricing and
bond analytics subsections.

Important remaining instrument gaps are native ``OvernightIndexSwap`` and equity
``Futures`` as independently constructible products in both bindings. OIS quote
bootstrapping through curve configurations does not provide a standalone OIS
trade constructor. Excel also has no ``Loan``, ``SteppedCouponBond``, or
``ScriptedProduct`` constructor. These should not be substituted with a vanilla
``Swap`` or another product with different cashflow semantics.

Native pricing engines are selected through ``PricerConfiguration`` and context
evaluation. Python additionally exposes ``LoanPricer`` and custom
``Instrument`` / ``Pricer`` / ``MarketDataProvider`` / ``Evaluator`` interfaces.
Most concrete Rust pricing-engine structs are not independent binding classes;
that does not mean their configured valuation workflows are unavailable.

Models, Simulation, and Risk
----------------------------

Hull-White/LGM model selection and calibration are exposed through native
simulation/pricer configurations. Python constructed simulation views and Excel
``QS.SIMULATION.*`` queries provide dates, paths, and calibration results;
the Rust model and Monte Carlo engine structs are not independent user objects.

Python scenarios, scoped shocks, collateral agreements, netting sets, exposure
profiles, and XVA results are exposed in :py:mod:`quantsupport.risk`. Excel can
load scenarios for context construction, but has no public netting-set, exposure,
or XVA worksheet workflow. Low-level contingent-claim visitors and custom XVA
aggregator implementations remain Rust APIs.

Python ``ScriptedProduct`` uses the native payoff engine. Its parser, AST nodes,
and step-by-step runtime are not separately exposed. Numerical solvers, raw AD
tapes/scalars, FFT routines, and generic visitor infrastructure also remain Rust
APIs; valuation sensitivities use these internally.

Reference Organization
----------------------

Every public Python class is assigned to exactly one domain subsection. Every
registered Excel function is assigned to a domain and focused subsection. The
documentation build rejects unclassified additions, while binding tests check
native calendar variants and public exports. This ensures discoverability, but
does not claim automatic semantic parity with every Rust component.