"""Direct curve observations share quote sides, scenarios, and source risk."""

import math
import tempfile
import unittest
from pathlib import Path

import quantsupport as qs


REFERENCE = "2026-02-24"


def context(observations, *, enable_extrapolation=False, **kwargs):
    return qs.PricingContext(
        qs.QuoteStore.from_dict({
            "reference_date": REFERENCE,
            "quotes": [{"identifier": label, **levels} for label, levels in observations],
        }),
        [qs.CurveConfiguration(
            market_index="ESTR", quotes=[label for label, _ in observations],
            day_counter="Actual365", interpolator="Linear", enable_extrapolation=enable_extrapolation,
        )],
        **kwargs,
    )


class DirectCurveQuoteTests(unittest.TestCase):
    def test_discount_factors_sort_dates_and_tenors_and_fix_the_origin(self):
        observations = [
            ("DiscountFactor_EUR_ESTR_2Y", {"mid": 0.90, "bid": 0.89, "ask": 0.91}),
            ("DiscountFactor_EUR_ESTR_2027-02-24", {"mid": 0.96, "bid": 0.95, "ask": 0.97}),
            ("DiscountFactor_EUR_ESTR_0D", {"mid": 1.0, "bid": 1.0, "ask": 1.0}),
        ]
        for level, expected in [(qs.Level.Mid, 0.96), (qs.Level.Bid, 0.95), (qs.Level.Ask, 0.97)]:
            with self.subTest(level=level), context(observations, quote_level=level) as ctx:
                curve = ctx.curve("ESTR")
                self.assertEqual(curve.discount_factor(REFERENCE), 1.0)
                self.assertAlmostEqual(curve.discount_factor("2027-02-24"), expected)
                weight = qs.DayCounter.Actual365.year_fraction(REFERENCE, "2026-08-25")
                self.assertAlmostEqual(curve.discount_factor("2026-08-25"), 1 + weight * (expected - 1))
                self.assertEqual(set(curve.pillars().label), {label for label, _ in observations})
                with self.assertRaises(qs.QuantSupportError):
                    curve.discount_factor("2029-02-24")

    def test_zero_rate_conventions_and_interpolation(self):
        for suffix, factor in [
            ("", lambda rate: math.exp(-rate)),
            ("_Simple", lambda rate: 1 / (1 + rate)),
            ("_Compounded_Semiannual", lambda rate: (1 + rate / 2) ** -2),
        ]:
            observations = [
                (f"ZeroRate_EUR_ESTR_2Y{suffix}", {"mid": 0.06}),
                (f"ZeroRate_EUR_ESTR_0D{suffix}", {"mid": 0.02}),
            ]
            with self.subTest(suffix=suffix), context(observations) as ctx:
                curve = ctx.curve("ESTR")
                self.assertAlmostEqual(curve.discount_factor("2027-02-24"), factor(0.04))
                self.assertAlmostEqual(curve.discount_factor("2026-02-24"), 1.0)

    def test_scenarios_rebuild_direct_and_composite_curves_without_mutating_quotes(self):
        for family, values, bump in [("DiscountFactor", [0.96, 0.90], -0.01),
                                      ("ZeroRate", [0.04, 0.05], 0.01)]:
            labels = [f"{family}_EUR_ESTR_{tenor}" for tenor in ["1Y", "2Y"]]
            ctx = context([(label, {"mid": value}) for label, value in zip(labels, values)])
            composite = qs.MarketIndex.other("COMPOSITE")
            ctx.curves += [qs.CurveConfiguration(
                market_index=composite,
                construction=qs.CurveConstruction.composite(base_index="ESTR", spread_index="ESTR"),
            )]
            for _ in range(2):
                with self.subTest(family=family), ctx:
                    base = ctx.curve("ESTR").discount_factor("2027-02-24")
                    with qs.Scenario(shocks=[qs.Shock(
                        target=labels[0], value=bump, shock_type=qs.ScenarioType.Absolute,
                    )]):
                        shifted = ctx.curve("ESTR").discount_factor("2027-02-24")
                        self.assertLess(shifted, base)
                        self.assertAlmostEqual(ctx.curve(composite).discount_factor("2027-02-24"), shifted ** 2)
                    self.assertAlmostEqual(ctx.curve("ESTR").discount_factor("2027-02-24"), base)
                    self.assertEqual(ctx.quotes.value(labels[0]), values[0])

    def test_source_quote_risk_matches_repricing(self):
        loan = qs.Loan(
            identifier="DIRECT-QUOTE-RISK", start_date=REFERENCE, maturity_date="2028-02-24", notional=100_000,
            currency="EUR", coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06))],
            repayment=qs.LoanRepaymentStructure.french(), payment_frequency="Monthly",
        )
        policy = qs.FixedIncomeDiscountPolicy({qs.Currency.EUR: qs.MarketIndex.ESTR})
        for family, values in [("DiscountFactor", [0.96, 0.90]), ("ZeroRate", [0.04, 0.05])]:
            labels = [f"{family}_EUR_ESTR_{tenor}" for tenor in ["1Y", "2Y"]]

            def value(shocks=(), risk=False):
                with context([(label, {"mid": rate}) for label, rate in zip(labels, values)],
                             enable_extrapolation=True) as ctx:
                    with qs.Scenario(shocks=list(shocks)):
                        return qs.LoanPricer(discount_policy=policy).evaluate(
                            qs.Trade(loan, REFERENCE), ctx,
                            [qs.Request.Value, qs.Request.Sensitivities] if risk else [qs.Request.Value],
                        )

            risk = value(risk=True).sensitivities.set_index("pillar")["value"]
            self.assertEqual(set(risk.index), set(labels))
            for label in labels:
                with self.subTest(family=family, label=label):
                    epsilon = 1e-6
                    def shifted(sign):
                        return value([qs.Shock(target=label, value=sign * epsilon,
                                              shock_type=qs.ScenarioType.Absolute)]).price
                    numerical = (shifted(1) - shifted(-1)) / (2 * epsilon)
                    self.assertAlmostEqual(risk[label], numerical, delta=max(0.01, abs(numerical) * 1e-6))

    def test_quote_files_and_market_snapshots_rebuild_each_side(self):
        label = "ZeroRate_EUR_ESTR_1Y_Compounded_Quarterly"
        ctx = context([(label, {"mid": 0.04, "ask": 0.05})])
        snapshot = qs.MarketSnapshot.from_context("2026-02-24T12:00:00Z", ctx)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "markets.json"
            store = qs.MarketSnapshotStore.from_dict([snapshot.to_dict()])
            store.to_json(str(path))
            restored = qs.MarketSnapshotStore.from_json(str(path))
        for level, rate in [(qs.Level.Mid, 0.04), (qs.Level.Ask, 0.05)]:
            with restored.pricing_context(qs.SnapshotSelector(snapshot.timestamp, level=level)) as historical:
                self.assertAlmostEqual(historical.curve("ESTR").discount_factor("2027-02-24"),
                                       (1 + rate / 4) ** -4)

    def test_calibrated_dependents_retain_direct_parent_quote_risk(self):
        trade = qs.Trade(qs.Loan(
            identifier="DIRECT-PARENT-RISK", start_date=REFERENCE, maturity_date="2028-02-24",
            notional=100_000, currency="EUR",
            coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06))],
        ), REFERENCE)
        for family, values in [("DiscountFactor", [0.96, 0.85]), ("ZeroRate", [0.04, 0.055])]:
            labels = [f"{family}_EUR_ESTR_{tenor}" for tenor in ["1Y", "3Y"]]
            swap_labels = [f"Swap_EUR_EURIBOR3m_{tenor}" for tenor in ["1Y", "2Y"]]

            def value(bump=0, risk=False):
                quotes = qs.QuoteStore.from_dict({
                    "reference_date": REFERENCE,
                    "quotes": [{"identifier": label, "mid": rate + bump}
                               for label, rate in zip(labels, values)]
                    + [{"identifier": label, "mid": rate}
                       for label, rate in zip(swap_labels, [0.045, 0.05])],
                })
                with qs.PricingContext(
                    quotes, [
                        qs.CurveConfiguration(market_index="EURIBOR3m", quotes=swap_labels),
                        qs.CurveConfiguration(market_index="ESTR", quotes=labels, interpolator="Linear"),
                    ],
                    csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
                ) as ctx:
                    return qs.LoanPricer(discount_policy=qs.FixedIncomeDiscountPolicy({
                        qs.Currency.EUR: qs.MarketIndex.EURIBOR3m,
                    })).evaluate(trade, ctx, [qs.Request.Value, qs.Request.Sensitivities]
                                 if risk else [qs.Request.Value])

            with self.subTest(family=family):
                sensitivities = value(risk=True).sensitivities.set_index("pillar")["value"]
                self.assertTrue(set(labels).issubset(sensitivities.index))
                epsilon = 1e-6
                numerical = (value(epsilon).price - value(-epsilon).price) / (2 * epsilon)
                self.assertGreater(abs(numerical), 0.01)
                self.assertAlmostEqual(sensitivities.loc[labels].sum(), numerical,
                                       delta=max(0.02, abs(numerical) * 1e-5))

    def test_invalid_quotes_fail_with_context(self):
        cases = [
            ([("DiscountFactor_EUR_ESTR_1Y", {"mid": 0.0})], "positive"),
            ([("DiscountFactor_EUR_ESTR_0D", {"mid": 0.99})], "must equal 1"),
            ([("DiscountFactor_EUR_ESTR_0D", {"mid": 1.0})], "future pillar"),
            ([("ZeroRate_EUR_ESTR_2026-02-23", {"mid": 0.04})], "reference date"),
            ([("ZeroRate_EUR_SOFR_1Y", {"mid": 0.04})], "curve index"),
            ([("ZeroRate_EUR_ESTR_1Y", {"mid": 0.04}),
              ("DiscountFactor_EUR_ESTR_2Y", {"mid": 0.90})], "one direct quote family"),
            ([("FixedRateDeposit_EUR_ESTR_1Y", {"mid": 0.04}),
              ("ZeroRate_EUR_ESTR_2Y", {"mid": 0.05})], "one direct quote family"),
            ([("ZeroRate_EUR_ESTR_1Y", {"mid": 0.04}),
              ("ZeroRate_EUR_ESTR_2Y_Simple", {"mid": 0.05})], "rate conventions"),
            ([("ZeroRate_EUR_ESTR_1Y", {"mid": 0.04}),
              ("ZeroRate_EUR_ESTR_2027-02-24", {"mid": 0.05})], "distinct dates"),
        ]
        for observations, error in cases:
            with self.subTest(error=error), context(observations) as ctx:
                with self.assertRaisesRegex(qs.QuantSupportError, error):
                    ctx.curve("ESTR")
        with context([("ZeroRate_EUR_ESTR_1Y", {"mid": 0.04})], quote_level=qs.Level.Ask) as ctx:
            with self.assertRaisesRegex(qs.QuantSupportError, "Ask.*ZeroRate_EUR_ESTR_1Y"):
                ctx.curve("ESTR")


if __name__ == "__main__":
    unittest.main()
