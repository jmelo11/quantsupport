Constructing and Selecting a Market
===================================

From Observations to Curves
-------------------------------

A quote store records observations and a reference date; it is not a discount
curve. A construction configuration determines how those observations become
pricing objects. This distinction matters in risk: differentiating a
bootstrapped price with respect to an OIS quote is not the same experiment as
moving an interpolated zero rate.

The example below bootstraps a USD SOFR curve from a deposit and OIS quotes.
The identifiers encode instrument and market conventions understood by the
native quote parser. They are not arbitrary maturity labels.

.. code-block:: python
   :name: guide-python-bootstrap

   import quantsupport as qs

   observations = [
       ("FixedRateDeposit_USD_SOFR_1D", 0.04500),
       ("OIS_USD_SOFR_1Y", 0.04837),
       ("OIS_USD_SOFR_2Y", 0.04528),
       ("OIS_USD_SOFR_5Y", 0.04077),
   ]
   bootstrap_quotes = qs.QuoteStore.from_dict({
       "reference_date": "2026-02-24",
       "quotes": [{"identifier": key, "mid": value}
                  for key, value in observations],
   })
   bootstrap_spec = qs.CurveConfiguration(
       market_index="SOFR", quotes=[key for key, _ in observations],
       day_counter="Actual360", interpolator="LogLinear",
       enable_extrapolation=False,
   )
   bootstrap_context = qs.PricingContext(
       bootstrap_quotes, [bootstrap_spec],
       csa_policy=qs.SingleCurveCSADiscountPolicy("USD", "SOFR"),
   )
   with bootstrap_context:
       curve = bootstrap_context.curve("SOFR")
       print(curve.nodes())
       print(curve.pillars())
       forward = curve.forward_rate(
           "2027-02-24", "2028-02-24",
           qs.Compounding.Simple, qs.Frequency.Annual,
       )
       assert forward > 0
       assert curve.discount_factor("2028-02-24") < 1

``nodes()`` describes the curve's interpolation dates and discount factors.
``pillars()`` describes labeled market inputs where available. Neither should
be mistaken for a complete historical quote table. Construction may require
other configured projection or collateral curves; a missing dependency is an
error, not a request to substitute a nearby index.

For a simple forward under the curve's time basis,

.. math::

   F(t;T_1,T_2)
   = \frac{D(t,T_1)/D(t,T_2)-1}{\tau(T_1,T_2)}.

Changing the day counter, compounding convention, or coupon schedule changes
the interpretation of a rate even when the displayed number is unchanged.
Log-linear discount-factor interpolation preserves a different curve shape
from linear zero-rate interpolation. Extrapolation is a modeling choice and
must be recorded explicitly.

Native OIS quote bootstrapping is available, but the Python binding does not
expose a standalone ``OvernightIndexSwap`` instrument. A vanilla ``Swap`` is
not a substitute for an OIS contract with different compounding semantics.

Explicit and Composite Construction
-----------------------------------

:doc:`first-valuation` used ``CurveConstruction.zero_rate``. Its dates or tenors
are measured from the context reference date, and each pillar has one annual
decimal rate. The enclosing ``CurveConfiguration`` supplies day count,
interpolation, and extrapolation; the zero-rate rule supplies rate compounding
and frequency.

The separate ``ZeroRateTermStructure`` and ``TenorBasedZeroRateTermStructure``
classes are reusable numeric term structures. Their defaults are Actual365,
Continuous, Annual, Linear, and disabled extrapolation. A configuration's
defaults are different: Actual360, LogLinear, and enabled extrapolation.
Choose conventions rather than transferring a number between these APIs and
assuming identical behavior. A tenor curve can be serialized as the equivalent
dated zero-rate curve.

A composite curve multiplies two component discount factors:

.. math::

   D_{\mathrm{composite}}(t,T)
   =D_{\mathrm{base}}(t,T)D_{\mathrm{spread}}(t,T).

Continuous zero rates add when their time bases agree. This is a
discount-factor composition, not an arithmetic average of quoted yields.
Configure it with
``CurveConstruction.composite(base_index=..., spread_index=...)`` and supply
both component configurations. Dependencies are resolved by construction;
cycles and missing parents are errors.

For a loan funding experiment, a useful specification is a benchmark curve, an
explicit spread curve identified by ``MarketIndex.other("FUNDINGSPREAD")``,
and their composite. Choose the composite through the fixed-income discount
policy rather than adding a discount index to the loan contract.

Quotes, Fixings, FX, and Policies
-------------------------------------

Quote levels are selected with ``quote_level=Level.Mid``, ``Bid``, or ``Ask``.
The default is Mid. Every quote consumed by construction or calibration must
contain the selected side; missing Bid/Ask values do not fall back to Mid.
Quote units depend on the instrument: a decimal rate, a futures price, and a
volatility quote are not transformed into a common percentage convention.

Historical fixings are separate observations. ``FixingStore.add(index, date,
value)`` records a published fixing, while a forward curve projects an
unobserved future reset. A contract that needs a past fixing should fail when
that observation is missing; a zero fixing is not an acceptable substitute.
Read the product's fixing calendar and lag independently of its payment
schedule.

Missing historical observations require an explicit data-quality decision,
not a silent interpolation assumption. The native ``fill_missing_fixings``
routine is not a Python binding operation and currently uses reversed
interpolation coordinates. It is not a reliable reference for imputation;
supply verified observations or document an independently validated
preprocessing policy.

``FxStore`` stores rates in quote-currency units per base-currency unit.
For example ``{"base": "EUR", "quote": "USD", "rate": 1.1}`` means
1 EUR = 1.1 USD. Direct, inverse, and triangulated spot lookup are available.
An FX spot store does not automatically convert every returned price or
cashflow report to a user-chosen reporting currency.

``SingleCurveCSADiscountPolicy`` defines collateral currency and remuneration
index for curve construction and derivative valuation. A different cashflow
currency may require its collateral-adjusted curve. ``FixedIncomeDiscountPolicy``
maps currencies to bond/loan discount curves and controls whether an instrument
index takes precedence. ``CsaTerms`` instead describes one netting agreement's
credit, funding, and collateral terms. These three choices answer different
financial questions.

Python and Excel share native components, but their exposed policy controls
are not identical. Document the actual pricer/policy path before comparing
cross-interface prices; do not infer universal reporting conversion or
discount-policy parity merely because both interfaces accept a base currency.

Volatility and Calibration Inputs
---------------------------------

A volatility configuration identifies quote inputs, smile coordinates, and
construction policies. A constructed ``VolatilitySurface`` or
``VolatilityCube`` is a read-only context view, not a replacement quote store.
Rate strikes are annual decimal rates; equity and FX strikes are absolute
prices under their own currency conventions.

The configured volatility type matters: a Black lognormal volatility is not a
normal volatility or an absolute short-rate sigma. Inspect ``volatility_type``
and ``smile_type`` before interpreting a surface lookup. For a cube, expiry
and underlying tenor are separate coordinates.

Native multidimensional construction can retain sparse slices and singleton
axes. It does not manufacture a complete grid. The default native surface
extrapolation policy is ``Reject``; an ATM calibration basket can fall outside
a quoted strike slice even when all expiry dates look valid. Either choose an
in-domain basket or specify and explain an explicit ``Flat`` policy. Silently
clamping an out-of-domain calibration is a different model assumption.

``PricerConfiguration`` selects an instrument pricing implementation and
index-specific model assignments. ``SimulationConfiguration`` describes a
generated simulation. Calibrated Hull–White/LGM parameters consume the selected
quote side and configured volatility source. A calibrated sigma schedule is an
output of that fit; fixed ``sigma`` is an input in absolute short-rate units,
not an option-market volatility percentage.

Native Serialization
--------------------

Configuration ``from_dict`` returns one configuration; ``from_dict_list`` and
``from_json(path)`` return lists and accept a native object, array, or single-key
array wrapper where documented. Product ``from_dict`` instead calls the
product constructor using keyword terms. Do not confuse these two schemas.

Python ``from_json`` takes a filesystem path. Excel's similarly named functions
take JSON text in worksheet cells. Parameterized native indices use tagged
objects such as ``{"Equity": "SPX"}``, ``{"Credit": "ACME"}``, or
``{"FxPair": ["EUR", "USD"]}``; typed Python factories avoid reliance on
display-label parsing.

Historical Repricing Without Look-Ahead
---------------------------------------

A historical snapshot captures one coherent market: quotes and construction,
volatility, simulation, and pricer inputs. It is selected by observation
timestamp, while its quote reference date remains the valuation date.
The following independent example makes that distinction explicit.

.. code-block:: python
   :name: guide-python-history

   import quantsupport as qs

   def recorded_market(day, rate):
       observations = qs.QuoteStore.from_dict({
           "reference_date": f"2026-02-{day:02d}", "quotes": [],
       })
       specification = qs.CurveConfiguration(
           market_index="SOFR", day_counter="Actual360",
           interpolator="Linear",
           construction=qs.CurveConstruction.zero_rate(
               pillars=["0D", "10Y"], rates=[rate, rate],
           ),
       )
       return qs.PricingContext(observations, [specification])

   archive = qs.MarketSnapshotStore()
   for day, rate in [(23, 0.03), (24, 0.031)]:
       snapshot = qs.MarketSnapshot.from_context(
           f"2026-02-{day:02d}T16:00:00Z", recorded_market(day, rate),
       )
       archive.insert(snapshot)
   selector = qs.SnapshotSelector(
       "2026-02-25T08:00:00+00:00",
       rule=qs.SnapshotSelectionRule.Previous, level=qs.Level.Mid,
   )
   selected = archive.select(selector)
   with archive.pricing_context(selector) as historical:
       assert str(historical.evaluation_date) == "2026-02-24"
       assert historical.snapshot_timestamp == selected.timestamp
       assert historical.curve("SOFR").discount_factor("2027-02-24") < 1
   assert len(archive.timestamps) == 2

``Previous`` is the default and never chooses a later observation. ``Exact``
requires equality; ``Closest`` may choose a future market and resolves ties
to the earlier timestamp. Duplicate UTC instants are errors even if the input
strings use different offsets. Inspect the matched timestamp rather than
reporting the requested timestamp as though it were observed data.

Snapshots rebuild from construction inputs rather than serializing runtime
curves, caches, or AD state. Explicit zero rates and FX spots are
side-independent. Fixings are supplied separately to
``archive.pricing_context(selector, fixings=...)``; there is no automatic
publication-vintage filtering. A rigorous backtest must supply the fixing set
that was actually known at each historical observation.

Continue with :doc:`valuation` to choose product-specific requests and models,
or study the :doc:`curve notebook <../studies/notebooks/curves>` for a larger
calibration investigation.
