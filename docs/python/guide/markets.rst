Constructing and selecting a market
===================================

From observations to curves
-------------------------------

Curve construction translates observed prices and rates into discount
factors and forwards. A quote store holds the observations and reference date;
a curve configuration selects the calibration instruments and numerical
conventions. The constructed curve can then be queried or used for valuation.

This connection also determines risk reporting. A bootstrapped valuation can
report sensitivities to its OIS quotes, while a direct zero-rate curve
reports sensitivities to the rates supplied at its pillars.

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
       print(forward)

``nodes()`` reports interpolation dates and discount factors; ``pillars()``
reports labelled market inputs where available. Together they show how the
curve is represented and which inputs determine its sensitivities.
Construction may also need other projection or collateral curves. Supply
those dependencies under their configured market indices.

For a simple forward under the curve's time basis,

.. math::

   F(t;T_1,T_2)
   = \frac{D(t,T_1)/D(t,T_2)-1}{\tau(T_1,T_2)}.

Changing the day counter, compounding convention, or coupon schedule changes
the interpretation of a rate even when the displayed number is unchanged.
Log-linear discount-factor interpolation preserves a different curve shape
from linear zero-rate interpolation. Extrapolation is a modeling choice and
must be recorded explicitly.

The bootstrap uses native OIS calibration instruments with compounded
overnight coupons. Python's standalone ``Swap`` uses regular single-fixing
coupons; a standalone ``OvernightIndexSwap`` wrapper is currently unavailable.

Direct curve quotes
-------------------

Discount factors and zero rates can also be supplied directly in ``QuoteStore``.
Reference their identifiers in ``CurveConfiguration.quotes`` with the default
construction rule:

.. code-block:: python

   direct_quotes = qs.QuoteStore.from_dict({
       "reference_date": "2026-02-24",
       "quotes": [
           {"identifier": "DiscountFactor_USD_SOFR_1Y", "mid": 0.96},
           {"identifier": "DiscountFactor_USD_SOFR_2028-02-24", "mid": 0.91},
       ],
   })
   direct_spec = qs.CurveConfiguration(
       market_index="SOFR", quotes=direct_quotes.identifiers(),
       day_counter="Actual365", interpolator="LogLinear",
   )
   with qs.PricingContext(direct_quotes, [direct_spec]) as context:
       print(context.curve("SOFR").discount_factor("2027-02-24"))

Use ``DiscountFactor_CCY_Index_Pillar`` for dimensionless factors, or
``ZeroRate_CCY_Index_Pillar[_Compounding[_Frequency]]`` for annual decimal
zero rates. ``Pillar`` is a tenor such as ``1Y`` or an ISO date. Zero rates
default to ``Continuous`` compounding and ``Annual`` frequency; for example,
``ZeroRate_USD_SOFR_1Y_Compounded_Semiannual`` quotes a semiannually compounded
annual rate. The enclosing configuration supplies the day counter,
interpolator, and extrapolation setting. Interpolation applies to factors for
discount-factor quotes and to rates for zero-rate quotes.

Direct pillars are sorted by date. Each curve uses one direct quote family,
currency, and rate convention, with identifiers matching its market index.
Do not mix direct observations with calibration instruments in one curve.
Dates must be distinct and on or after the reference date. Discount factors
must be finite and positive; the reference-date factor is fixed at one and
is inserted automatically. A supplied reference-date factor must equal one,
and a discount curve needs at least one future pillar.

Direct quotes use the selected Mid/Bid/Ask level and retain their original
identifiers for scenarios and sensitivities. They can be components of
composite curves or dependencies of calibrated curves. All observed values stay
in ``QuoteStore``; configuration contains identifiers and construction conventions.

Composite curves and standalone term structures
-----------------------------------------------

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
``CurveConfiguration.composite(market_index=..., base_index=..., spread_index=...)`` and supply
both component configurations. Dependencies are resolved by construction;
cycles and missing parents are errors.

For a loan funding experiment, a useful specification is a benchmark curve, an
quoted spread curve identified by ``MarketIndex.other("FUNDINGSPREAD")``,
and their composite. Choose the composite through the fixed-income discount
policy rather than adding a discount index to the loan contract.

Quotes, fixings, FX, and policies
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

Supply verified historical observations. If the source has gaps, handle them
through an explicitly validated preprocessing policy. The native
``fill_missing_fixings`` routine uses reversed interpolation coordinates and
is unavailable through Python.

``FxStore`` stores rates in quote-currency units per base-currency unit.
For example ``{"base": "EUR", "quote": "USD", "rate": 1.1}`` means
1 EUR = 1.1 USD. Direct, inverse, and triangulated spot lookup are available.
Native Rust pricers use this store to convert monetary PVs and sensitivities to
``context.base_currency``, including FX conversion risk. Python's
``PricingContext.evaluate`` returns those native results.
Cashflow reports retain their original payment currencies. Missing conversion
paths raise an error when the native valuation requires conversion.

``SingleCurveCSADiscountPolicy`` defines collateral currency and remuneration
index for curve construction and derivative valuation. A different cashflow
currency may require its collateral-adjusted curve. ``FixedIncomeDiscountPolicy``
maps currencies to bond/loan discount curves and controls whether an instrument
index takes precedence. ``CsaTerms`` instead describes one netting agreement's
credit, funding, and collateral terms. These three choices answer different
financial questions.

When comparing Python and Excel valuations, use matching pricers, discount
policies and reporting currencies. The bindings expose different policy
controls, so a common base currency alone is insufficient to establish those
assumptions.

Volatility and calibration inputs
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

Surface and cube construction supports sparse slices and singleton axes.
The default extrapolation policy is ``Reject``, so each query must lie within
the selected slices. An ATM calibration strike can fall outside a smile even
when its expiry is covered. Choose an in-domain basket or an explicit ``Flat``
policy consistent with the intended boundary behavior.

``PricerConfiguration`` selects an instrument pricing implementation and
index-specific model assignments. ``SimulationConfiguration`` describes a
generated simulation. Calibrated Hull–White/LGM parameters consume the selected
quote side and configured volatility source. A calibrated sigma schedule is an
output of that fit; fixed ``sigma`` is an input in absolute short-rate units,
not an option-market volatility percentage.

Native serialization
--------------------

Configuration ``from_dict`` returns one configuration; ``from_dict_list``
returns a list and accepts a native object, array, or single-key
array wrapper where documented. Product ``from_dict`` instead calls the
product constructor using keyword terms. Do not confuse these two schemas.

Read files with Python's ``json.load`` and pass the decoded data to the appropriate
dictionary constructor:

.. code-block:: python

   import json

   with open("quotes.json", encoding="utf-8") as stream:
       quotes = qs.QuoteStore.from_dict(json.load(stream))
   with open("curve_specs.json", encoding="utf-8") as stream:
       curves = qs.CurveConfiguration.from_dict_list(json.load(stream))

Excel's JSON functions take JSON text in worksheet cells. Parameterized native indices use tagged
objects such as ``{"Equity": "SPX"}``, ``{"Credit": "ACME"}``, or
``{"FxPair": ["EUR", "USD"]}``; typed Python factories avoid reliance on
display-label parsing.

Historical repricing without look-ahead
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
           "reference_date": f"2026-02-{day:02d}", "quotes": [
               {"identifier": f"ZeroRate_USD_SOFR_{tenor}", "mid": rate}
               for tenor in ["0D", "10Y"]
           ],
       })
       specification = qs.CurveConfiguration(
           market_index="SOFR", day_counter="Actual360",
           interpolator="Linear",
           quotes=observations.identifiers(),
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

``Previous`` is the default and never chooses a later observation. ``Exact``
requires equality; ``Closest`` may choose a future market and resolves ties
to the earlier timestamp. Duplicate UTC instants are errors even if the input
strings use different offsets. Inspect the matched timestamp rather than
reporting the requested timestamp as though it were observed data.

Snapshots rebuild from construction inputs rather than serializing runtime
curves, caches, or AD state. Zero rates and discount factors use the selected
quote side, just like calibration instruments; FX spots are side-independent. Fixings are supplied separately to ``archive.pricing_context(selector, fixings=...)``; there is
no automatic publication-vintage filtering. For a historical backtest, supply
the fixing set that was known at each observation instant.

Continue with :doc:`valuation` to choose product-specific requests and models,
or study the :doc:`curve notebook <../studies/notebooks/curves>` for a larger
calibration investigation.
