"""Cost-of-funds discounting through the native loan pricer."""
from contextlib import contextmanager
import unittest

import quantsupport as qs

REFERENCE = "2026-02-24"
TENORS = ["1D", "1Y", "2Y", "3Y"]
FUNDING = qs.MarketIndex.other("COSTOFFUNDS")
SPREAD = qs.MarketIndex.other("FUNDINGSPREAD")


@contextmanager
def funding_context(scenarios=None, funding_shift=0):
    inputs = [("ESTR", [0.03] * 4), ("EURIBOR12m", [0.04] * 4)]
    quotes = qs.QuoteStore.from_dict({"reference_date": REFERENCE, "quotes": [
        {"identifier": f"FixedRateDeposit_EUR_{index}_{tenor}", "mid": rate}
        for index, rates in inputs for tenor, rate in zip(TENORS, rates)
    ]})
    curves = [qs.CurveConfiguration.from_dict({
        "market_index": index,
        "quotes": [f"FixedRateDeposit_EUR_{index}_{tenor}" for tenor in TENORS],
    }) for index, _ in inputs]
    fixings = qs.FixingStore()
    fixings.add("EURIBOR12m", "2026-02-20", 0.04)
    with qs.PricingContext(
        quotes, curves, fixings=fixings, scenarios=scenarios,
        csa_policy=qs.SingleCurveCSADiscountPolicy(qs.Currency.EUR, qs.MarketIndex.ESTR),
        fixed_income_policy=qs.FixedIncomeDiscountPolicy(
            {qs.Currency.EUR: FUNDING}, prefer_instrument_index=False,
        ),
    ) as context:
        context.add_zero_curve(SPREAD, qs.TenorBasedZeroRateTermStructure(
            REFERENCE, ["0D", "1Y", "2Y", "3Y"],
            [rate + funding_shift for rate in [0.005, 0.0075, 0.01, 0.0125]],
            day_counter=qs.DayCounter.Actual360, compounding=qs.Compounding.Continuous,
        ))
        yield context


def loan(floating=False):
    return qs.Loan(
        identifier="FUNDING", start_date=REFERENCE, maturity_date="2028-02-24",
        notional=100_000, currency=qs.Currency.EUR, discount_index=qs.MarketIndex.ESTR,
        coupons=[qs.Coupon.floating("EURIBOR12m", 0.01) if floating else qs.Coupon.fixed(0.06)],
        amortization_rate=0.05 if floating else None,
        repayment=qs.LoanRepaymentStructure.french(), payment_frequency=qs.Frequency.Monthly,
        day_counter=qs.DayCounter.Actual360, calendar="TARGET", fixing_lag=2,
        business_day_convention=qs.BusinessDayConvention.Following,
    )


def value(instrument, scenarios=None, requests=None, funding_shift=0):
    with funding_context(scenarios, funding_shift) as context:
        context.add_composite_curve(FUNDING, "EURIBOR12m", SPREAD)
        return qs.LoanPricer().evaluate(
            qs.Trade(instrument, REFERENCE), context, requests or [qs.Request.Value],
        )


class CompositeLoanFundingTests(unittest.TestCase):
    def test_composite_factors_and_forward_use_both_components(self):
        with funding_context() as context:
            context.add_composite_curve(FUNDING, "EURIBOR12m", SPREAD)
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
            context.add_composite_curve(FUNDING, "EURIBOR12m", SPREAD)
            result = qs.LoanPricer().evaluate(qs.Trade(instrument, REFERENCE), context,
                                             [qs.Request.Value, qs.Request.Cashflows, qs.Request.FairRate])
            expected = sum(row.amount * context.curve(FUNDING).discount_factor(row.payment_date)
                           for row in result.cashflows.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=8)
            self.assertGreater(result.fair_rate, 0.04)
            estr = qs.LoanPricer(discount_policy=qs.FixedIncomeDiscountPolicy(
                {qs.Currency.EUR: qs.MarketIndex.ESTR}, prefer_instrument_index=False,
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
                    return value(instrument, [qs.Scenario(selector, sign * epsilon,
                                                        qs.ScenarioType.Absolute)]).price
                numerical = (shifted(1) - shifted(-1)) / (2 * epsilon)
                risk = base.sensitivities
                analytic = risk.loc[risk.pillar.str.contains(selector), "value"].sum()
                self.assertAlmostEqual(analytic, numerical, delta=max(0.02, abs(numerical) * 1e-6))

    def test_registration_rejects_missing_and_existing_curves(self):
        with funding_context() as context:
            with self.assertRaisesRegex(qs.QuantSupportError, "MISSING"):
                context.add_composite_curve(FUNDING, "EURIBOR12m", qs.MarketIndex.other("MISSING"))
            with self.assertRaisesRegex(qs.QuantSupportError, "already exists"):
                context.add_composite_curve("ESTR", "EURIBOR12m", SPREAD)
            context.add_composite_curve(FUNDING, "EURIBOR12m", SPREAD)
            with self.assertRaisesRegex(qs.QuantSupportError, "already exists"):
                context.add_composite_curve(FUNDING, "EURIBOR12m", SPREAD)

    def test_zero_curve_registration_checks_context_and_conventions(self):
        empty = qs.PricingContext(qs.QuoteStore.from_dict({
            "reference_date": REFERENCE, "quotes": [],
        }), [])
        curve = qs.ZeroRateTermStructure(
            REFERENCE, [REFERENCE, "2029-02-24"], [0.01, 0.01],
            day_counter=qs.DayCounter.Actual365,
        )
        with self.assertRaisesRegex(qs.QuantSupportError, "not initialized"):
            empty.add_zero_curve(SPREAD, curve)
        with self.assertRaisesRegex(qs.QuantSupportError, "not initialized"):
            empty.add_composite_curve(FUNDING, "EURIBOR12m", SPREAD)
        with funding_context() as context:
            with self.assertRaises(TypeError):
                context.add_zero_curve(FUNDING, object())
            with self.assertRaisesRegex(qs.QuantSupportError, "already exists"):
                context.add_zero_curve(SPREAD, curve)
            wrong_date = qs.ZeroRateTermStructure(
                "2026-02-25", ["2026-02-25", "2029-02-25"], [0.01, 0.01],
            )
            with self.assertRaisesRegex(qs.QuantSupportError, "reference date"):
                context.add_zero_curve(FUNDING, wrong_date)
            context.add_zero_curve(FUNDING, curve)
            with self.assertRaisesRegex(qs.QuantSupportError, "day counters"):
                context.add_composite_curve(qs.MarketIndex.other("MISMATCH"),
                                            "EURIBOR12m", FUNDING)


if __name__ == "__main__":
    unittest.main()
