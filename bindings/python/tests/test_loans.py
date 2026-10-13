"""Loan contracts stay fixed; pricer requests resolve expected receipts."""
import datetime
import math
import unittest

import quantsupport as qs

REFERENCE = "2026-02-24"


def loan(**overrides):
    terms = dict(identifier="LOAN", start_date=REFERENCE, maturity_date="2028-02-24",
                 notional=100_000.0, rate=qs.LoanRateDefinition.fixed(0.06),
                 currency=qs.Currency.EUR, discount_index=qs.MarketIndex.ESTR,
                 day_counter=qs.DayCounter.Actual360, calendar="TARGET", fixing_lag=2,
                 business_day_convention=qs.BusinessDayConvention.Following)
    terms.update(overrides)
    return qs.Loan(**terms)


def floating(**overrides):
    return loan(rate=qs.LoanRateDefinition.floating(qs.MarketIndex.EURIBOR12m, 0.01),
                amortization_rate=0.05, **overrides)


def context(instrument=None, forward=0.04, reference=REFERENCE, include_fixings=True):
    tenors = ["1D", "1Y", "2Y", "3Y"]
    quotes = qs.QuoteStore.from_dict({"reference_date": reference, "quotes": [
        {"identifier": f"FixedRateDeposit_EUR_{index}_{tenor}", "mid": rate}
        for index, rate in [("ESTR", 0.03), ("EURIBOR12m", forward)] for tenor in tenors
    ]})
    curves = [qs.CurveConfiguration.from_dict({"market_index": index,
        "quotes": [f"FixedRateDeposit_EUR_{index}_{tenor}" for tenor in tenors]})
        for index in ["ESTR", "EURIBOR12m"]]
    fixings = qs.FixingStore()
    if instrument is not None and include_fixings:
        for row in instrument.fixing_requests().itertuples(index=False):
            if row.fixing_date < reference:
                fixings.add(row.market_index, row.fixing_date, 0.04)
    return qs.PricingContext(quotes, curves, fixings=fixings,
        csa_policy=qs.SingleCurveCSADiscountPolicy(qs.Currency.EUR, qs.MarketIndex.ESTR))


def evaluate(instrument, ctx, requests=None, **pricer_options):
    return qs.LoanPricer(**pricer_options).evaluate(qs.Trade(instrument, REFERENCE), ctx, requests)


class LoanWorkflowTests(unittest.TestCase):
    def test_typed_terms_roundtrip_and_serialized_inputs(self):
        rates = [qs.LoanRateDefinition.fixed(0.06),
                 qs.LoanRateDefinition.floating(qs.MarketIndex.EURIBOR12m, 0.01),
                 qs.LoanRateDefinition.mixed(0.04, datetime.date(2027, 2, 24), qs.MarketIndex.EURIBOR12m, 0.01)]
        repayments = [qs.LoanRepaymentStructure.french(), qs.LoanRepaymentStructure.bullet(), qs.LoanRepaymentStructure.balloon(0.25)]
        prepayments = [qs.PrepaymentModelConfiguration.none(), qs.PrepaymentModelConfiguration.cpr(0.12),
                      qs.PrepaymentModelConfiguration.cpr_curve([(REFERENCE, 0.12)]),
                      qs.PrepaymentModelConfiguration.refinancing(0.02, 4.0, 0.4)]
        for rate in rates:
            for repayment in repayments:
                for prepayment in prepayments:
                    with self.subTest(rate=repr(rate), repayment=repr(repayment), prepayment=repr(prepayment)):
                        instrument = loan(rate=rate, repayment=repayment, prepayment=prepayment, amortization_rate=0.05)
                        terms = instrument.to_dict()
                        self.assertEqual(qs.Loan.from_dict(terms).to_dict(), terms)
                        self.assertEqual(loan(rate=rate.to_dict(), repayment=repayment.to_dict(),
                            prepayment=prepayment.to_dict(), amortization_rate=0.05).to_dict(), terms)

    def test_invalid_contract_and_floating_amortization_inputs(self):
        with self.assertRaises(qs.QuantSupportError):
            loan(notional=-1)
        with self.assertRaises(TypeError):
            loan(notinoal=50_000)
        with self.assertRaisesRegex(qs.QuantSupportError, "amortization_rate"):
            loan(rate=qs.LoanRateDefinition.floating("EURIBOR12m", 0.01))
        for model in [qs.PrepaymentModelConfiguration.cpr(1.1),
                      qs.PrepaymentModelConfiguration.refinancing(0.4, 2.0, 0.2),
                      qs.PrepaymentModelConfiguration.cpr_curve([(REFERENCE, 0.1), (REFERENCE, 0.2)])]:
            with self.assertRaises(qs.QuantSupportError):
                loan(prepayment=model)

    def test_cashflow_request_pv_sign_notional_and_request_order(self):
        instrument = floating()
        with context(instrument) as ctx:
            result = evaluate(instrument, ctx, [qs.Request.Cashflows, qs.Request.Cashflows,
                qs.Request.FairRate, qs.Request.Sensitivities, qs.Request.Value])
            table = result.cashflows
            expected = sum(row.amount * ctx.curve("ESTR").discount_factor(row.payment_date)
                           for row in table.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=7)
            self.assertEqual(set(table.type), {"FloatingRateCoupon", "Redemption"})
            self.assertTrue((table.payment_date > REFERENCE).all())
            self.assertAlmostEqual(ctx.evaluate(qs.Trade(instrument, REFERENCE)).price, result.price, places=7)
            short = ctx.evaluate(qs.Trade(instrument, REFERENCE, qs.Side.PayShort), ["Value", "Cashflows"])
            self.assertAlmostEqual(short.price, -result.price, places=7)
            self.assertTrue((short.cashflows.amount < 0).all())
            half = ctx.evaluate(qs.Trade(instrument, REFERENCE, notional=50_000))
            self.assertAlmostEqual(half.price, result.price / 2, places=7)
            self.assertEqual(instrument.notional, 100_000)

    def test_market_moves_preserve_contractual_principal_and_annual_fixings(self):
        instrument = floating()
        snapshot = instrument.to_dict()
        principal = []
        for forward in [0.02, 0.06]:
            with context(instrument, forward=forward) as ctx:
                table = evaluate(instrument, ctx, ["Cashflows"]).cashflows
                principal.append(table.loc[table.type == "Redemption", "amount"].tolist())
                coupons = table.loc[table.type == "FloatingRateCoupon"]
                self.assertEqual(coupons.fixing.iloc[:12].tolist(), [0.04] * 12)
        self.assertEqual(principal[0], principal[1])
        self.assertEqual(instrument.to_dict(), snapshot)
        requests = instrument.fixing_requests()
        self.assertEqual(len(requests), 2)
        self.assertEqual(requests.start_date.tolist(), [REFERENCE, "2027-02-24"])

    def test_missing_fixings_and_uninitialized_context_report_errors(self):
        instrument = floating()
        with self.assertRaisesRegex(qs.QuantSupportError, "not initialized"):
            evaluate(instrument, context(instrument))
        with context(instrument, include_fixings=False) as ctx:
            with self.assertRaises(qs.QuantSupportError):
                evaluate(instrument, ctx, ["Cashflows"])

    def test_prepayment_is_priced_separately_and_principal_reconciles(self):
        instrument = loan(prepayment=qs.PrepaymentModelConfiguration.cpr(0.25))
        snapshot = instrument.to_dict()
        with context() as ctx:
            result = evaluate(instrument, ctx, ["Value", "Cashflows"])
            table = result.cashflows
            self.assertEqual(set(table.type), {"FixedRateCoupon", "Redemption"})
            self.assertTrue((table.leg_index == 1).any())
            self.assertAlmostEqual(table.loc[table.type == "Redemption", "amount"].sum(), 100_000, places=7)
            expected = sum(row.amount * ctx.curve("ESTR").discount_factor(row.payment_date)
                           for row in table.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=7)
            self.assertEqual(instrument.to_dict(), snapshot)

    def test_fair_rate_can_preserve_or_recompute_amortization(self):
        for is_floating in [False, True]:
            instrument = floating() if is_floating else loan()
            initial_rate = 0.05 if is_floating else 0.06
            with context(instrument) as ctx:
                for rebuild in [False, True]:
                    fair = evaluate(instrument, ctx, ["FairRate"], recompute_schedule=rebuild).fair_rate
                    rate = qs.LoanRateDefinition.floating("EURIBOR12m", fair) if is_floating else qs.LoanRateDefinition.fixed(fair)
                    amortization_rate = (0.04 + fair if is_floating else fair) if rebuild else initial_rate
                    candidate = loan(rate=rate, amortization_rate=amortization_rate)
                    par = evaluate(candidate, ctx).price
                    self.assertAlmostEqual(par, 100_000, places=5)

    def test_refinancing_model_uses_configured_market_and_cashflow_requests(self):
        instrument = loan(prepayment=qs.PrepaymentModelConfiguration.refinancing(0.02, 4.0, 0.4))
        with context() as ctx:
            with self.assertRaises(qs.QuantSupportError):
                evaluate(instrument, ctx)
            result = evaluate(instrument, ctx, ["Value", "Cashflows", "Sensitivities"], refinancing_index="EURIBOR12m")
            self.assertGreater(result.cashflows.loc[result.cashflows.leg_index == 1, "amount"].sum(), 0)
            self.assertTrue(math.isfinite(result.price))
            self.assertGreater(len(result.sensitivities), 0)


if __name__ == "__main__":
    unittest.main()
