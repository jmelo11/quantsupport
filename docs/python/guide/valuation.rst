Valuation, quotation, and model choice
==========================================

Each product has its own quotation conventions and supported analytics.
``Value`` requests monetary PV, while ``FairRate`` requests the product's
breakeven rate or price. Bonds additionally use price or spread anchors to
specify the valuation.

The examples below reuse ``reference`` and ``context`` from
:doc:`first-valuation`. Enter the context again for each calculation; its
market objects are reconstructed as needed.

Build products explicitly
-------------------------

Makers retain construction parameters and expose ``build()`` and
``build_using(context)``. Both return the same concrete product type.

``maker.is_resolved()`` reports whether date inputs and required market
observations are already resolved. ``build()`` checks this first; an unresolved error identifies
the missing input and explains how to supply it or call ``build_using(context)``.
Contract validation still occurs when the maker is ready.
``build()`` raises ``UnresolvedProductErr`` for unresolved dates or market
observations. Use ``build_using(context)`` for date tenors, including maturity
tenors paired with an explicit start date.

.. code-block:: python

   maker = qs.MakeSwap(
       identifier="spot-5y", start_tenor="0D", tenor="5Y",
       notional=1_000_000, fixed_rate=0.03, currency="USD", market_index="SOFR",
   )
   with context:
       instrument = maker.build_using(context)
       trade = qs.Trade(instrument, context.evaluation_date)
       result = context.evaluate(trade)

The maker remains reusable. The built instrument retains its dates and
contractual principal across valuations and scenarios. ``evaluate`` accepts
concrete trades; it does not build products or supply execution dates.
``MakeTrade(..., trade_tenor="0D").build_using(context)`` provides an
explicit construction path for a relative execution date.

Scheduled makers use ``start_tenor`` and ``tenor``; options use ``expiry_tenor``
and forwards use ``delivery_tenor``. Swaptions use ``expiry_tenor`` and an
underlying ``tenor``. A date and tenor for the same field are mutually exclusive.

``PricingContext(anchor_fixings=True, ...)`` requires stored index fixings
strictly before evaluation. Today and future observations project, even when
today's fixing is stored. Set ``context.anchor_fixings = False`` to project all
observations; the curve must support the observation interval.

Floating amortizing loans require an explicit ``amortization_rate`` or
``MakeLoan(...).build_using(context)``. Construction obtains the initial
index observation, spread, and payoff before fixing the principal schedule.
Missing historical observations fail during construction. Python loan
serialization retains the initial amortization rate for context-free reconstruction.

Rates and direction
-------------------

Vanilla swaps, basis swaps, both cross-currency swap forms, deposits, and
floating-rate notes expose contractual schedules and rate conventions.
Their projections require the appropriate index curves and any historical
fixings. Each constructor defines its leg ordering and the meaning of receive/pay
side. Use those definitions when assigning the position's direction.

``Trade.notional`` overrides the position size without modifying the shared
instrument. Cross-currency positions have domestic and foreign overrides;
futures use ``num_contracts``. Position amounts are finite and nonnegative in
the Python interface. FX forwards, equity forwards, options, CDS, and caplets
have no contractual notional field in their Python product specification;
their size is carried by the trade, normally defaulting to one.

For rate futures, a price such as 95.25 implies an annual decimal rate of
0.0475. ``contract_size`` is currency per **price point**, not currency per
basis point. The default 2500 therefore corresponds to 25 currency units for a
0.01 price-point change before contract-count and side conventions. Do not
feed a futures price to an API expecting a decimal interest rate.

An ``InterestRate`` stores one rate and its quotation conventions, not a term
structure. Use its compound/discount factor operations for that stored
quotation and a constructed curve's forward query for market-dependent forwards.

Straight bonds: price is a quotation convention
---------------------------------------------------

Bond terms describe coupon, principal, schedule, and currency. The position
owns the observed quote and valuation choices:
``pricing_anchor``, ``settlement_date``, and ``spread_day_counter``.
An explicit anchor is required; absence does not mean a zero spread.

The next calculation asks what continuously compounded Z-spread is consistent
with a clean quote of 99.25. Clean and dirty prices are quoted per the
instrument's quotation units, normally 100; returned ``price`` is monetary PV.
Accrued interest distinguishes the two quotation anchors.

.. code-block:: python
   :name: guide-python-bond

   bond = qs.FixedRateBond(
       "USD-BOND", reference, reference + "5Y", 1_000_000,
       0.04, "USD", day_counter="Actual360",
       compounding="Simple", rate_frequency="Annual",
       payment_frequency="Semiannual",
   )
   bond_trade = qs.Trade(
       bond, reference,
       pricing_anchor=qs.BondPricingAnchor.clean_price(99.25),
       settlement_date=reference,
   )
   with context:
       bond_result = context.evaluate(bond_trade, [
           "Value", "YieldToMaturity", "ModifiedDuration",
           "ZSpread", "Sensitivities",
       ])
       print(bond_result.price, bond_result.z_spread)
       print(bond_result.yield_to_maturity, bond_result.modified_duration)

``BondPricingAnchor.z_spread(0.0)`` instead asks for the unspread model value;
``dirty_price(...)`` includes accrued interest. ``with_pricing_anchor`` creates
a new trade rather than mutating contractual cashflows. Curve risk and the
bond's ``<identifier>.ZSpread`` risk are different derivative coordinates.
A model-implied spread from a price anchor is a calibration result, not a
second independently observed market quote.

The context's fixed-income discount policy maps currency to curve. When
``prefer_instrument_index=True``, straight products with an explicit contractual
discount index can take precedence; setting it False enforces the mapping.
The pricer reports an error for an unmapped payment currency.

Issuer-callable stepped bonds
---------------------------------

Use ``SteppedCouponBond`` for stepped coupons, amortization and issuer calls. Coupon
templates and outstanding-notional percentages are positional, and their last
entries repeat through subsequent periods. ``PayoffOps`` represents native
coupon algebra, for example a geared floating coupon with floor and cap.
Serialized coupon objects retain this expression tree.

Calls are issuer calls with explicit strikes and either a discrete date or an
inclusive date window. On an exercise date, same-day contractual coupons and
amortization are paid; the call strike caps the remaining value. A call strike
is quoted in the bond's units, not as a decimal interest rate.

Choose ``TreeBased`` or ``BackwardFourierEvolution`` through a
``PricerConfiguration`` with an appropriate Hull–White model. For example, the
configuration below specifies a tree with fixed mean reversion and short-rate
volatility.

.. code-block:: python

   tree_configuration = qs.PricerConfiguration.from_dict({
       "instrument_type": "SteppedCouponBond",
       "pricer": {"TreeBased": {"time_grid": {
           "steps_per_year": 48, "max_steps": 4096,
           "day_counter": "Actual365",
       }}},
       "models": [{"market_index": "SOFR", "model": {"HullWhite": {
           "alpha": 0.05, "parameter_source": {"Fixed": {"sigma": 0.01}},
       }}}],
   })

With calls present, use ``BondPricingAnchor.OAS(0.0)`` for a zero-OAS model
value, or a clean/dirty anchor to imply OAS. Without call entries use a
Z-spread anchor. Yield to maturity and modified duration are intentionally not
defined for this stepped/callable product. Tree/Fourier grid and model choices
are numerical assumptions; convergence and calibration diagnostics belong in
the valuation record. The
:doc:`callable-bond study <../studies/notebooks/callable-bond>` works through
these choices.

Loans: contractual principal and expected receipts
------------------------------------------------------

A loan separates the contractual repayment schedule from the valuation of
prepayment and default. Specify ``LoanRepaymentStructure`` and one or more
native coupon templates; select discounting through the context/pricer policy,
not a ``discount_index`` product field. French amortization fixes principal from
the initial amortization rate; later floating coupon changes do not silently
rewrite the original principal schedule.

Set accrual conventions on the coupon: floating coupons use their explicit
``day_counter``, or the index convention when omitted; fixed coupons use their
``InterestRate`` day counter. The loan uses the same convention for principal
amortization. The former ``Loan(day_counter=...)`` argument has been removed;
move it to each coupon. French payments are level through the first reset when
``amortization_rate`` matches that reset's all-in fixing, including spread and
payoff bounds. Later resets can change payments without changing principal.

.. code-block:: python
   :name: guide-python-loan

   loan = qs.Loan(
       "USD-LOAN", reference, reference + "2Y", 100_000,
       [qs.FixedRateCoupon(rate=qs.InterestRate(
           rate=0.06, day_counter="Actual360",
       ))],
       "USD", repayment=qs.LoanRepaymentStructure.bullet(),
       payment_frequency="Quarterly",
       prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.10),
   )
   original_terms = loan.to_dict()
   with context:
       loan_result = context.evaluate(
           qs.Trade(loan, reference),
           ["Value", "Cashflows", "Sensitivities"],
       )
       print(loan_result.price)
   assert loan.to_dict() == original_terms

``Value`` is signed PV of future expected receipts, **excluding the initial
advance**. A lender's origination NPV therefore needs the advance considered
separately. The loan cashflow report is pricer-specific: scheduled, prepayment,
and recovery receipts occupy legs 0, 1, and 2. Do not apply the generic swap
cashflow-table interpretation indiscriminately to this report.

Prepayment may be absent, constant annual CPR, or refinancing-incentive driven.
The latter requires ``refinancing_benchmark_index`` in the context's native
``PricerConfiguration.loan(...)``; the
benchmark is an index rate, excluding replacement-loan lending spreads and
fees. Payment frequency, reset frequency, fixing lag, and observation tenor are
independent contractual inputs. ``coupon_fixing_requests()`` identifies required
historical observations, not a promise that missing fixings will be projected.

``Request.FairRate(index)`` selects a zero-based contiguous loan coupon/spread
segment and returns its scalar in ``fair_rate``. It solves earlier segments
first, shortening the loan and repaying remaining capital at each segment end;
solved rates stay fixed when extending to the requested segment. Mixed and
stepped schedules follow this same rule. The original contract is unchanged.
Each evaluation accepts one target; invalid indices and bounded coupons fail.
The search interval for each segment is [-5%, 50%].
``rebuild_amortization_for_fair_rate=False`` preserves principal during trial rates; True reconstructs trial
amortization without changing the original contract. Choose the setting that
matches whether the proposed new coupon should also change contractual
principal repayments. Set these options through native pricer configuration
and keep using ``context.evaluate(...)``:

.. code-block:: python

   context.pricer_configurations = [qs.PricerConfiguration.loan(
       rebuild_amortization_for_fair_rate=True,
       refinancing_benchmark_index=qs.MarketIndex.SOFR,
   )]
   with context:
       par = context.evaluate(qs.Trade(loan, reference), [qs.Request.FairRate(0)])

The context dispatches ordinary ``qs.Trade`` loan positions directly to the
core Rust ``LoanPricer``, including portfolio members and scenarios. Discounting
follows ``context.fixed_income_policy``. Omitting the loan configuration uses
native defaults: no refinancing benchmark and preserved amortization during
fair-rate trials. The :doc:`loan study <../studies/notebooks/loans>` reconciles the settings.

Options, calibration, and simulation
------------------------------------

Caps/floors, caplets/floorlets, European equity/FX options, and European
swaptions expose distinct pricers and market requirements. The rate cap
interfaces select Black pricing; an equity option needs its spot fixing and
forward/dividend conventions; a swaption needs an explicit
``EuropeanSwaption``/``HullWhiteClosedForm`` configuration.

This swaption calculation reuses the transparent curve market, with a fixed
absolute short-rate sigma. It is a model illustration rather than a fit to a
quoted volatility surface.

.. code-block:: python
   :name: guide-python-swaption

   model_configuration = qs.PricerConfiguration.from_dict({
       "instrument_type": "EuropeanSwaption",
       "pricer": "HullWhiteClosedForm",
       "models": [{"market_index": "SOFR", "model": {"HullWhite": {
           "alpha": 0.1, "parameter_source": {"Fixed": {"sigma": 0.01}},
       }}}],
   })
   context.pricer_configurations = [model_configuration]
   swaption = qs.EuropeanSwaption(
       "PAYER-1Y5Y", reference + "1Y", reference + "6Y",
       0.03, 1_000_000, "USD", "SOFR", swaption_type="Payer",
   )
   with context:
       option_result = context.evaluate(
           qs.Trade(swaption, reference), ["Value", "Sensitivities"],
       )
       print(option_result.price)

For calibration replace the fixed parameter source with ``Calibrated``,
specifying ``Surface`` or ``Cube`` and an explicit calibration basket, and
supply that source's quotes/configuration to the context. Cube baskets select
one tenor per expiry. Quote side and strike-domain policies apply during calibration as well as
later volatility queries.

Prefer absolute numeric option strikes, as in the example. A serialized
``Relative`` strike does not have one library-wide financial interpretation:
the generic native ``Strike.resolve`` helper uses :math:`K=F+\mathrm{offset}`,
while Black and Hull–White cap/caplet pricers independently use
:math:`K=F(1+\mathrm{pct})`. Thus a caplet payload ``{"Relative": 0.10}`` turns a
4% forward into a 4.4% strike, not a 14% strike. The equity/FX helper convention
must not be transferred to a rate-option consumer. Volatility lookup
coordinates are also distinct from the payoff strike; inspect both before
comparing valuations.

Constructed Hull–White simulation diagnostics, when available, report
identifier, expiry, times ``t``/``big_t``, market volatility, market/model prices,
calibrated sigma, forward, strike, and signed ``price_error`` (model minus
market). Fixed-parameter or other model simulations can return no diagnostics.

Generated simulation paths are materialized from numeric inputs; their storage
does not retain calibration pillars or imply differentiated spot/volatility
paths. This differs from model-backed ``LgmMarketModel`` computations used by
scripting and XVA. The standalone Hull–White path generator uses Euler state evolution and
requires positive alpha. Assess its discretization by refining the grid and
its sampling precision by comparing path counts and seeds. Direct LGM dynamics
support a separate zero-lambda limit.

Coupon interest between two dates
----------------------------------

We use ``Request.AccruedAmount(start, end)`` to calculate signed, undiscounted
interest earned in a reporting window. The loan and linear cashflow pricers
resolve observations, clip the window to each coupon's accrual period, and
exclude principal advances and repayments. Results use the context base
currency; portfolios aggregate the member amounts.
Accrual arithmetic uses plain ``f64`` values. An accrual request does not
compute PV or start an AD tape; valuation and sensitivity requests use the
differentiable pricing path.

.. code-block:: python
   :name: guide-python-accrual

   accrual_deposit = qs.FixedRateDeposit(
       identifier="ACCRUAL", start_date=reference,
       maturity_date=reference + qs.Period(1, "Years"),
       notional=1_000_000, rate=0.04, currency="USD", discount_index="SOFR",
       day_counter="Actual360", compounding="Simple",
   )
   accrual_portfolio = qs.Portfolio("ACCRUAL", [qs.Trade(accrual_deposit, reference)])
   with context:
       earned = context.evaluate(
           accrual_portfolio,
           [qs.Request.AccruedAmount(reference, reference + qs.Period(1, "Months"))],
       ).accrued_amount

Each evaluation accepts one accrual window. Equal start/end dates select zero;
reversed dates are rejected. Loan windows start at or after the context
evaluation date, because native loan projection does not reconstruct historical
prepayments or already-paid coupons. Partial overnight accrual requires daily
observation amounts and is rejected by this interface.

At coupon level, ``coupon.accrued_amount(start, end)`` uses the same date-window
semantics. Floating and option-embedded coupons require a resolved observation
for nonzero accrual. Adjacent windows use differences in cumulative accrual
measured from the contractual coupon start. This preserves reconciliation for
30/360 conventions as well as Actual/360 and Actual/365. Cashflow and accrual
requests resolve their inputs without requiring a discounted value of historical
principal advances.

FX, equity, and credit
--------------------------

Use ``MarketIndex.equity("SPX")`` for a typed equity identity and
``MarketIndex.fx_pair("EUR", "USD")`` for a pair. A direct Python string is
accepted only where its documented native enum form is supported.
The equity forward needs an evaluation-date spot fixing and a forward curve;
absence of a dividend curve means zero dividends.

FX direction is quoted currency per base unit. A market-implied forward and a
contracted forward are different values. The current FX-forward pricer uses
``spot * df_quote / df_base``; the FX-option path uses its separately
implemented base/quote carry convention. Do not silently replace one formula
with the other in a parity claim. Check the actual instrument pricer, discount
policy, and returned currency before constructing a hedge.

CDS valuation additionally requires a survival/credit curve and recovery.
``CreditCurveConfiguration`` bootstraps native credit quotes and can be supplied
as ``credit_curves`` to the context. Recovery and spread units are explicit
inputs; a flat-spread hazard approximation is not the same model as a
bootstrapped survival term structure.

Scripted payoffs
--------------------

Python's scripting bridge evaluates dated payoff events through the native
engine. A script can retain state across events and declare payments that are
discounted by its market model. The example below pays 100 USD in two years
under a zero-volatility model and compares its value with the context curve.

.. code-block:: python
   :name: guide-python-script

   script = {
       "name": "SCRIPTED-PAYMENT", "currency": "USD",
       "result_variable": "note",
       "events": [{
           "date": str(reference),
           "code": 'note = 0; note pays 100 on "2028-02-24";',
       }],
       "models": [{"market_index": "SOFR", "model": {"Lgm": {
           "lambda": 0.05, "parameter_source": {"Fixed": {"sigma": 0.0}},
       }}}],
   }
   with context:
       scripted_values = context.evaluate_script(script, n_paths=8, seed=42)
       expected = 100 * context.curve("SOFR").discount_factor("2028-02-24")
       assert abs(scripted_values["note"] - expected) < 1e-8

``evaluate_script`` returns a plain dictionary of averaged numeric variables,
not Python sensitivity or cashflow tables. Models must be configured for the
discount and requested forward indices; there is no implicit zero-volatility
fallback. Context pricer assignments are overridden by simulation assignments,
then by inline script assignments. A script in a non-base currency requires
an explicit discount index. This bridge currently assembles rate models, not a
general correlated FX/equity-factor system.

With stochastic models, comparisons also depend on the estimator. Martingale
controls can adjust the selected result variable, while native cashflow
diagnostics retain raw sampled payment PVs. Their sum can therefore differ
from the controlled result at finite path counts. Select a variable that
accumulates the intended payments and assess differences using the sampling
error and comparison tolerance.

Continue with :doc:`risk` for full-revaluation checks and exposure aggregation,
or consult the :doc:`official studies <../studies/index>` for larger product
and calibration experiments.
