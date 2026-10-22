Risk, netting, and extensibility
================================

Sensitivity labels define the experiment
----------------------------------------

A sensitivity measures how a valuation changes locally with one market input.
Under a bootstrapped market, that input can be a deposit, swap or volatility
quote. For an explicit zero curve, it is a dated rate pillar. A bond spread
can also be registered as an independent input.

The pillar label identifies the input, and its units determine how to scale
the derivative into a monetary change. Check those units and the valuation
currency before aggregating a portfolio risk report.

For a small input move,

.. math::

   \Delta V \simeq \sum_k \frac{\partial V}{\partial q_k}\Delta q_k.

Decimal-rate derivatives become approximate +1 bp currency changes by
multiplication by :math:`10^{-4}`. A lognormal volatility quoted as a decimal
typically moves by 0.01 for one absolute volatility percentage point; a
futures price uses price points instead. Neither conversion should be applied
to every row automatically. Curve sensitivities also include numerical calibration differentiation.
Compare selected derivatives with full revaluation to assess that local
approximation.

The following check reuses ``context`` and ``trade`` from
:doc:`first-valuation`. It shifts both explicit SOFR zero-rate pillars and
compares their summed derivative with central full revaluation.

.. code-block:: python
   :name: guide-python-scenario

   import math

   h = 0.00001
   up = qs.Scenario(shocks=[qs.Shock(
       target="SOFR", value=h, shock_type=qs.ScenarioType.Absolute,
   )])
   down = qs.Scenario(shocks=[qs.Shock(
       target="SOFR", value=-h, shock_type=qs.ScenarioType.Absolute,
   )])
   with context:
       base_result = context.evaluate(trade, ["Value", "Sensitivities"])
       parallel_derivative = base_result.sensitivities["value"].sum()
       with up:
           up_value = context.evaluate(trade).price
       with down:
           down_value = context.evaluate(trade).price
       restored_value = context.evaluate(trade).price
   finite_difference = (up_value - down_value) / (2 * h)
   assert math.isclose(parallel_derivative, finite_difference, rel_tol=2e-5)
   assert math.isclose(restored_value, base_result.price, abs_tol=1e-8)
   print("Approximate +1bp change:", parallel_derivative * 0.0001)

Enter a context before its ``Scenario`` scope. The innermost active context
supplies the scenario's base market. Requested shocked objects and their
dependencies are constructed in an independent cache. Nested scenarios compose
in entry order; exit restores the parent market, including after exceptions.
The same inactive scenario can be reused with another context. Do not retain a
shocked AD-backed market view after its scope ends.

An Absolute shock adds a value in the target's units; a Relative shock
multiplies by ``1 + value``. Targets use case-insensitive set membership of
underscore-separated tokens after an initial identifier-equality check.
Token order and multiplicity are ignored: a full identifier is
**not an exact-only selector**.
Repeated tenor/expiry tokens can make one full caplet ID select another expiry.
Overlapping shocks then compose in order without deduplication, potentially
moving one quote twice.

Use one unambiguous family selector when one family-wide move is intended;
for example ``CapletFloorlet_USD_SOFR`` rather than one shock per full caplet
ID. For an explicit funding spread curve, ``FUNDINGSPREAD`` selects its
pillars; appending a date narrows the token selector but does not enable an
exact-only mode. Inspect every transformed quote and explicit pillar, including
unexpected matches, and verify each intended input changes once before
interpreting a stress as a parallel market move. An aggregate NPV difference
alone cannot reveal a duplicated or unintended input move.

Portfolio aggregation
---------------------

The :doc:`portfolio aggregation notebook <../studies/notebooks/portfolio-aggregation>`
uses ``qs.Portfolio(trades).evaluate(context, requests, value_currency="USD")``
and returns standard ``EvaluationResults``. Its six-position deposit/swap book
matches the Excel portfolio workbook. It reconciles PV, every sensitivity and
payment-date totals against individual evaluations, then optionally exports
five tables as CSV.

``result.coupons`` preserves coupon types, accrual dates, contractual and
effective fixing indices, signed notionals and compatible weighted rates or
spreads. The two deposits combine to 400,000 at 5.5%, while SOFR and TermSOFR3m
coupons and opposing directions remain separate. ``result.cashflow_summary``
reports payment-date interest, capital, other and total amounts, without
allocating interest to accrual periods. Numerical aggregation follows each
trade's differentiation, avoiding a combined portfolio AD graph.

Value each explicit ``Trade`` under the same market assumptions, then aggregate
compatible monetary values and risk labels. Two opposite positions on the
same contract cancel only after side and size are applied; raw unsigned
cashflow amounts need not cancel. A sum of native-currency cashflows is not a
reporting-currency portfolio PV.

``EvaluationResults.sensitivities`` aggregates duplicate labels and sorts
them; ``raw_sensitivities`` retains the original pairs. For a portfolio, carry
trade identifiers alongside individual tables before grouping by pillar.
Keep model/input units in the report, and compare the aggregated first-order
prediction with a common full-revaluation scenario. Large shocks, options,
calibration changes, and exercise boundaries make linear predictions less
reliable.

Exposure profiles and netting
-----------------------------

``NettingSet`` groups trade positions under one client's ``CsaTerms``.
Supported Python contingent-claim conversion currently covers ``Swap``,
``CrossCurrencySwap``, ``BasisSwap``, ``FixFloatCrossCurrencySwap``,
``FixedRateBond``, ``FloatingRateNote``, and ``FixedRateDeposit``. It does not
accept every product that ordinary context evaluation can price; loans,
callable bonds, options, and custom Python pricers are not automatically XVA
claim implementations.

For path value :math:`V_p(t_i)`, the stored profiles are pointwise means:

.. math::

   \operatorname{EPE}(t_i)&=\frac{1}{P}\sum_p\max(V_p(t_i),0),\\
   \operatorname{ENE}(t_i)&=\frac{1}{P}\sum_p\min(V_p(t_i),0),\\
   \operatorname{EE}(t_i)&=\operatorname{EPE}(t_i)+\operatorname{ENE}(t_i).

ENE is signed and nonpositive. These profiles are neither PFE quantiles nor
time averages. Their values are in the engine's base currency as of each future
simulation date, before variation margin; they are not already discounted
adjustment PVs.

The example below groups the swap into a netting set and runs an XVA
calculation with variation margin. Its eight paths keep the example quick;
increase the path count and compare seeds for a numerical study.

.. code-block:: python
   :name: guide-python-xva

   xva_configuration = qs.XvaConfig.from_dict({
       "model_configs": [{
           "market_index": "SOFR", "lambda": 0.05,
           "parameter_source": {"Fixed": {"sigma": 0.005}},
       }],
       "n_paths": 8, "seed": 42, "frequency": "Quarterly",
   })
   terms = qs.CsaTerms(
       collateral_index="SOFR", collateral_currency="USD",
       credit_spread=0.01, recovery=0.40, funding_spread=0.005,
       variation_margin=True, counterparty_threshold=1000,
       own_threshold=1000, minimum_transfer_amount=500,
   )
   agreement = qs.NettingSet("CLIENT-A", [trade], terms)
   with context:
       adjustments = context.run_xva(xva_configuration, [agreement])
   print(adjustments.xva_values)
   profile = adjustments.exposures[0].to_dataframe()
   assert (profile["epe"] >= 0).all()
   assert (profile["ene"] <= 0).all()
   assert (profile["ee"] - profile["epe"] - profile["ene"]).abs().max() < 1e-8

High-level ``run_xva`` constructs CVA and FVA aggregators. Native DVA support is
a lower-level aggregator capability, not an additional high-level Python
result promised by this call. With flat credit spread :math:`s_c` and recovery
:math:`R`, the flat hazard approximation is :math:`\lambda=s_c/(1-R)`, not
:math:`\lambda=s_c`. A configured credit curve provides a different, explicitly
bootstrapped survival input.

FVA integrates signed path NPV, not EPE alone. Its right-endpoint time
increments use ACT/365; optional system discounting and variation-margin
treatment apply in the aggregator. Schematically,

.. math::

   \operatorname{FVA}
   \simeq \mathbb{E}\left[\sum_i
   D_{\mathrm{sys}}(0,t_i)\,s_f(t_i)\,
   \widetilde V_p(t_i)\,\Delta_i\right],

where :math:`\widetilde V` denotes the aggregator's margin-treated value.
Variation margin affects adjustment aggregation, not the stored pre-margin
exposure profiles. An explicit funding-spread zero curve can overlay a funding
index where configured; it is not a separate ``FundingSpreadCurve`` class.

Adjustment risk needs an additional coordinate check. With ``credit_index``,
the engine snapshots numeric survival nodes and their labels, then the CVA
factory creates independent survival-probability leaves. A ``CVA.<label>``
row therefore measures :math:`\partial\mathrm{XVA}/\partial S(T_k)`, not a
derivative with respect to the original CDS spread, even when the label looks
like a CDS quote identifier. No credit-bootstrap Jacobian is replayed along
this aggregator path.

Likewise, curve-driven FVA creates independent derived funding-spread leaves,
prefixed ``FVA.``; it does not replay the original funding-curve calibration.
These are spread-pillar coordinates, not original funding-quote risk. Flat
``credit_spread`` and ``funding_spread`` parameters instead retain their own
decimal-rate units. Do not scale a survival-probability derivative as though it
were a CDS-spread DV01. Ordinary deterministic CDS pricing is a different
differentiation graph and must be interpreted separately.

System/base discount factors supplied to the adjustment aggregators are
numerical snapshots held fixed in their weights; this contribution is not
differentiated system-curve risk. Calibrated sigma also omits curve/strike
recalibration terms. Differentiable model-backed paths do not restore those
omitted dependencies. Interpret the returned coordinates as partial risks and
check proposed hedges by full market revaluation, rather than treating a
quote-like label as a complete recalibrated market-quote derivative.

Thresholds, minimum transfer amount, recovery, credit/funding indices, and the
model/path/date grid are economic and numerical assumptions. Netting-set names
must be unique in one run. Fixed seeds allow reproducible comparisons. Vary path counts and seeds to
assess whether a client-level change exceeds sampling variation. The
:doc:`XVA study <../studies/notebooks/xva>` develops the comparison.

Extending valuation in Python
-----------------------------

The binding exposes extension interfaces rather than requiring new native
code for every product. ``Instrument`` describes terms, ``Trade`` a position,
``Pricer`` the valuation method, ``MarketDataProvider`` the required state, and
``Evaluator`` dispatch. Structural registration also permits ordinary Python
classes and dataclasses. A custom implementation owns its treatment of side,
position size, requests, units, and cashflow filtering.

This example implements a deterministic USD receipt using the same market as
the first valuation.

.. code-block:: python
   :name: guide-python-extension

   class Receipt(qs.Instrument):
       def __init__(self, identifier, maturity, amount):
           super().__init__(identifier)
           self.maturity = maturity
           self.amount = amount

   class ReceiptTrade(qs.Trade):
       pass

   class ReceiptPricer(qs.Pricer):
       def evaluate(self, trade, requests, context):
           instrument = trade.instrument
           size = trade.notional if trade.notional is not None else 1.0
           value = (
               instrument.amount * size * trade.side.sign()
               * context.curve("SOFR").discount_factor(instrument.maturity)
           )
           return qs.EvaluationResults(
               context.evaluation_date, instrument.identifier,
               price=value if qs.Request.Value in requests else None,
           )

   receipt = Receipt("RECEIPT", reference + "1Y", 100)
   receipt_trade = ReceiptTrade(receipt, reference)
   context.register_pricer(ReceiptTrade, ReceiptPricer())
   with context:
       custom_result = context.evaluate(receipt_trade, ["Value"])
       assert 0 < custom_result.price < 100
   assert context.unregister_pricer(ReceiptTrade)

Registration selects the exact trade type first and then compatible bases;
Python registrations override built-in dispatch. Register the trade type, not
the contractual instrument class. An unregistered trade subclass containing a
native instrument still uses native pricing. A standalone ``Evaluator`` can
instead use a custom provider; it is not necessary to subclass every supplied
base interface.

The custom pricer returns Value. Additional measures require their own
implementation and validation: arbitrary Python arithmetic is outside native
AD, and native XVA requires a supported claim conversion.
