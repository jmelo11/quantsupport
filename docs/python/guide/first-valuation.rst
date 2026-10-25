A position, a market, and a valuation
=========================================

Installation and reproducibility
------------------------------------

Install a released wheel into the interpreter used by your application or
notebook kernel:

.. code-block:: console

   python -m pip install quantsupport

For a manual download, choose your operating system, architecture and Python
version on the `Python packages page <https://jmelo11.github.io/quantsupport/python/downloads.html>`_.
It provides wheels and folder ZIPs containing only QuantSupport. Install
dependencies separately with ``python -m pip install pandas`` before using a
folder ZIP.

A wheel needs no Rust compiler. Optional editor support is available through
``quantsupport[typing]``. For development from this repository, create a virtual
environment, install Maturin, and compile the checkout:

.. code-block:: console

   python -m venv .venv
   . .venv/bin/activate
   python -m pip install "maturin>=1.5,<2.0"
   maturin develop -m bindings/python/Cargo.toml --release

On Windows activate ``.venv\Scripts\Activate.ps1`` instead. Source builds require
the stable Rust toolchain. Select the same interpreter in your application or
notebook so that it loads the checkout-built binding. To reproduce a valuation,
retain the market data, conventions, package revision and model parameters.

Contract and position
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

Consider a five-year USD swap that receives a fixed coupon of 3% and pays a
floating coupon on a notional of USD 10 million. We define the instrument,
wrap it in a trade and value it against a flat, continuously compounded 3%
zero-rate curve. Two equal rate pillars specify that curve explicitly.

.. code-block:: python
   :name: guide-python-swap

   import quantsupport as qs

   reference = qs.Date(2026, 2, 24)
   quotes = qs.QuoteStore.from_dict({
       "reference_date": str(reference), "quotes": [
           {"identifier": f"ZeroRate_USD_SOFR_{tenor}", "mid": 0.03}
           for tenor in ["0D", "10Y"]
       ],
   })
   curve_spec = qs.CurveConfiguration(
       market_index=qs.MarketIndex.SOFR,
       quotes=quotes.identifiers(),
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
           qs.Request.Value, qs.Request.FairRate(0),
           qs.Request.Cashflows, qs.Request.Sensitivities,
       ])
       discount = context.curve("SOFR").discount_factor(reference + "5Y")
       print(result.price, result.fair_rate, discount)
       print(result.sensitivities)

The receive-fixed position has a negative PV because the continuous curve
implies floating coupons above the simple fixed coupon. The result's
``fair_rate`` is the annual decimal fixed coupon that balances the vanilla
swap under these conventions. Monetary values use the position's notional of
ten million USD; rates retain decimal units.

For a single-currency fixed-versus-floating swap the economic decomposition is

.. math::

   V_{\mathrm{receive\ fixed}}
   = \sum_i D(t,T_i)\,C_i^{\mathrm{fixed}}
     - \sum_j D(t,U_j)\,C_j^{\mathrm{floating}}.

The familiar par coupon :math:`K^*=PV_{\mathrm{float}}/A`, where
:math:`A=N\sum_i\alpha_iD(t,T_i)`, is a vanilla-swap relationship.
``Request.FairRate(0)`` solves the first leg's fixed coupon.
``Request.FairRate(1)`` solves the second leg's floating spread. Each request
uses the whole trade PV, including principal exchanges and each leg's native
curves, discount policy and direction; only the selected leg's quote changes.
Invalid indices fail explicitly. Each evaluation returns one scalar fair rate.

Interpreting results
--------------------

An :py:class:`~quantsupport.pricing.EvaluationResults` holds optional measures:
a property is ``None`` when it was not produced. ``Value`` requests monetary
PV, not a clean bond quote. ``Sensitivities`` returns a pandas table with
``pillar`` and ``value`` columns. Rows are raw derivatives with respect to
their named input, not automatically DV01 or vega-per-volatility-point.

For a decimal-rate risk factor :math:`q`, the first-order change for a +1
bp move is :math:`(\partial V/\partial q)10^{-4}`. The explicit curve above has two independently
labelled zero-rate inputs. Summing their derivatives estimates the effect of a
parallel move in those zero rates. A bootstrapped market reports derivatives
with respect to its calibration quotes instead. See :doc:`risk` for a
full-revaluation check.

``cashflows`` contains ``payment_date``, ``type``, ``amount``, ``fixing``,
``accrual_period``, ``currency``, ``caplet_strike``, ``floorlet_strike``, and
``leg_index``. For the generic discounted-cashflow pricer, amounts include leg
direction and reversed disbursements: receipts are positive and payments are negative.
The table may include initial disbursements and redemptions on both legs.
Summing ``amount`` therefore does not reproduce the swap PV. A reconciliation
must apply discounting and FX conversion where applicable. The table already
includes the position's cashflow signs.

The generic discounted-cashflow pricer includes all supplied cashflows,
including any with past payment dates. This example is valued at inception.
When valuing a seasoned contract, select payments according to the required
valuation convention. Specialized pricers have their own payment-selection
and reporting rules.

Context lifetime
----------------

Constructing :py:class:`~quantsupport.pricing.PricingContext` stores inputs.
Entering ``with context:`` validates them, activates the scope and enables
construction of requested curves, volatility objects, models and simulations.
Call ``context.initialize()`` to construct all configured objects eagerly.
Valuation services manage their differentiation recording within this
lifecycle.

Read constructed views inside their active scope. Final scope exit releases
constructed market data and performs the context's AD cleanup, including when
an exception propagates. Contexts and scenario stacks are thread-confined;
enter and exit on the owning thread in reverse order. Scalar results and
copied result tables can be retained, but an AD-backed curve view should not
be used after its context or scenario ends.

Public context input setters invalidate dependent construction. Store and
configuration getters return copies. To change an input, edit the copy and
assign it back to the context; dependent objects are then rebuilt for the next
valuation. The contract remains available for comparison across markets.

Dates, types, and protocols
-------------------------------

Financial dates accept the documented ``Date``, ISO text, or Python
``datetime.date`` forms. ``Date`` arithmetic with ``Period`` or a tenor such as
``"6M"`` is calendar arithmetic, not business-day adjustment. A ``Calendar``
supplies that adjustment. ``Schedule`` supports iteration and indexing and
returns copied dates and regularity flags; coupon accrual conventions are
separate from schedule generation.

The examples use Actual360 or Actual365. Two other day counters have current
implementation restrictions: ``ActualActual.year_fraction`` omits one complete
intervening year when endpoint years differ by at least two, and
``Thirty360US`` adjusts February month-ends independently after adjusting 31sts.
The latter ordering differs from common US/NASD/BondBasis rules. Validate
these conventions against the required contractual dates before using them.

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
``RuntimeError``. Use the error context to identify the missing input or incompatible
convention before changing the market specification.
