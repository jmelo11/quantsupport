"""Cost-of-funds discounting through the native loan pricer."""
import unittest

import quantsupport as qs

REFERENCE = "2026-02-24"
TENORS = ["1D", "1Y", "2Y", "3Y"]
FUNDING = qs.MarketIndex.other("COSTOFFUNDS")
SPREAD = qs.MarketIndex.other("FUNDINGSPREAD")


def funding_context(funding_shift=0):
    inputs = [("ESTR", [0.03] * 4), ("EURIBOR12m", [0.04] * 4)]
    quotes = qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": [
        {"identifier": f"FixedRateDeposit_EUR_{index}_{tenor}", "mid": rate}
        for index, rates in inputs for tenor, rate in zip(TENORS, rates)
    ]})
    curves = [qs.CurveConfiguration.from_dict({
        "market_index": index,
        "quotes": [f"FixedRateDeposit_EUR_{index}_{tenor}" for tenor in TENORS],
    }) for index, _ in inputs]
    curves.extend(qs.CurveConfiguration.from_dict_list([
        {"market_index": {"Other": "COSTOFFUNDS"}, "construction": {
            "type": "Composite", "base_index": "EURIBOR12m",
            "spread_index": {"Other": "FUNDINGSPREAD"},
        }},
        {"market_index": {"Other": "FUNDINGSPREAD"}, "interpolator": "Linear",
         "construction": {
            "type": "ZeroRate", "pillars": ["0D", "1Y", "2Y", "3Y"],
            "rates": [rate + funding_shift for rate in [0.005, 0.0075, 0.01, 0.0125]],
            "compounding": "Continuous", "frequency": "Annual",
        }},
    ]))
    fixings = qs.FixingStore()
    fixings.add("EURIBOR12m", "2026-02-20", 0.04)
    return qs.PricingContext(
        quotes, curves, fixings=fixings,
        csa_policy=qs.SingleCurveCSADiscountPolicy(qs.Currency.EUR, qs.MarketIndex.ESTR),
        fixed_income_policy=qs.FixedIncomeDiscountPolicy(
            {qs.Currency.EUR: FUNDING},
        ),
    )


def loan(floating=False):
    return qs.Loan(
        identifier="FUNDING", start_date=REFERENCE, maturity_date="2028-02-24",
        notional=100_000, currency=qs.Currency.EUR,
        coupons=[qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01) if floating else qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06))],
        amortization_rate=0.05 if floating else None,
        repayment=qs.LoanRepaymentStructure.french(), payment_frequency=qs.Frequency.Monthly,
        day_counter=qs.DayCounter.Actual360, calendar="TARGET", fixing_lag=2,
        business_day_convention=qs.BusinessDayConvention.Following,
    )


def value(instrument, scenarios=None, requests=None, funding_shift=0):
    with funding_context(funding_shift=funding_shift) as context, qs.Scenario(shocks=scenarios or []):
        return qs.LoanPricer().evaluate(
            qs.Trade(instrument, REFERENCE), context, requests or [qs.Request.Value],
        )


class CompositeLoanFundingTests(unittest.TestCase):
    def test_composite_factors_and_forward_use_both_components(self):
        with funding_context() as context:
            curve = context.curve(FUNDING)
            for date in ["2026-03-24", "2027-02-24", "2028-02-24"]:
                expected = (context.curve("EURIBOR12m").discount_factor(date)
                            * context.curve(SPREAD).discount_factor(date))
                self.assertAlmostEqual(curve.discount_factor(date), expected, places=13)
            start, end = "2027-02-24", "2028-02-24"
            tau = qs.DayCounter.Actual360.year_fraction(start, end)
            self.assertAlmostEqual(curve.forward_rate(start, end),
                                   (curve.discount_factor(start) / curve.discount_factor(end) - 1) / tau,
                                   places=12)

    def test_native_discount_policy_prices_receipts_and_par_coupon(self):
        instrument = loan()
        with funding_context() as context:
            result = qs.LoanPricer().evaluate(qs.Trade(instrument, REFERENCE), context,
                                             [qs.Request.Value, qs.Request.Cashflows, qs.Request.FairRate])
            expected = sum(row.amount * context.curve(FUNDING).discount_factor(row.payment_date)
                           for row in result.cashflows.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=8)
            self.assertGreater(result.fair_rate, 0.04)
            estr = qs.LoanPricer(discount_policy=qs.FixedIncomeDiscountPolicy(
                {qs.Currency.EUR: qs.MarketIndex.ESTR},
            )).evaluate(qs.Trade(instrument, REFERENCE), context, [qs.Request.Value])
            self.assertLess(result.price, estr.price)

    def test_scenarios_rebuild_components_and_preserve_contract(self):
        instrument = loan(True)
        contract = instrument.to_dict()
        base = value(instrument, requests=[qs.Request.Value, qs.Request.Cashflows])
        stressed = value(instrument, requests=[qs.Request.Value, qs.Request.Cashflows], funding_shift=0.005)
        self.assertLess(stressed.price, base.price)
        self.assertEqual(stressed.cashflows.amount.tolist(), base.cashflows.amount.tolist())
        self.assertEqual(instrument.to_dict(), contract)
        self.assertAlmostEqual(value(instrument).price, base.price, places=9)

    def test_floating_quote_risk_combines_projection_and_composite_discounting(self):
        instrument = loan(True)
        base = value(instrument, requests=[qs.Request.Value, qs.Request.Sensitivities])
        epsilon = 1e-6
        for selector in ["EURIBOR12m", "FUNDINGSPREAD"]:
            with self.subTest(selector=selector):
                def shifted(sign):
                    if selector == "FUNDINGSPREAD":
                        return value(instrument, funding_shift=sign * epsilon).price
                    return value(instrument, [{"target": selector, "value": sign * epsilon, "type": qs.ScenarioType.Absolute}]).price
                numerical = (shifted(1) - shifted(-1)) / (2 * epsilon)
                risk = base.sensitivities
                analytic = risk.loc[risk.pillar.str.contains(selector), "value"].sum()
                self.assertAlmostEqual(analytic, numerical, delta=max(0.02, abs(numerical) * 1e-6))

    def test_configured_curves_rebuild_on_context_reentry(self):
        context = funding_context()
        for _ in range(2):
            with context:
                self.assertEqual(len(context.discount_curves()), 4)
                factor = context.curve(FUNDING).discount_factor("2028-02-24")
                self.assertGreater(factor, 0)
                self.assertFalse(hasattr(context, "add_composite_curve"))
                self.assertFalse(hasattr(context, "add_zero_curve"))

    def test_calibration_uses_composite_parents_and_retains_source_risk(self):
        instrument = loan()

        def calibrated_value(benchmark_shift=0, spread_shift=0, sensitivities=False):
            specifications = [
                {"market_index": "EURIBOR3m", "quotes": [
                    f"Swap_EUR_EURIBOR3m_{tenor}" for tenor in ["1Y", "2Y", "3Y"]
                ]},
                {"market_index": {"Other": "PARENT"}, "construction": {
                    "type": "Composite", "base_index": "ESTR",
                    "spread_index": {"Other": "SPREAD"},
                }},
                {"market_index": "ESTR", "quotes": [
                    f"FixedRateDeposit_EUR_ESTR_{tenor}" for tenor in TENORS
                ]},
                {"market_index": {"Other": "SPREAD"}, "interpolator": "Linear",
                 "construction": {"type": "ZeroRate", "pillars": ["0D", "1Y", "3Y"],
                    "rates": [rate + spread_shift for rate in [0.005, 0.01, 0.02]],
                    "compounding": "Continuous", "frequency": "Annual"}},
            ]
            quotes = qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": [
                {"identifier": f"FixedRateDeposit_EUR_ESTR_{tenor}", "mid": 0.03 + benchmark_shift}
                for tenor in TENORS
            ] + [
                {"identifier": f"Swap_EUR_EURIBOR3m_{tenor}", "mid": rate}
                for tenor, rate in [("1Y", 0.04), ("2Y", 0.045), ("3Y", 0.047)]
            ]})
            with qs.PricingContext(
                quotes, qs.CurveConfiguration.from_dict_list(specifications),
                csa_policy=qs.SingleCurveCSADiscountPolicy(qs.Currency.EUR, qs.MarketIndex.other("PARENT")),
            ) as context:
                requests = [qs.Request.Value, qs.Request.Sensitivities] if sensitivities else [qs.Request.Value]
                pricer = qs.LoanPricer(discount_policy=qs.FixedIncomeDiscountPolicy(
                    {qs.Currency.EUR: qs.MarketIndex.EURIBOR3m},
                ))
                return pricer.evaluate(qs.Trade(instrument, REFERENCE), context, requests)

        base = calibrated_value(sensitivities=True)
        epsilon = 1e-6
        for label, bump in [("ESTR", "benchmark_shift"), ("SPREAD", "spread_shift")]:
            with self.subTest(parent_source=label):
                numerical = (calibrated_value(**{bump: epsilon}).price
                             - calibrated_value(**{bump: -epsilon}).price) / (2 * epsilon)
                risk = base.sensitivities
                analytic = risk.loc[risk.pillar.str.contains(label), "value"].sum()
                self.assertGreater(abs(numerical), 0.01)
                self.assertAlmostEqual(analytic, numerical, delta=max(0.02, abs(numerical) * 1e-5))

    def test_configuration_serialization_and_dependency_validation(self):
        def empty_context(specifications):
            return qs.PricingContext(qs.QuoteStore.from_dict({
                "reference_date": REFERENCE, "quotes": [],
            }), qs.CurveConfiguration.from_dict_list(specifications))

        zero = {"market_index": {"Other": "SPREAD"}, "interpolator": "Linear",
                "construction": {"type": "ZeroRate", "pillars": [REFERENCE, "2029-02-24"],
                                 "rates": [0.005, 0.01], "compounding": "Continuous",
                                 "frequency": "Annual"}}
        composite = {"market_index": {"Other": "FUNDS"}, "construction": {
            "type": "Composite", "base_index": {"Other": "SPREAD"},
            "spread_index": {"Other": "SPREAD"},
        }}
        parsed = qs.CurveConfiguration.from_dict(composite)
        self.assertEqual(parsed.to_dict()["construction"], composite["construction"])
        with empty_context([composite, zero]) as context:
            self.assertAlmostEqual(
                context.curve(qs.MarketIndex.other("FUNDS")).discount_factor("2028-02-24"),
                context.curve(qs.MarketIndex.other("SPREAD")).discount_factor("2028-02-24") ** 2,
                places=13,
            )
        with self.assertRaisesRegex(qs.QuantSupportError, "requires SPREAD"):
            empty_context([composite]).initialize()
        with self.assertRaisesRegex(qs.QuantSupportError, "Duplicate curve"):
            empty_context([zero, zero]).initialize()
        recursive = {"market_index": {"Other": "SPREAD"}, "construction": {
            "type": "Composite", "base_index": {"Other": "FUNDS"},
            "spread_index": {"Other": "FUNDS"},
        }}
        with self.assertRaisesRegex(qs.QuantSupportError, "Circular dependency"):
            empty_context([composite, recursive]).initialize()
        recursive["construction"]["base_index"] = {"Other": "SPREAD"}
        with self.assertRaisesRegex(qs.QuantSupportError, "references itself"):
            empty_context([recursive]).initialize()
        invalid = dict(zero, quotes=["FixedRateDeposit_EUR_ESTR_1Y"])
        with self.assertRaisesRegex(qs.QuantSupportError, "calibration quotes"):
            empty_context([invalid]).initialize()
        invalid = dict(zero, construction=dict(zero["construction"], rates=[0.005]))
        with self.assertRaisesRegex(qs.QuantSupportError, "matching non-empty"):
            empty_context([invalid]).initialize()


if __name__ == "__main__":
    unittest.main()
