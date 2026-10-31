"""Independent income/balance checks for the teaching forecast layer."""

from datetime import date
import unittest

import pandas as pd
import quantsupport as qs

from nii_forecast import (
    ForecastPortfolio, ForecastPosition, IncomeForecaster, NewProduction,
    ProductionTemplate, RolloverRule,
)


AS_OF = "2026-03-01"


def make_context(shift=0.0, reference=AS_OF):
    indices = [("ESTR", 0.025), ("EURIBOR3m", 0.03)]
    pillars = ["0D", "1Y", "10Y"]
    return qs.PricingContext(
        quotes=qs.QuoteStore.from_dict({"reference_date": reference, "quotes": [
            {"identifier": f"ZeroRate_EUR_{index}_{tenor}", "mid": rate + shift}
            for index, rate in indices for tenor in pillars
        ]}),
        curves=[qs.CurveConfiguration(
            market_index=index, quotes=[f"ZeroRate_EUR_{index}_{tenor}" for tenor in pillars],
            day_counter="Actual360", interpolator="Linear",
        ) for index, _ in indices],
        csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
    )


def deposit(name="DEPOSIT", rate=0.06, side="LongReceive", start=AS_OF,
            maturity="2027-03-01", notional=1000):
    return qs.Trade(qs.FixedRateDeposit(
        identifier=name, start_date=start, maturity_date=maturity, notional=notional,
        rate=rate, currency="EUR", discount_index="ESTR", day_counter="Actual365",
        compounding="Simple",
    ), AS_OF, side)


def loan(repayment="Bullet", cpr=0.12):
    return qs.Trade(qs.Loan(
        identifier="LOAN", start_date=AS_OF, maturity_date="2027-03-01", notional=1000,
        coupons=[qs.FixedRateCoupon(rate=qs.InterestRate(
            rate=0.06, day_counter="Actual365", compounding="Simple",
        ))], currency="EUR", repayment=repayment, payment_frequency="Monthly",
        prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(cpr),
    ), AS_OF)


def portfolio(*trades):
    return ForecastPortfolio("BOOK", [
        ForecastPosition(f"position-{i}", f"segment-{i}", trade)
        for i, trade in enumerate(trades)
    ])


class IncomeForecastTests(unittest.TestCase):
    def test_deposit_cashflows_match_native_amounts_and_coupon_metadata(self):
        with make_context() as context:
            for side in ("LongReceive", "PayShort"):
                for amount in (None, 250, 0):
                    for rate in (0.06, -0.02):
                        with self.subTest(side=side, amount=amount, rate=rate):
                            original = deposit(rate=rate)
                            trade = qs.Trade(original.instrument, AS_OF, side, notional=amount)
                            native = context.evaluate(trade, [qs.Request.Cashflows]).coupons
                            expected = native[native.type != "Disbursement"].sort_values("type").reset_index(drop=True)
                            actual = IncomeForecaster._cashflows(context, trade).sort_values("type").reset_index(drop=True)
                            pd.testing.assert_frame_equal(actual, expected)

    def test_cashflow_extractor_rejects_unsupported_direct_calls(self):
        compounded = qs.FixedRateDeposit(
            identifier="COMPOUNDED", start_date=AS_OF, maturity_date="2027-03-01",
            notional=1000, rate=0.06, currency="EUR", discount_index="ESTR",
            compounding="Compounded", rate_frequency="Semiannual",
        )
        swap = qs.Swap(
            identifier="SWAP", start_date=AS_OF, maturity_date="2027-03-01",
            notional=1000, fixed_rate=0.04, currency="EUR", market_index="ESTR",
        )
        with make_context() as context:
            with self.assertRaisesRegex(ValueError, "simple-interest"):
                IncomeForecaster._cashflows(context, qs.Trade(compounded, AS_OF))
            with self.assertRaisesRegex(ValueError, "supports Loan and FixedRateDeposit"):
                IncomeForecaster._cashflows(context, qs.Trade(swap, AS_OF))

    def test_accrual_includes_payment_after_horizon_and_excludes_principal(self):
        book = portfolio(deposit(), deposit("FUNDING", 0.02, "PayShort"))
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2026-06-01").forecast(context, book)
        # Neither annual coupon has paid, but three months of interest are earned.
        expected_days = [31, 30, 31]
        for row, days in zip(result.monthly.itertuples(), expected_days):
            self.assertAlmostEqual(row.interest_income, 1000 * 0.06 * days / 365)
            self.assertAlmostEqual(row.interest_expense, 1000 * 0.02 * days / 365)
            self.assertAlmostEqual(row.nii, 1000 * 0.04 * days / 365)
        self.assertAlmostEqual(result.monthly.net_principal_cashflow.sum(), 0)

    def test_seasoned_coupon_and_partial_final_month(self):
        book = portfolio(deposit(start="2025-12-01"))
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2026-05-15").forecast(context, book)
        days = (date(2026, 5, 15) - date(2026, 3, 1)).days
        self.assertAlmostEqual(result.monthly.nii.sum(), 1000 * 0.06 * days / 365)
        self.assertEqual(result.monthly.period_end.iloc[-1], date(2026, 5, 15))

    def test_forecast_start_in_the_middle_of_a_month(self):
        with make_context(reference="2026-03-15") as context:
            result = IncomeForecaster("2026-03-15", "2026-04-10").forecast(
                context, portfolio(deposit()),
            )
        self.assertEqual(result.monthly.period_start.tolist(), [date(2026, 3, 15), date(2026, 4, 1)])
        for actual, days in zip(result.monthly.nii, [17, 9]):
            self.assertAlmostEqual(actual, 1000 * 0.06 * days / 365)

    def test_prepayment_reduces_future_interest_once_and_conserves_principal(self):
        book = portfolio(loan())
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2026-05-01").forecast(context, book)
        remaining = 1000 * 0.88 ** (1 / 12)
        self.assertAlmostEqual(result.monthly.nii.iloc[0], 1000 * 0.06 * 31 / 365)
        self.assertAlmostEqual(result.monthly.nii.iloc[1], remaining * 0.06 * 30 / 365)
        self.assertAlmostEqual(result.monthly.asset_balance.iloc[0], remaining)
        self.assertAlmostEqual(result.cashflows.query("type == 'Redemption'").amount.sum(), 1000)

    def test_recursive_replacement_preserves_balances_and_original_contract(self):
        trade = loan("French")
        original = trade.instrument.to_dict()
        template = ProductionTemplate("LOANS", tenor_months=12,
                                      prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.12))
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2027-03-01").forecast(
                context, portfolio(trade), rollovers=[RolloverRule("segment-0", template)],
            )
        self.assertTrue((abs(result.monthly.asset_balance - 1000) < 1e-8).all())
        # Replacements themselves repay; just replacing the original book is insufficient.
        self.assertGreater(result.productions.amount.sum(), 1000)
        self.assertEqual(result.productions.date.min(), date(2026, 4, 1))
        self.assertEqual(trade.instrument.to_dict(), original)
        self.assertTrue((abs(result.monthly.net_principal_cashflow) < 1e-8).all())

    def test_scheduled_and_prepayment_replacement_fractions_are_distinct(self):
        template = ProductionTemplate("LOANS", tenor_months=12)
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2026-04-01").forecast(
                context, portfolio(loan()),
                rollovers=[RolloverRule("segment-0", template, scheduled_fraction=0, prepayment_fraction=0.5)],
            )
        prepaid = 1000 * (1 - 0.88 ** (1 / 12))
        self.assertAlmostEqual(result.productions.amount.sum(), prepaid / 2)
        self.assertAlmostEqual(result.monthly.asset_balance.iloc[0], 1000 - prepaid / 2)

    def test_renewal_lag_and_horizon_boundary(self):
        template = ProductionTemplate("DEPOSITS", kind="deposit", tenor_months=1, index="ESTR")
        book = portfolio(deposit(maturity="2026-04-01"))
        rule = RolloverRule("segment-0", template, lag_days=15)
        with make_context() as context:
            short = IncomeForecaster(AS_OF, "2026-04-01").forecast(context, book, rollovers=[rule])
            longer = IncomeForecaster(AS_OF, "2026-05-01").forecast(context, book, rollovers=[rule])
        self.assertTrue(short.productions.empty)
        self.assertAlmostEqual(short.monthly.asset_balance.iloc[-1], 0)
        self.assertEqual(longer.productions.date.tolist(), [date(2026, 4, 16)])
        self.assertAlmostEqual(longer.monthly.asset_balance.iloc[-1], 1000)
        new_coupon = longer.cashflows.query("origin == 'replacement' and type == 'FixedRateCoupon'").iloc[0]
        self.assertAlmostEqual(longer.monthly.nii.iloc[-1], 1000 * new_coupon.rate * 15 / 365)

    def test_planned_growth_and_funding_start_on_their_actual_date(self):
        assets = ProductionTemplate("ASSETS", tenor_months=12)
        funding = ProductionTemplate("FUNDING", kind="deposit", side="PayShort", index="ESTR", margin=-0.005)
        plan = [NewProduction("2026-04-15", 200, "growth-assets", assets),
                NewProduction("2026-04-15", 200, "growth-funding", funding)]
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2026-05-01").forecast(context, portfolio(deposit()), planned=plan)
        self.assertEqual(set(result.productions.origin), {"planned"})
        self.assertFalse((result.income_detail.query("origin == 'planned'").period_start < date(2026, 4, 1)).any())
        self.assertAlmostEqual(result.monthly.asset_balance.iloc[-1], 1200)
        self.assertAlmostEqual(result.monthly.liability_balance.iloc[-1], 200)
        self.assertTrue((abs(result.monthly.net_principal_cashflow) < 1e-8).all())

    def test_position_override_sign_and_full_life_income_reconcile_to_native_portfolio(self):
        asset = deposit(rate=0.06)
        half_funding = qs.Trade(asset.instrument, AS_OF, "PayShort", notional=500)
        book = portfolio(asset, half_funding)
        with make_context() as context:
            result = IncomeForecaster(AS_OF, "2027-03-01").forecast(context, book)
            native = context.evaluate(book.native, [qs.Request.Cashflows]).cashflows
        self.assertAlmostEqual(result.monthly.nii.sum(), 500 * 0.06)
        self.assertAlmostEqual(result.monthly.nii.sum(), native.query("type == 'FixedRateCoupon'").amount.sum())

    def test_independent_scenario_runs_reprice_production_and_preserve_prior_result(self):
        template = ProductionTemplate("NEW", kind="deposit", tenor_months=1, index="ESTR")
        book = portfolio(deposit(maturity="2026-04-01"))
        forecaster = IncomeForecaster(AS_OF, "2026-06-01")
        rule = [RolloverRule("segment-0", template)]
        with make_context() as context:
            base = forecaster.forecast(context, book, rollovers=rule)
            saved = base.monthly.copy(deep=True)
            with qs.Scenario(shocks=[qs.Shock("ESTR", 0.01, qs.ScenarioType.Absolute)]):
                shocked = forecaster.forecast(context, book, rollovers=rule)
            restored = forecaster.forecast(context, book, rollovers=rule)
        self.assertAlmostEqual(base.monthly.nii.iloc[0], shocked.monthly.nii.iloc[0])
        self.assertGreater(shocked.monthly.nii.sum(), base.monthly.nii.sum())
        pd.testing.assert_frame_equal(base.monthly, saved)
        pd.testing.assert_frame_equal(base.monthly, restored.monthly)

    def test_floating_resets_keep_a_known_historical_observation(self):
        floating = qs.Loan(
            identifier="FLOAT", start_date=AS_OF, maturity_date="2027-03-01",
            notional=1000, currency="EUR", repayment="Bullet", payment_frequency="Monthly",
            coupons=[qs.FloatingRateCoupon(index="EURIBOR3m", spread=0.01, day_counter="Actual360")],
            fixing_frequency="Quarterly", index_tenor="3M", fixing_lag=2,
        )
        book = portfolio(qs.Trade(floating, AS_OF))
        context = make_context()
        fixings = qs.FixingStore()
        fixings.add("EURIBOR3m", "2026-02-27", 0.025)
        context.fixings = fixings
        with context:
            forecaster = IncomeForecaster(AS_OF, "2026-09-01")
            base = forecaster.forecast(context, book)
            with qs.Scenario(shocks=[qs.Shock("EURIBOR3m", 0.01, qs.ScenarioType.Absolute)]):
                shocked = forecaster.forecast(context, book)
        self.assertAlmostEqual(base.monthly.nii.iloc[0], 1000 * 0.035 * 31 / 360)
        for first, second in zip(base.monthly.nii.iloc[:3], shocked.monthly.nii.iloc[:3]):
            self.assertAlmostEqual(first, second)
        self.assertGreater(shocked.monthly.nii.iloc[3], base.monthly.nii.iloc[3])

    def test_invalid_scope_is_rejected(self):
        with self.assertRaises(ValueError):
            RolloverRule("loans", ProductionTemplate("NEW"), prepayment_fraction=1.1)
        with self.assertRaises(ValueError):
            NewProduction(AS_OF, -1, "loans", ProductionTemplate("NEW"))
        with make_context() as context:
            forecaster = IncomeForecaster(AS_OF, "2026-06-01")
            with self.assertRaisesRegex(ValueError, "preserve"):
                forecaster.forecast(context, portfolio(deposit()), rollovers=[RolloverRule(
                    "segment-0", ProductionTemplate("WRONG-SIDE", kind="deposit", side="PayShort"),
                )])
            with self.assertRaisesRegex(ValueError, "currency"):
                forecaster.forecast(context, portfolio(deposit()), planned=[NewProduction(
                    AS_OF, 100, "USD", ProductionTemplate("USD", currency="USD"),
                )])


if __name__ == "__main__":
    unittest.main()
