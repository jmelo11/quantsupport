A Position, a Market, and a Valuation
=========================================

Installation and Reproducibility
------------------------------------

Install a released wheel into the interpreter used by your application or
notebook kernel:

.. code-block:: console

   python -m pip install quantsupport

A wheel needs no Rust compiler. Optional editor support is available through
``quantsupport[typing]``. For development from this repository, create a virtual
environment, install Maturin, and compile the checkout:

.. code-block:: console

   python -m venv .venv
   . .venv/bin/activate
   python -m pip install "maturin>=1.5,<2.0"
   maturin develop -m bindings/python/Cargo.toml --release

On Windows activate ``.venv\Scripts\Activate.ps1`` instead. Source builds require
the stable Rust toolchain. A source checkout and an older installed wheel are
not interchangeable: inspect the selected interpreter before diagnosing a
missing class. Market data, conventions, package revision, and model parameters
are part of the reproducibility record, not merely the final price.

Contract and Position
---------------------

A :py:class:`~quantsupport.instruments.Swap` describes contractual terms.
A :py:class:`~quantsupport.pricing.Trade` adds execution date, side, and optional
position-size overrides. Two trades can share the same contract while holding
opposite positions. The execution date is required; it is not inferred from
maturity, exercise, or the context date.

For a vanilla swap, ``LongReceive`` receives fixed and pays floating;
``PayShort`` reverses that direction. Use nonnegative position sizes and express
direction through ``side``. A product dictionary contains constructor terms,
not ``side``, ``trade_date``, bond anchors, or futures contract counts.

The following complete example values a five-year USD receive-fixed swap.
It deliberately uses an explicit flat *continuous* zero-rate curve, not a
calibrated market quote. Two identical rate pillars make its construction
transparent. A 3% continuous zero rate does not make a 3% simple coupon par.

.. code-block:: python
   :name: guide-python-swap

   import quantsupport as qs

   reference = qs.Date(2026, 2, 24)
   quotes = qs.QuoteStore.from_dict({
       "reference_date": str(reference), "quotes": [],
   })
   curve_spec = qs.CurveConfiguration(
       market_index=qs.MarketIndex.SOFR,
       construction=qs.CurveConstruction.zero_rate(
           pillars=["0D", "10Y"], rates=[0.03, 0.03],
           compounding="Continuous", frequency="Annual",
       ),
       day_counter="Actual360", interpolator="Linear",
       enable_extrapolation=True,
   )
   policy = qs.SingleCurveCSADiscountPolicy("USD", "SOFR")
   context = qs.PricingContext(quotes, [curve_spec], csa_policy=policy)
   swap = qs.Swap(
       identifier="USD-IRS-5Y", start_date=reference,
       maturity_date=reference + "5Y", notional=10_000_000,
       fixed_rate=0.03, currency="USD", market_index="SOFR",
       day_counter="Actual360", compounding="Simple",
       rate_frequency="Annual", fixed_leg_frequency="Semiannual",
       floating_leg_frequency="Quarterly",
   )
   trade = qs.Trade(swap, reference, qs.Side.LongReceive)

   with context:
       result = context.evaluate(trade, [
           qs.Request.Value, qs.Request.FairRate,
           qs.Request.Cashflows, qs.Request.Sensitivities,
       ])
       discount = context.curve("SOFR").discount_factor(reference + "5Y")
       print(result.price, result.fair_rate, discount)
       print(result.sensitivities)
       assert result.price < 0
       assert result.fair_rate > swap.fixed_rate
       assert 0 < discount < 1

The negative PV is not a sign error: coupon accrual, payment frequency, and
compounding differ from the continuous curve quotation. ``fair_rate`` is an
annual decimal fixed coupon under the swap's conventions, not a price or
spread in basis points. The position's monetary scale is ten million USD;
neither the rate ``0.03`` nor a returned sensitivity is multiplied by 100.

For a single-currency fixed-versus-floating swap the economic decomposition is

.. math::

   V_{\mathrm{receive\ fixed}}
   = \sum_i D(t,T_i)\,C_i^{\mathrm{fixed}}
     - \sum_j D(t,U_j)\,C_j^{\mathrm{floating}}.

The familiar par coupon :math:`K^*=PV_{\mathrm{float}}/A`, where
:math:`A=N\sum_i\alpha_iD(t,T_i)`, is a vanilla-swap relationship.
The current generic ``FairRate`` implementation uses the first forward curve,
fixed coupon annuity, and floating amounts, and ignores principal flows.
Do not treat it as a general multi-leg breakeven solver for basis or
cross-currency positions.

Interpreting Results
--------------------

An :py:class:`~quantsupport.pricing.EvaluationResults` holds optional measures:
a property is ``None`` when it was not produced. ``Value`` requests monetary
PV, not a clean bond quote. ``Sensitivities`` returns a pandas table with
``pillar`` and ``value`` columns. Rows are raw derivatives with respect to
their named input, not automatically DV01 or vega-per-volatility-point.

For a decimal-rate risk factor :math:`q`, the first-order change for a +1 bp
move is :math:`(\partial V/\partial q)10^{-4}`. The explicit curve above has
two independently labeled zero-rate inputs; summing their derivatives estimates
a parallel zero-rate move. That sum is not a sensitivity to a calibrated OIS
quote. See :doc:`risk` for a full-revaluation check.

``cashflows`` contains ``payment_date``, ``type``, ``amount``, ``fixing``,
``accrual_period``, ``currency``, ``caplet_strike``, ``floorlet_strike``, and
``leg_index``. For the generic discounted-cashflow pricer these are underlying
cashflow amounts before leg-side multiplication, not a signed PV ledger.
The table may include initial disbursements and redemptions on both legs.
Summing ``amount`` therefore does not reproduce the swap PV. A reconciliation
must apply the particular product's leg signs, discounting, FX conversion where
applicable, and position conventions explicitly.

The generic discounted-cashflow value path iterates all supplied cashflows; it
does not itself filter them by payment date. The example is valued at inception
so no past-payment assumption is hidden. Do not roll this contract into the
past and assume settled payments disappear automatically. Other pricers may
have different cashflow filtering and reporting rules.

Context Lifetime
----------------

Constructing :py:class:`~quantsupport.pricing.PricingContext` stores inputs.
Entering ``with context:`` validates them, activates the scope, and enables
lazy construction of requested curves, volatility objects, models, and
simulations. Merely entering does not build every configured object.
``context.initialize()`` explicitly constructs all configured objects when
eager construction is wanted; it is not equivalent to entering a scenario
scope or a general promise to start AD recording.

Read constructed views inside their active scope. Final scope exit releases
constructed market data and performs the context's AD cleanup, including when
an exception propagates. Contexts and scenario stacks are thread-confined;
enter and exit on the owning thread in reverse order. Scalar results and
copied result tables can be retained, but an AD-backed curve view should not
be used after its context or scenario ends.

Public context input setters invalidate dependent construction. Store and
configuration getters return copies: mutate a copy and assign it back, rather
than assuming ``context.quotes`` is a live shared market object. These semantics
allow a second valuation to change market assumptions without mutating the
contract or retaining obsolete constructed objects.

Dates, Types, and Protocols
-------------------------------

Financial dates accept the documented ``Date``, ISO text, or Python
``datetime.date`` forms. ``Date`` arithmetic with ``Period`` or a tenor such as
``"6M"`` is calendar arithmetic, not business-day adjustment. A ``Calendar``
supplies that adjustment. ``Schedule`` supports iteration and indexing and
returns copied dates and regularity flags; coupon accrual conventions are
separate from schedule generation.

Day-count names are selectors, not a general conformity guarantee.
The current ``ActualActual.year_fraction`` omits one complete intervening
year when endpoint years differ by at least two; same/adjacent-year handling
does not establish general ISDA correctness. ``Thirty360US`` adjusts 31sts
before independently mapping each February month-end to day 30, differing
from common US/NASD/BondBasis rules. The verified workflows here use
Actual360/Actual365 rather than either restricted convention.

Timestamps are different: ``DateTime`` requires offset-aware input, normalizes
to UTC, and retains native nanoseconds; conversion to Python datetime truncates
to microseconds. An observation's UTC day is not necessarily its market
valuation date.

Many financial constructors accept typed enums or names, but not every API
does. ``Shock.shock_type`` and typed calendar/schedule parameters require their
documented enum values. Parameterized market indices should use factories such
as ``MarketIndex.equity("SPX")`` or native tagged dictionaries, not display
labels such as ``"EQ:SPX"``. The reference shows precise accepted forms.

Native failures generally raise ``QuantSupportError``; Python argument and
scope checks can instead raise ``TypeError``, ``ValueError``, or
``RuntimeError``. A failed construction is information about the requested
market, not a reason to silently substitute another calendar, quote side,
volatility convention, or extrapolation policy.
