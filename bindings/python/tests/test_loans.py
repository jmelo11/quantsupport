"""Loan contracts stay fixed; pricer requests resolve expected receipts."""
import math
import json
from pathlib import Path
import tempfile
import unittest

import quantsupport as qs

from market_fixtures import append_quotes, zero_rate_quotes

REFERENCE = "2026-02-24"


def build_loan_contract(**overrides):
    """Build the default EUR loan contract with explicit term overrides."""
    terms = dict(identifier="LOAN", start_date=REFERENCE, maturity_date="2028-02-24",
                 notional=100_000.0, coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06))],
                 currency=qs.Currency.EUR,
                 day_counter=qs.DayCounter.Actual360, calendar="TARGET", fixing_lag=2,
                 business_day_convention=qs.BusinessDayConvention.Following)
    terms.update(overrides)
    return qs.Loan(**terms)


def build_floating_rate_loan(**overrides):
    """Build a EURIBOR12m loan with a fixed initial amortization rate."""
    return build_loan_contract(coupons=[qs.FloatingRateCoupon(index=qs.MarketIndex.EURIBOR12m, spread=0.01)],
                amortization_rate=0.05, **overrides)


def build_pricing_context(instrument=None, forward=0.04, reference=REFERENCE, include_fixings=True):
    """Supply EUR curves and the contract's historical coupon fixings."""
    tenors = ["1D", "1Y", "2Y", "3Y"]
    quotes = qs.QuoteStore.from_dict({"reference_date": reference, "quotes": [
        {"identifier": f"FixedRateDeposit_EUR_{index}_{tenor}", "mid": rate}
        for index, rate in [("ESTR", 0.03), ("EURIBOR12m", forward), ("EURIBOR3m", forward)] for tenor in tenors
    ]})
    curves = [qs.CurveConfiguration.from_dict({"market_index": index,
        "quotes": [f"FixedRateDeposit_EUR_{index}_{tenor}" for tenor in tenors]})
        for index in ["ESTR", "EURIBOR12m", "EURIBOR3m"]]
    fixings = qs.FixingStore()
    if instrument is not None and include_fixings:
        for row in instrument.coupon_fixing_requests().itertuples(index=False):
            if row.fixing_date < reference:
                fixings.add(row.market_index, row.fixing_date, 0.04)
    return qs.PricingContext(quotes, curves, fixings=fixings,
        csa_policy=qs.SingleCurveCSADiscountPolicy(qs.Currency.EUR, qs.MarketIndex.ESTR))


def evaluate_loan_trade(instrument, ctx, requests=None, **pricer_options):
    """Configure the core Rust loan pricer and use native context dispatch."""
    ctx.pricer_configurations = [qs.PricerConfiguration.loan(**pricer_options)]
    return ctx.evaluate(qs.Trade(instrument, REFERENCE), requests)


class LoanWorkflowTests(unittest.TestCase):
    def test_native_loan_configuration_applies_to_portfolios_and_scenarios(self):
        instrument = build_loan_contract(
            prepayment=qs.PrepaymentModelConfiguration.refinancing_incentive(0.02, 4.0, 0.4),
        )
        terms = instrument.to_dict()
        trade = qs.Trade(instrument, REFERENCE)
        opposite_half = qs.Trade(instrument, REFERENCE, qs.Side.PayShort, notional=50_000)
        context = build_pricing_context()
        context.pricer_configurations = [qs.PricerConfiguration.loan(
            refinancing_benchmark_index=qs.MarketIndex.EURIBOR12m,
        )]
        context.fixed_income_policy = qs.FixedIncomeDiscountPolicy({"EUR": "EURIBOR12m"})
        self.assertIsNone(context.pricer_for(trade))
        requests = [qs.Request.Value, qs.Request.Cashflows, qs.Request.Sensitivities]
        with context:
            result = context.evaluate(trade, requests)
            self.assertGreater(result.cashflows.loc[result.cashflows.leg_index == 1, "amount"].sum(), 0)
            curve = context.curve("EURIBOR12m")
            expected = sum(row.amount * curve.discount_factor(row.payment_date)
                           for row in result.cashflows.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=7)
            portfolio = context.evaluate(qs.Portfolio("LOANS", [trade, opposite_half]), requests)
            self.assertAlmostEqual(portfolio.price, result.price / 2, places=7)
            for pillar, value in portfolio.raw_sensitivities:
                self.assertAlmostEqual(value, dict(result.raw_sensitivities)[pillar] / 2, places=6)
            pillar = "FixedRateDeposit_EUR_EURIBOR12m_1Y"
            bumped = {}
            for bump in [-1e-5, 1e-5]:
                with qs.Scenario(shocks=[qs.Shock(pillar, bump, qs.ScenarioType.Absolute)]):
                    bumped[bump] = context.evaluate(trade).price
            derivative = (bumped[1e-5] - bumped[-1e-5]) / 2e-5
            self.assertAlmostEqual(derivative, dict(result.raw_sensitivities)[pillar], delta=0.01)
            self.assertAlmostEqual(context.evaluate(trade).price, result.price, places=7)
            deposit = qs.FixedRateDeposit("DEPOSIT", REFERENCE, "2027-02-24", 100_000, 0.04, "EUR", "ESTR")
            self.assertIsNone(context.pricer_for(qs.Trade(deposit, REFERENCE)))
            self.assertTrue(math.isfinite(context.evaluate(qs.Trade(deposit, REFERENCE)).price))
            context.fixed_income_policy = None
            self.assertGreater(context.evaluate(trade).price, result.price)
            context.pricer_configurations = []
            with self.assertRaises(qs.QuantSupportError):
                context.evaluate(trade)
        self.assertEqual(instrument.to_dict(), terms)

    def test_native_context_converts_receipts_and_risk_to_base_currency(self):
        instrument = build_loan_contract()
        trade = qs.Trade(instrument, REFERENCE)
        context = build_pricing_context()
        with context:
            local = context.evaluate(trade, ["Value", "Sensitivities", "Cashflows"])
        context.quotes = append_quotes(context.quotes, zero_rate_quotes("SOFR", ["0D", "5Y"], [0.03, 0.03], "USD"))
        context.curves = [*context.curves, qs.CurveConfiguration(
            market_index="SOFR",
            quotes=["ZeroRate_USD_SOFR_0D", "ZeroRate_USD_SOFR_5Y"],
        )]
        context.csa_policy = qs.SingleCurveCSADiscountPolicy("USD", "SOFR")
        context.fixed_income_policy = qs.FixedIncomeDiscountPolicy({"EUR": "ESTR"})
        fx = qs.FxStore()
        fx.add("EUR", "USD", 1.1)
        context.fx = fx
        with context:
            converted = context.evaluate(trade, ["Value", "Sensitivities", "Cashflows"])
            risk_only = context.evaluate(trade, ["Sensitivities"])
        self.assertAlmostEqual(converted.price, local.price * 1.1, places=7)
        self.assertIsNone(risk_only.price)
        self.assertEqual(dict(risk_only.raw_sensitivities), dict(converted.raw_sensitivities))
        self.assertEqual(converted.cashflows.amount.tolist(), local.cashflows.amount.tolist())
        self.assertEqual(set(converted.cashflows.currency), {"EUR"})
        self.assertAlmostEqual(dict(converted.raw_sensitivities)["EUR/USD"], local.price, places=7)
        for pillar, value in local.raw_sensitivities:
            self.assertAlmostEqual(dict(converted.raw_sensitivities)[pillar], value * 1.1, places=6)

    def test_discount_curve_is_a_pricing_choice_and_rejected_in_contract_terms(self):
        instrument = build_loan_contract()
        self.assertNotIn("discount_index", instrument.to_dict())
        self.assertFalse(hasattr(instrument, "discount_index"))
        with self.assertRaises(TypeError):
            build_loan_contract(discount_index=qs.MarketIndex.ESTR)
        with self.assertRaises(TypeError):
            qs.Loan.from_dict(dict(instrument.to_dict(), discount_index="ESTR"))
        contract = instrument.to_dict()
        with build_pricing_context() as context:
            context.fixed_income_policy = qs.FixedIncomeDiscountPolicy(
                discount_curve_by_currency={qs.Currency.EUR: qs.MarketIndex.ESTR},
            )
            estr = evaluate_loan_trade(instrument, context, ["Value", "Cashflows"])
            context.fixed_income_policy = qs.FixedIncomeDiscountPolicy(
                discount_curve_by_currency={qs.Currency.EUR: qs.MarketIndex.EURIBOR12m},
            )
            euribor = evaluate_loan_trade(instrument, context, ["Value", "Cashflows"])
        self.assertLess(euribor.price, estr.price)
        self.assertEqual(estr.cashflows.amount.tolist(), euribor.cashflows.amount.tolist())
        self.assertEqual(instrument.to_dict(), contract)

    def test_fixed_and_multiple_floating_index_segments_price_without_contract_mutation(self):
        instrument = build_loan_contract(coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.03))] * 6 +
            [qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01)] * 6 +
            [qs.FloatingRateCoupon(index="EURIBOR3m", spread=0.005)])
        snapshot = instrument.to_dict()
        self.assertEqual(set(instrument.coupon_fixing_requests().market_index), {"EURIBOR12m", "EURIBOR3m"})
        with build_pricing_context(instrument) as ctx:
            result = evaluate_loan_trade(instrument, ctx, ["Value", "Cashflows", "Sensitivities"])
            coupons = result.cashflows[result.cashflows.type.isin(["FixedRateCoupon", "FloatingRateCoupon"])]
            self.assertEqual(len(coupons), 24)
            self.assertEqual(sum(coupons.type == "FixedRateCoupon"), 6)
            expected = sum(row.amount * ctx.curve("ESTR").discount_factor(row.payment_date)
                           for row in result.cashflows.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=7)
        self.assertEqual(instrument.to_dict(), snapshot)

    def test_coupon_list_repeats_last_template_and_preserves_step_rates(self):
        instrument = build_loan_contract(coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.03)), {"Fixed": {"rate": 0.05}}],
                                        repayment=qs.LoanRepaymentStructure.bullet())
        with build_pricing_context() as ctx:
            result = evaluate_loan_trade(instrument, ctx, ["Value", "Cashflows", "Sensitivities"])
            coupons = result.cashflows.loc[result.cashflows.type == "FixedRateCoupon"]
            rates = coupons.amount / (100_000 * coupons.accrual_period)
            self.assertEqual(len(rates), 24)
            self.assertAlmostEqual(rates.iloc[0], 0.03)
            self.assertTrue(all(math.isclose(rate, 0.05) for rate in rates.iloc[1:]))
            for target in [0, 1]:
                fair = evaluate_loan_trade(instrument, ctx, [qs.Request.FairRate(target)])
                self.assertIsInstance(fair.fair_rate, float)
                self.assertFalse(hasattr(fair, "fair_rates"))
            with self.assertRaises(qs.QuantSupportError):
                evaluate_loan_trade(instrument, ctx, [qs.Request.FairRate(2)])

    def test_coupon_schedule_json_and_subclass_roundtrip(self):
        class CustomLoan(qs.Loan):
            pass
        instrument = build_loan_contract(coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.04))] * 12 +
                                        [qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01)])
        terms = instrument.to_dict()
        self.assertNotIn("rate", terms)
        self.assertEqual(len(terms["coupons"]), 13)
        self.assertIsInstance(CustomLoan.from_dict(terms), CustomLoan)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "loan.json"
            path.write_text(json.dumps(terms))
            restored = CustomLoan.from_json(str(path))
            self.assertIsInstance(restored, CustomLoan)
            self.assertEqual(restored.to_dict(), terms)

    def test_invalid_coupon_lists_and_legacy_rate_keyword_are_rejected(self):
        for coupons in [[], [qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.04))] * 25, [{"Redemption": {"amount": 100}}]]:
            with self.subTest(coupons=coupons), self.assertRaises((qs.QuantSupportError, ValueError)):
                build_loan_contract(coupons=coupons)
        with self.assertRaises(TypeError):
            build_loan_contract(rate=0.04)

    def test_bounded_coupon_projection_uses_contractual_day_counter_and_trade_side(self):
        instrument = build_loan_contract(
            coupons=[qs.OptionEmbeddedCoupon(index="EURIBOR12m", payoff=(qs.PayoffOps.Index + 0.01).min(0.07).max(0.06))],
            repayment=qs.LoanRepaymentStructure.bullet(), day_counter="Thirty360")
        with build_pricing_context(instrument) as ctx:
            result = evaluate_loan_trade(instrument, ctx, ["Value", "Cashflows", "Sensitivities"])
            coupons = result.cashflows.loc[result.cashflows.type == "OptionEmbeddedCoupon"]
            self.assertEqual(len(coupons), 24)
            self.assertTrue(all(math.isclose(fraction, 1/12) for fraction in coupons.accrual_period))
            self.assertTrue(all(math.isclose(amount, 500.0) for amount in coupons.amount))
            expected = sum(row.amount * ctx.curve("ESTR").discount_factor(row.payment_date)
                           for row in result.cashflows.itertuples(index=False))
            self.assertAlmostEqual(result.price, expected, places=7)
            short = ctx.evaluate(qs.Trade(instrument, REFERENCE, qs.Side.PayShort))
            self.assertAlmostEqual(short.price, -result.price, places=7)
            with self.assertRaisesRegex(qs.QuantSupportError, "bounded"):
                evaluate_loan_trade(instrument, ctx, ["FairRate"])

    def test_typed_terms_roundtrip_and_serialized_inputs(self):
        coupon_schedules = [
            [qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.06))],
            [qs.FloatingRateCoupon(index=qs.MarketIndex.EURIBOR12m, spread=0.01)],
            [qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.04))] * 12 + [qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01)],
            [qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.03)), qs.FixedRateCoupon(rate=qs.InterestRate(rate=0.05))],
            [qs.OptionEmbeddedCoupon(index="EURIBOR12m", payoff=(qs.PayoffOps.Index + 0.01).min(0.08).max(0.0))],
        ]
        repayments = [qs.LoanRepaymentStructure.french(), qs.LoanRepaymentStructure.bullet(), qs.LoanRepaymentStructure.balloon(0.25)]
        prepayments = [qs.PrepaymentModelConfiguration.no_prepayment(), qs.PrepaymentModelConfiguration.constant_annual_cpr(0.12),
                      qs.PrepaymentModelConfiguration.dated_annual_cpr([(REFERENCE, 0.12)]),
                      qs.PrepaymentModelConfiguration.refinancing_incentive(0.02, 4.0, 0.4)]
        for coupons in coupon_schedules:
            for repayment in repayments:
                for prepayment in prepayments:
                    with self.subTest(coupons=repr(coupons), repayment=repr(repayment), prepayment=repr(prepayment)):
                        instrument = build_loan_contract(coupons=coupons, repayment=repayment, prepayment=prepayment, amortization_rate=0.05)
                        terms = instrument.to_dict()
                        self.assertEqual(qs.Loan.from_dict(terms).to_dict(), terms)
                        self.assertEqual(build_loan_contract(coupons=[coupon.to_dict() for coupon in coupons], repayment=repayment.to_dict(),
                            prepayment=prepayment.to_dict(), amortization_rate=0.05).to_dict(), terms)

    def test_invalid_contract_and_floating_amortization_inputs(self):
        with self.assertRaises(qs.QuantSupportError):
            build_loan_contract(notional=-1)
        with self.assertRaises(TypeError):
            build_loan_contract(notinoal=50_000)
        with self.assertRaisesRegex(qs.QuantSupportError, "amortization_rate"):
            build_loan_contract(coupons=[qs.FloatingRateCoupon(index="EURIBOR12m", spread=0.01)])
        for model in [qs.PrepaymentModelConfiguration.constant_annual_cpr(1.1),
                      qs.PrepaymentModelConfiguration.refinancing_incentive(0.4, 2.0, 0.2),
                      qs.PrepaymentModelConfiguration.dated_annual_cpr([(REFERENCE, 0.1), (REFERENCE, 0.2)])]:
            with self.assertRaises(qs.QuantSupportError):
                build_loan_contract(prepayment=model)

    def test_cashflow_request_pv_sign_notional_and_request_order(self):
        instrument = build_floating_rate_loan()
        with build_pricing_context(instrument) as ctx:
            result = evaluate_loan_trade(instrument, ctx, [qs.Request.Cashflows, qs.Request.Cashflows,
                qs.Request.FairRate(0), qs.Request.Sensitivities, qs.Request.Value])
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
        instrument = build_floating_rate_loan()
        snapshot = instrument.to_dict()
        principal = []
        for forward in [0.02, 0.06]:
            with build_pricing_context(instrument, forward=forward) as ctx:
                table = evaluate_loan_trade(instrument, ctx, ["Cashflows"]).cashflows
                principal.append(table.loc[table.type == "Redemption", "amount"].tolist())
                coupons = table.loc[table.type == "FloatingRateCoupon"]
                self.assertEqual(coupons.fixing.iloc[:12].tolist(), [0.04] * 12)
        self.assertEqual(principal[0], principal[1])
        self.assertEqual(instrument.to_dict(), snapshot)
        requests = instrument.coupon_fixing_requests()
        self.assertEqual(len(requests), 2)
        self.assertEqual(requests.start_date.tolist(), [REFERENCE, "2027-02-24"])

    def test_missing_fixings_and_uninitialized_context_report_errors(self):
        instrument = build_floating_rate_loan()
        with self.assertRaisesRegex(qs.QuantSupportError, "not initialized"):
            evaluate_loan_trade(instrument, build_pricing_context(instrument))
        with build_pricing_context(instrument, include_fixings=False) as ctx:
            with self.assertRaises(qs.QuantSupportError):
                evaluate_loan_trade(instrument, ctx, ["Cashflows"])

    def test_prepayment_is_priced_separately_and_principal_reconciles(self):
        instrument = build_loan_contract(prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.25))
        snapshot = instrument.to_dict()
        with build_pricing_context() as ctx:
            result = evaluate_loan_trade(instrument, ctx, ["Value", "Cashflows"])
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
            instrument = build_floating_rate_loan() if is_floating else build_loan_contract()
            initial_rate = 0.05 if is_floating else 0.06
            with build_pricing_context(instrument) as ctx:
                for rebuild in [False, True]:
                    fair = evaluate_loan_trade(instrument, ctx, ["FairRate"], rebuild_amortization_for_fair_rate=rebuild).fair_rate
                    rate = qs.FloatingRateCoupon(index="EURIBOR12m", spread=fair) if is_floating else qs.FixedRateCoupon(rate=qs.InterestRate(rate=fair))
                    amortization_rate = (0.04 + fair if is_floating else fair) if rebuild else initial_rate
                    candidate = build_loan_contract(coupons=[rate], amortization_rate=amortization_rate)
                    par = evaluate_loan_trade(candidate, ctx).price
                    self.assertAlmostEqual(par, 100_000, places=5)

    def test_fair_rate_request_keeps_its_target_and_rejects_ambiguous_results(self):
        self.assertEqual(qs.Request.FairRate(1), qs.Request.parse("FairRate(1)"))
        self.assertNotEqual(qs.Request.FairRate(0), qs.Request.FairRate(1))
        self.assertEqual(qs.Request.FairRate(1).index, 1)
        self.assertIsNone(qs.Request.Value.index)
        self.assertEqual(repr(qs.Request.FairRate(1)), "Request.FairRate(1)")
        for text in ["FairRate(-1)", "FairRate(1.5)"]:
            with self.assertRaises(qs.QuantSupportError): qs.Request.parse(text)
        with build_pricing_context() as ctx:
            with self.assertRaises(qs.QuantSupportError):
                evaluate_loan_trade(build_loan_contract(), ctx, [qs.Request.FairRate(0), qs.Request.FairRate(1)])

    def test_mixed_and_stepped_fair_rates_bootstrap_each_prefix(self):
        def coupons(rates):
            return ([qs.FixedRateCoupon(rate=qs.InterestRate(rate=rates[0]))] * 6 +
                    [qs.FixedRateCoupon(rate=qs.InterestRate(rate=rates[1]))] * 6 +
                    [qs.FloatingRateCoupon(index="EURIBOR12m", spread=rates[2])] * 6 +
                    [qs.FloatingRateCoupon(index="EURIBOR12m", spread=rates[3])])
        instrument = build_loan_contract(coupons=coupons([0.06, 0.04, 0.01, 0.02]), amortization_rate=0.05)
        snapshot = instrument.to_dict()
        with build_pricing_context(instrument) as ctx:
            for rebuild in [False, True]:
                with self.subTest(rebuild=rebuild):
                    rates = [evaluate_loan_trade(instrument, ctx, [qs.Request.FairRate(target)],
                             rebuild_amortization_for_fair_rate=rebuild).fair_rate for target in range(4)]
                    candidate = build_loan_contract(coupons=coupons(rates),
                        amortization_rate=rates[0] if rebuild else 0.05)
                    par = evaluate_loan_trade(candidate, ctx, ["Value", "Cashflows"])
                    self.assertAlmostEqual(par.price, 100_000, places=5)
                    for end in ["2026-08-24", "2027-02-24", "2027-08-24", "2028-02-24"]:
                        # Apply the same solved schedule to each shortened loan,
                        # redeeming its outstanding capital at the boundary.
                        table = par.cashflows.loc[par.cashflows.payment_date <= end]
                        repaid = table.loc[table.type == "Redemption", "amount"].sum()
                        prefix_pv = sum(row.amount * ctx.curve("ESTR").discount_factor(row.payment_date)
                                        for row in table.itertuples(index=False))
                        prefix_pv += (100_000 - repaid) * ctx.curve("ESTR").discount_factor(end)
                        self.assertAlmostEqual(prefix_pv, 100_000, places=5)
        self.assertEqual(instrument.to_dict(), snapshot)

    def test_refinancing_model_uses_configured_market_and_cashflow_requests(self):
        instrument = build_loan_contract(prepayment=qs.PrepaymentModelConfiguration.refinancing_incentive(0.02, 4.0, 0.4))
        with build_pricing_context() as ctx:
            with self.assertRaises(qs.QuantSupportError):
                evaluate_loan_trade(instrument, ctx)
            result = evaluate_loan_trade(instrument, ctx, ["Value", "Cashflows", "Sensitivities"], refinancing_benchmark_index="EURIBOR12m")
            self.assertGreater(result.cashflows.loc[result.cashflows.leg_index == 1, "amount"].sum(), 0)
            self.assertTrue(math.isfinite(result.price))
            self.assertGreater(len(result.sensitivities), 0)


if __name__ == "__main__":
    unittest.main()
