"""Independent income/balance checks against the notebook's forecast classes."""

from datetime import date
import json
import math
from pathlib import Path
import sys
from types import ModuleType
import unittest

import numpy as np
import pandas as pd
import quantsupport as qs

NOTEBOOK = (
    Path(__file__).resolve().parents[2]
    / "bindings/python/examples/nii_forecasting/nii_forecast.ipynb"
)
notebook = json.loads(NOTEBOOK.read_text())
definitions = "\n\n".join(
    "".join(cell["source"]) for cell in notebook["cells"]
    if cell["cell_type"] == "code"
    and "nii-forecast-definitions" in cell.get("metadata", {}).get("tags", [])
)
example = ModuleType("nii_forecast_notebook")
sys.modules[example.__name__] = example
exec(compile(definitions, str(NOTEBOOK), "exec"), example.__dict__)

ForecastPortfolio = example.ForecastPortfolio
ForecastPosition = example.ForecastPosition
IncomeForecaster = example.IncomeForecaster
NewProduction = example.NewProduction
ProductionTemplate = example.ProductionTemplate
RolloverRule = example.RolloverRule
generate_opening_portfolio = example.generate_opening_portfolio
DepositHedgingEngine = example.DepositHedgingEngine


AS_OF = date(2026, 3, 1)


def make_context(shift=0.0, reference=AS_OF):
    indices = [("ESTR", 0.025), ("EURIBOR3m", 0.03)]
    pillars = ["0D", "1Y", "10Y"]
    context = qs.PricingContext(
        quotes=qs.QuoteStore.from_dict({"reference_date": reference.isoformat(), "quotes": [
            {"identifier": f"ZeroRate_EUR_{index}_{tenor}", "mid": rate + shift}
            for index, rate in indices for tenor in pillars
        ]}),
        curves=[qs.CurveConfiguration(
            market_index=index, quotes=[f"ZeroRate_EUR_{index}_{tenor}" for tenor in pillars],
            day_counter="Actual360", interpolator="Linear",
        ) for index, _ in indices],
        csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
        fixed_income_policy=qs.FixedIncomeDiscountPolicy({"EUR": "ESTR"}),
    )
    context.pricer_configurations = [qs.PricerConfiguration.loan(
        refinancing_benchmark_index="EURIBOR3m",
        rebuild_amortization_for_fair_rate=True,
    )]
    return context


def deposit(name="DEPOSIT", rate=0.06, side="LongReceive", start=AS_OF,
            maturity=date(2027, 3, 1), notional=1000):
    return qs.Trade(qs.FixedRateDeposit(
        identifier=name, start_date=start, maturity_date=maturity, notional=notional,
        rate=rate, currency="EUR", discount_index="ESTR", day_counter="Actual365",
        compounding="Simple",
    ), AS_OF, side)


def loan(repayment="Bullet", cpr=0.12):
    return qs.Trade(qs.Loan(
        identifier="LOAN", start_date=AS_OF, maturity_date=date(2027, 3, 1), notional=1000,
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
                            native = context.evaluate(trade, [qs.Request.Cashflows]).cashflows
                            expected = native[native.type != "Disbursement"].sort_values("type").reset_index(drop=True)
                            for column in ("payment_date", "accrual_start", "accrual_end"):
                                expected[column] = pd.to_datetime(expected[column]).dt.date
                            actual = IncomeForecaster.read_cashflows(context, trade).sort_values("type").reset_index(drop=True)
                            pd.testing.assert_frame_equal(actual, expected)


    def test_accrual_includes_payment_after_horizon_and_excludes_principal(self):
        book = portfolio(deposit(), deposit("FUNDING", 0.02, "PayShort"))
        with make_context() as context:
            result = IncomeForecaster(AS_OF, date(2026, 6, 1)).forecast(context, book)
        # Neither annual coupon has paid, but three months of interest are earned.
        expected_days = [31, 30, 31]
        for row, days in zip(result.monthly.itertuples(), expected_days):
            self.assertAlmostEqual(row.interest_income, 1000 * 0.06 * days / 365)
            self.assertAlmostEqual(row.interest_expense, 1000 * 0.02 * days / 365)
            self.assertAlmostEqual(row.nii, 1000 * 0.04 * days / 365)
        self.assertAlmostEqual(result.monthly.net_principal_cashflow.sum(), 0)

    def test_seasoned_coupon_and_partial_final_month(self):
        book = portfolio(deposit(start=date(2025, 12, 1)))
        with make_context() as context:
            result = IncomeForecaster(AS_OF, date(2026, 5, 15)).forecast(context, book)
        days = (date(2026, 5, 15) - date(2026, 3, 1)).days
        self.assertAlmostEqual(result.monthly.nii.sum(), 1000 * 0.06 * days / 365)
        self.assertEqual(result.monthly.period_end.iloc[-1], date(2026, 5, 15))

    def test_forecast_start_in_the_middle_of_a_month(self):
        with make_context(reference=date(2026, 3, 15)) as context:
            result = IncomeForecaster(date(2026, 3, 15), date(2026, 4, 10)).forecast(
                context, portfolio(deposit()),
            )
        self.assertEqual(result.monthly.period_start.tolist(), [date(2026, 3, 15), date(2026, 4, 1)])
        for actual, days in zip(result.monthly.nii, [17, 9]):
            self.assertAlmostEqual(actual, 1000 * 0.06 * days / 365)

    def test_prepayment_reduces_future_interest_once_and_conserves_principal(self):
        book = portfolio(loan())
        with make_context() as context:
            result = IncomeForecaster(AS_OF, date(2026, 5, 1)).forecast(context, book)
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
            result = IncomeForecaster(AS_OF, date(2027, 3, 1)).forecast(
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
            result = IncomeForecaster(AS_OF, date(2026, 4, 1)).forecast(
                context, portfolio(loan()),
                rollovers=[RolloverRule("segment-0", template, scheduled_fraction=0, prepayment_fraction=0.5)],
            )
        prepaid = 1000 * (1 - 0.88 ** (1 / 12))
        self.assertAlmostEqual(result.productions.amount.sum(), prepaid / 2)
        self.assertAlmostEqual(result.monthly.asset_balance.iloc[0], 1000 - prepaid / 2)

    def test_renewal_lag_and_horizon_boundary(self):
        template = ProductionTemplate("DEPOSITS", kind="deposit", tenor_months=1, index="ESTR")
        book = portfolio(deposit(maturity=date(2026, 4, 1)))
        rule = RolloverRule("segment-0", template, lag_days=15)
        with make_context() as context:
            short = IncomeForecaster(AS_OF, date(2026, 4, 1)).forecast(context, book, rollovers=[rule])
            longer = IncomeForecaster(AS_OF, date(2026, 5, 1)).forecast(context, book, rollovers=[rule])
        self.assertTrue(short.productions.empty)
        self.assertAlmostEqual(short.monthly.asset_balance.iloc[-1], 0)
        self.assertEqual(longer.productions.date.tolist(), [date(2026, 4, 16)])
        self.assertAlmostEqual(longer.monthly.asset_balance.iloc[-1], 1000)
        new_coupon = longer.cashflows.query("origin == 'replacement' and type == 'FixedRateCoupon'").iloc[0]
        self.assertAlmostEqual(longer.monthly.nii.iloc[-1], 1000 * new_coupon.rate * 15 / 365)

    def test_planned_growth_and_funding_start_on_their_actual_date(self):
        assets = ProductionTemplate("ASSETS", tenor_months=12)
        funding = ProductionTemplate("FUNDING", kind="deposit", side="PayShort", index="ESTR", margin=-0.005)
        plan = [NewProduction(date(2026, 4, 15), 200, "growth-assets", assets),
                NewProduction(date(2026, 4, 15), 200, "growth-funding", funding)]
        with make_context() as context:
            result = IncomeForecaster(AS_OF, date(2026, 5, 1)).forecast(context, portfolio(deposit()), planned=plan)
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
            result = IncomeForecaster(AS_OF, date(2027, 3, 1)).forecast(context, book)
            native = context.evaluate(book.native, [qs.Request.Cashflows]).cashflows
        self.assertAlmostEqual(result.monthly.nii.sum(), 500 * 0.06)
        self.assertAlmostEqual(result.monthly.nii.sum(), native.query("type == 'FixedRateCoupon'").amount.sum())

    def test_independent_scenario_runs_reprice_production_and_preserve_prior_result(self):
        template = ProductionTemplate("NEW", kind="deposit", tenor_months=1, index="ESTR")
        book = portfolio(deposit(maturity=date(2026, 4, 1)))
        forecaster = IncomeForecaster(AS_OF, date(2026, 6, 1))
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
            identifier="FLOAT", start_date=AS_OF, maturity_date=date(2027, 3, 1),
            notional=1000, currency="EUR", repayment="Bullet", payment_frequency="Monthly",
            coupons=[qs.FloatingRateCoupon(index="EURIBOR3m", spread=0.01, day_counter="Actual360")],
            fixing_frequency="Quarterly", index_tenor="3M", fixing_lag=2,
        )
        book = portfolio(qs.Trade(floating, AS_OF))
        context = make_context()
        fixings = qs.FixingStore()
        fixings.add("EURIBOR3m", date(2026, 2, 27), 0.025)
        context.fixings = fixings
        with context:
            forecaster = IncomeForecaster(AS_OF, date(2026, 9, 1))
            base = forecaster.forecast(context, book)
            with qs.Scenario(shocks=[qs.Shock("EURIBOR3m", 0.01, qs.ScenarioType.Absolute)]):
                shocked = forecaster.forecast(context, book)
        self.assertAlmostEqual(base.monthly.nii.iloc[0], 1000 * 0.035 * 31 / 360)
        for first, second in zip(base.monthly.nii.iloc[:3], shocked.monthly.nii.iloc[:3]):
            self.assertAlmostEqual(first, second)
        self.assertGreater(shocked.monthly.nii.iloc[3], base.monthly.nii.iloc[3])

    def test_fixed_loan_production_prices_its_amortization_and_forward_advance(self):
        start = date(2026, 6, 17)
        template = ProductionTemplate("PAR-FIXED", tenor_months=36, margin=0,
                                      prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.12))
        with make_context() as context:
            trade, quote = template.build(context, "NEW", start, 125_000)
            value = context.evaluate(trade, [qs.Request.Value]).price
            curve = context.curve("ESTR")
            advance = 125_000 * curve.discount_factor(start)
            self.assertAlmostEqual(value, advance, delta=0.0001)
            self.assertEqual(quote.quote_type, "fixed_coupon")
            self.assertAlmostEqual(trade.instrument.to_dict()["amortization_rate"], quote.par_quote)
            # The old maturity forward is not the par coupon of this amortizing contract.
            maturity = date(2029, 6, 17)
            tau = qs.DayCounter.Actual365.year_fraction(start, maturity)
            bullet_forward = (curve.discount_factor(start) / curve.discount_factor(maturity) - 1) / tau
            self.assertGreater(abs(quote.par_quote - bullet_forward), 0.0001)

    def test_floating_production_solves_a_spread_and_uses_native_index_conventions(self):
        start = date(2026, 6, 17)
        template = ProductionTemplate("PAR-FLOAT", tenor_months=36, rate_type="floating",
                                      day_counter="Actual365", margin=0)
        with make_context() as context:
            trade, quote = template.build(context, "NEW-FLOAT", start, 80_000)
            result = context.evaluate(trade, [qs.Request.Value, qs.Request.Cashflows])
            expected_advance = 80_000 * context.curve("ESTR").discount_factor(start)
            self.assertAlmostEqual(result.price, expected_advance, delta=0.0001)
            self.assertEqual(quote.quote_type, "floating_spread")
            self.assertLess(quote.par_quote, 0)  # 3% index versus 2.5% discounting.
            first = result.cashflows.query("type == 'FloatingRateCoupon'").iloc[0]
            self.assertAlmostEqual(first.spread, quote.par_quote)
            self.assertAlmostEqual(quote.initial_rate, first.fixing + first.spread)
            self.assertAlmostEqual(trade.instrument.to_dict()["amortization_rate"], quote.initial_rate)
            # EURIBOR quotes on Actual/360 even though this loan accrues on Actual/365.
            index_tau = qs.DayCounter.Actual360.year_fraction(start, date(2026, 9, 17))
            curve = context.curve("EURIBOR3m")
            native_fixing = (curve.discount_factor(start) / curve.discount_factor(date(2026, 9, 17)) - 1) / index_tau
            self.assertAlmostEqual(first.fixing, native_fixing)

    def test_french_par_coupon_matches_an_independent_annuity_valuation(self):
        start = date(2026, 6, 17)
        dates = [date(start.year + (start.month - 1 + i) // 12,
                      (start.month - 1 + i) % 12 + 1, 17) for i in range(37)]
        fractions = [(end - begin).days / 365 for begin, end in zip(dates, dates[1:])]
        # The fixture supplies continuously compounded 2.5% funding zero rates on ACT/360.
        discount = lambda when: math.exp(-0.025 * (when - date(2026, 3, 1)).days / 360)

        def receipt_pv(rate):
            running, annuity = 1.0, 0.0
            for fraction in fractions:
                running /= 1 + rate * fraction
                annuity += running
            instalment = 1 / annuity
            return instalment * sum(discount(when) for when in dates[1:])

        lower, upper = 0.0, 0.10
        for _ in range(60):
            middle = (lower + upper) / 2
            if receipt_pv(middle) > discount(start):
                upper = middle
            else:
                lower = middle
        independent_coupon = (lower + upper) / 2
        with make_context() as context:
            quote = ProductionTemplate("ANNUITY", tenor_months=36, margin=0).quote(context, start)
        self.assertAlmostEqual(quote.par_quote, independent_coupon, delta=1e-10)

    def test_floating_par_spread_matches_independent_discounted_coupon_equation(self):
        start = date(2026, 6, 17)
        dates = [date(start.year + (start.month - 1 + i) // 12,
                      (start.month - 1 + i) % 12 + 1, 17) for i in range(0, 13, 3)]
        reference = date(2026, 3, 1)
        funding_df = lambda when: math.exp(-0.025 * (when - reference).days / 360)
        projection_df = lambda when: math.exp(-0.03 * (when - reference).days / 360)
        floating_pv, annuity = 0.0, 0.0
        for begin, end in zip(dates, dates[1:]):
            index_fraction = (end - begin).days / 360
            coupon_fraction = (end - begin).days / 365
            fixing = (projection_df(begin) / projection_df(end) - 1) / index_fraction
            weight = coupon_fraction * funding_df(end)
            floating_pv += fixing * weight
            annuity += weight
        independent_spread = (funding_df(start) - funding_df(dates[-1]) - floating_pv) / annuity
        with make_context() as context:
            quote = ProductionTemplate("FLOAT-BULLET", tenor_months=12, margin=0,
                                       rate_type="floating", repayment="Bullet",
                                       payment_frequency="Quarterly", day_counter="Actual365").quote(context, start)
        self.assertAlmostEqual(quote.par_quote, independent_spread, delta=1e-10)

    def test_commercial_margin_is_added_once_to_the_correct_native_quote(self):
        with make_context() as context:
            for rate_type in ("fixed", "floating"):
                with self.subTest(rate_type=rate_type):
                    template = ProductionTemplate("MARGIN", rate_type=rate_type, margin=0.0125)
                    trade, quote = template.build(context, "WITH-MARGIN", date(2026, 4, 20), 10_000)
                    self.assertAlmostEqual(quote.contract_rate_or_spread, quote.par_quote + 0.0125)
                    terms = trade.instrument.to_dict()["coupons"][0]
                    if rate_type == "fixed":
                        actual = terms["FixedRateCoupon"]["rate"]["rate"]
                    else:
                        actual = terms["FloatingRateCoupon"]["spread"]
                    self.assertAlmostEqual(actual, quote.contract_rate_or_spread)

    def test_deposit_production_prices_the_forward_start_on_each_day_count(self):
        with make_context() as context:
            for basis in ("Actual360", "Actual365"):
                for side in ("LongReceive", "PayShort"):
                    with self.subTest(basis=basis, side=side):
                        template = ProductionTemplate("DEPOSIT", kind="deposit", tenor_months=7,
                                                      index="ESTR", day_counter=basis, margin=0, side=side)
                        trade, quote = template.build(context, "FORWARD-DEPOSIT", date(2026, 5, 10), 40_000)
                        result = context.evaluate(trade, [qs.Request.Value])
                        self.assertAlmostEqual(result.price, 0, delta=1e-5)
                        self.assertAlmostEqual(trade.instrument.to_dict()["rate"], quote.par_quote)

    def test_new_fixed_quotes_use_the_funding_curve_and_are_not_cached_between_scenarios(self):
        template = ProductionTemplate("FIXED", margin=0, tenor_months=36)
        with make_context() as context:
            base = template.quote(context, date(2026, 6, 1))
            with qs.Scenario(shocks=[qs.Shock("EURIBOR3m", 0.01, qs.ScenarioType.Absolute)]):
                projection_shift = template.quote(context, date(2026, 6, 1))
            with qs.Scenario(shocks=[qs.Shock("ESTR", 0.01, qs.ScenarioType.Absolute)]):
                funding_shift = template.quote(context, date(2026, 6, 1))
            restored = template.quote(context, date(2026, 6, 1))
        self.assertAlmostEqual(base.par_quote, projection_shift.par_quote)
        self.assertGreater(funding_shift.par_quote, base.par_quote + 0.009)
        self.assertEqual(base, restored)


    def test_monthly_accrual_uses_resolved_quarterly_coupon_and_expected_principal(self):
        instrument = qs.Loan(
            identifier="SEASONED-FLOAT", start_date=date(2026, 1, 15), maturity_date=date(2026, 7, 15),
            notional=1200, currency="EUR", repayment="Bullet", payment_frequency="Quarterly",
            fixing_frequency="Quarterly", fixing_lag=0, index_tenor="3M",
            coupons=[qs.FloatingRateCoupon(index="EURIBOR3m", spread=0.012, day_counter="Actual360")],
            amortization_rate=0.042,
        )
        context = make_context()
        fixings = qs.FixingStore()
        fixings.add("EURIBOR3m", date(2026, 1, 15), 0.03)
        context.fixings = fixings
        with context:
            result = IncomeForecaster(AS_OF, date(2026, 4, 1)).forecast(
                context, portfolio(qs.Trade(instrument, AS_OF)))
        self.assertAlmostEqual(result.monthly.nii.iloc[0], 4.34)
        detail = result.income_detail.iloc[0]
        coupon = result.cashflows.query("type == 'FloatingRateCoupon'").iloc[0]
        self.assertAlmostEqual(coupon.notional, 1200)
        self.assertAlmostEqual(coupon.fixing + coupon.spread, 0.042)
        self.assertEqual(detail.period_start, date(2026, 3, 1))
        self.assertEqual(detail.period_end, date(2026, 4, 1))

    def test_accrual_preserves_zero_negative_rates_and_position_overrides(self):
        with make_context() as context:
            for rate, expected in ((0.0, 0.0), (-0.02, -0.4246575342465753)):
                for side, sign in (("LongReceive", 1), ("PayShort", -1)):
                    trade = qs.Trade(deposit(rate=rate).instrument, AS_OF, side, notional=250)
                    result = IncomeForecaster(AS_OF, date(2026, 4, 1)).forecast(context, portfolio(trade))
                    self.assertAlmostEqual(result.monthly.nii.iloc[0], sign * expected)
                    self.assertAlmostEqual(result.cashflows.query("type == 'FixedRateCoupon'").notional.iloc[0], sign * 250)

    def test_generated_portfolio_has_300_individual_positions_and_matched_current_funding(self):
        context = make_context()
        book, inventory = generate_opening_portfolio(context, AS_OF)
        self.assertEqual(len(book.positions), 300)
        self.assertEqual(inventory.groupby("segment").size().to_dict(),
                         {"Fixed loans": 120, "Floating loans": 80, "Term funding": 100})
        self.assertEqual(inventory.position.nunique(), 300)
        self.assertGreater(inventory.original_notional.nunique(), 250)
        self.assertGreater(inventory.start_date.nunique(), 20)
        self.assertGreater(inventory.maturity_date.nunique(), 20)
        self.assertEqual(set(inventory.repayment), {"French", "Bullet", "Balloon"})
        self.assertEqual(set(inventory.day_counter), {qs.DayCounter.Actual360, qs.DayCounter.Actual365})
        assets = inventory.query("segment != 'Term funding'")
        funding = inventory.query("segment == 'Term funding'")
        self.assertAlmostEqual(assets.current_principal.sum(), funding.current_principal.sum(), delta=1e-7)
        self.assertLess(assets.current_principal.sum(), assets.original_notional.sum())
        with context:
            result = IncomeForecaster(AS_OF, date(2026, 5, 1)).forecast(context, book)
        self.assertEqual(result.income_detail.position.nunique(), 3)
        self.assertEqual(set(result.cashflows.origin), {"existing"})

    def test_forecast_evaluation_calls_scale_with_portfolio_groups(self):
        context = make_context()
        book, _ = generate_opening_portfolio(
            context, AS_OF, n_fixed=40, n_floating=30, n_deposits=30)
        calls = []

        class CountedContext:
            def evaluate(self, item, requests):
                calls.append((item, requests))
                return context.evaluate(item, requests)

        with context:
            IncomeForecaster(AS_OF, date(2028, 3, 1)).forecast(CountedContext(), book)

        self.assertTrue(all(isinstance(item, qs.Portfolio) for item, _ in calls))
        cashflow_groups = [item for item, requests in calls
                           if requests == [qs.Request.Cashflows]]
        self.assertEqual(sorted(map(len, cashflow_groups)), [30, 30, 40])
        self.assertLessEqual(len(calls), 3 * (1 + 24))

    def test_grouped_forecast_matches_position_audit_with_rollover_growth_and_shocks(self):
        book = ForecastPortfolio("MIXED", [
            ForecastPosition("bullet", "Retail", loan()),
            ForecastPosition("amortizing", "Retail", loan("French")),
            # A deposit and loans share a business segment and balance-sheet side.
            ForecastPosition("deposit", "Retail", deposit(maturity=date(2026, 6, 1))),
            ForecastPosition("funding", "Funding", deposit(rate=0.025, side="PayShort")),
        ])
        lending = ProductionTemplate("NEW-LOANS", tenor_months=6,
                                     prepayment=qs.PrepaymentModelConfiguration.constant_annual_cpr(0.12))
        funding = ProductionTemplate("NEW-FUNDING", kind="deposit", tenor_months=3,
                                     index="ESTR", side="PayShort")
        rules = [RolloverRule("Retail", lending), RolloverRule("Funding", funding)]
        planned = [NewProduction(date(2026, 4, 15), 200, "Retail", lending),
                   NewProduction(date(2026, 4, 15), 200, "Funding", funding)]
        forecaster = IncomeForecaster(AS_OF, date(2027, 3, 1))
        with make_context() as context:
            for shift in (0, 0.01):
                with self.subTest(shift=shift), qs.Scenario(shocks=[
                    qs.Shock("ESTR", shift, qs.ScenarioType.Absolute),
                    qs.Shock("EURIBOR3m", shift, qs.ScenarioType.Absolute),
                ]):
                    grouped = forecaster.forecast(context, book, rollovers=rules, planned=planned)
                    detailed = forecaster.forecast(
                        context, book, rollovers=rules, planned=planned, position_detail=True)
                    pd.testing.assert_frame_equal(grouped.monthly, detailed.monthly,
                                                  rtol=1e-10, atol=1e-8)
                    pd.testing.assert_frame_equal(grouped.balances, detailed.balances,
                                                  rtol=1e-10, atol=1e-8)
                    keys = ["payment_date", "type", "leg_index", "segment", "origin"]
                    pd.testing.assert_series_equal(
                        grouped.cashflows.groupby(keys).amount.sum(),
                        detailed.cashflows.groupby(keys).amount.sum(), rtol=1e-10, atol=1e-8)
                    audited = detailed.income_detail.query("origin == 'existing'")
                    self.assertEqual(set(audited.position), {p.name for p in book.positions})

    def test_generated_contracts_are_reproducible_and_replacement_keeps_funding_matched(self):
        first = make_context()
        second = make_context()
        book, inventory = generate_opening_portfolio(first, AS_OF, seed=17,
                                                     n_fixed=6, n_floating=4, n_deposits=5)
        repeated, again = generate_opening_portfolio(second, AS_OF, seed=17,
                                                    n_fixed=6, n_floating=4, n_deposits=5)
        pd.testing.assert_frame_equal(inventory, again)
        self.assertEqual([p.trade.instrument.to_dict() for p in book.positions],
                         [p.trade.instrument.to_dict() for p in repeated.positions])
        rules = [RolloverRule.constant_balance(example.Segment.FIXED_LOANS, ProductionTemplate("FIXED")),
                 RolloverRule.constant_balance(example.Segment.FLOATING_LOANS,
                     ProductionTemplate("FLOAT", rate_type=example.RateType.FLOATING)),
                 RolloverRule.constant_balance(example.Segment.TERM_FUNDING,
                     ProductionTemplate("FUNDS", kind=example.ProductKind.DEPOSIT,
                                        side=qs.Side.PayShort, index=qs.MarketIndex.ESTR))]
        with first:
            result = IncomeForecaster(AS_OF, date(2026, 7, 1)).forecast(first, book, rollovers=rules)
        for row in result.monthly.itertuples(index=False):
            self.assertAlmostEqual(row.asset_balance, row.liability_balance, delta=1e-7)
            self.assertAlmostEqual(row.net_principal_cashflow, 0, delta=1e-7)
        summary = example.ForecastTables.constant_balance_table(result)
        np.testing.assert_allclose(summary.minimum_closing, summary.initial_balance, rtol=0, atol=1e-7)
        np.testing.assert_allclose(summary.maximum_closing, summary.initial_balance, rtol=0, atol=1e-7)

    def test_forecast_reports_use_python_dates(self):
        template = ProductionTemplate("GROWTH", kind=example.ProductKind.DEPOSIT,
                                      index=qs.MarketIndex.ESTR)
        with make_context() as context:
            result = IncomeForecaster(AS_OF, date(2026, 6, 1)).forecast(
                context, portfolio(deposit()),
                planned=[NewProduction(date(2026, 4, 15), 200, example.Segment.FIXED_LOANS, template)])
        for table, columns in (
            (result.monthly, ("period_start", "period_end")),
            (result.balances, ("period_start", "period_end")),
            (result.productions, ("date", "maturity_date")),
            (result.cashflows, ("payment_date", "accrual_start", "accrual_end")),
        ):
            for column in columns:
                with self.subTest(column=column):
                    self.assertTrue(all(type(value) is date for value in table[column].dropna()))

    def test_native_accrual_handles_thirty_360_coupon_boundaries(self):
        instrument = qs.FixedRateDeposit(
            identifier="THIRTY360", start_date=date(2026, 1, 31), maturity_date=date(2026, 3, 31),
            notional=1000, rate=0.06, currency="EUR", discount_index="ESTR",
            day_counter="Thirty360", compounding="Simple",
        )
        with make_context() as context:
            result = IncomeForecaster(AS_OF, date(2026, 4, 1)).forecast(
                context, portfolio(qs.Trade(instrument, AS_OF)))
        # Accrual to Mar 31 minus accrual to Mar 1 is 29/360 of the annual coupon.
        self.assertAlmostEqual(result.monthly.nii.iloc[0], 1000 * 0.06 * 29 / 360)


class DepositHedgingTests(unittest.TestCase):
    @staticmethod
    def context():
        context = make_context()
        observations = {"reference_date": AS_OF.isoformat(), "quotes": [
            {"identifier": label, "mid": context.quotes.levels(label)[0]}
            for label in context.quotes.identifiers()
        ]}
        labels = [f"ZeroRate_EUR_FUNDINGSPREAD_{tenor}" for tenor in ("0D", "1Y", "10Y")]
        observations["quotes"].extend({"identifier": label, "mid": 0.005} for label in labels)
        context.quotes = qs.QuoteStore.from_dict(observations)
        funding, spread = qs.MarketIndex.other("LOANFUNDING"), qs.MarketIndex.other("FUNDINGSPREAD")
        context.curves = [*context.curves,
                          qs.CurveConfiguration(spread, labels, day_counter="Actual360", interpolator="Linear"),
                          qs.CurveConfiguration.composite(funding, "ESTR", spread)]
        context.fixed_income_policy = qs.FixedIncomeDiscountPolicy({"EUR": funding})
        return context

    @staticmethod
    def templates(indices=("ESTR", "LOANFUNDING", "EURIBOR3m")):
        return [ProductionTemplate(f"{index}-{months}M", kind="deposit", tenor_months=months,
                                   index=qs.MarketIndex.other(index) if index == "LOANFUNDING" else index,
                                   margin=0, day_counter="Actual360")
                for index in indices for months in (1, 6, 12, 120)]

    def test_joint_hedge_creates_signed_trades_and_reprices_all_source_risk(self):
        context = self.context()
        book, _ = generate_opening_portfolio(context, AS_OF, n_fixed=3, n_floating=2,
                                              n_deposits=4, funding_index=qs.MarketIndex.other("LOANFUNDING"))
        contracts = [p.trade.instrument.to_dict() for p in book.positions]
        engine = DepositHedgingEngine(self.templates())
        with context:
            result = engine.hedge(context, book, engine.sensitivity_report(context, book))
        self.assertEqual(set(result.tickets.side), {qs.Side.LongReceive, qs.Side.PayShort})
        self.assertTrue((result.tickets.notional > 0).all())
        self.assertAlmostEqual(result.tickets.signed_notional.sum(), 0, delta=1e-7)
        self.assertEqual(result.rank, 9)
        self.assertLess(np.linalg.norm(result.risk.after_pv01), 1e-3)
        np.testing.assert_allclose(result.risk.after_pv01, result.risk.predicted_after_pv01,
                                   atol=1e-7, rtol=1e-7)
        self.assertTrue(result.risk.index.str.contains("FUNDINGSPREAD").any())
        self.assertEqual([p.trade.instrument.to_dict() for p in book.positions], contracts)
        for position, ticket in zip(result.portfolio.positions[len(book.positions):],
                                    result.tickets.itertuples(index=False)):
            self.assertEqual(position.trade.side, ticket.side)
            self.assertEqual(position.trade.notional, ticket.notional)
            self.assertEqual(position.trade.instrument.to_dict()["rate"], ticket.rate)

    def test_seasoned_risk_preserves_original_future_coupon_and_position_override(self):
        engine = DepositHedgingEngine(self.templates(("ESTR",)))
        original = deposit(start=date(2025, 9, 1), maturity=date(2027, 3, 1))
        trade = qs.Trade(original.instrument, AS_OF, "PayShort", notional=250)
        book = portfolio(trade)
        with make_context() as context:
            report = engine.sensitivity_report(context, book)
            receipts = context.evaluate(trade, [qs.Request.Cashflows]).cashflows
            receipts["payment_date"] = pd.to_datetime(receipts.payment_date).dt.date
            future = receipts[receipts.payment_date > AS_OF]
            values = []
            epsilon = 1e-6
            for shift in (-epsilon, epsilon):
                with qs.Scenario(shocks=[qs.Shock("ESTR", shift, qs.ScenarioType.Absolute)]):
                    curve = context.curve("ESTR")
                    values.append(sum(row.amount * curve.discount_factor(row.payment_date)
                                      for row in future.itertuples(index=False)))
        # The independent valuation uses the original future coupon and principal,
        # including already-accrued interest, while excluding the settled advance.
        self.assertAlmostEqual(report.value.sum(), (values[1] - values[0]) / (2 * epsilon), delta=1e-5)
        self.assertEqual(trade.instrument.to_dict()["start_date"], "2025-09-01")

    def test_missing_hedge_curve_remains_visible_in_the_residual(self):
        context = self.context()
        book, _ = generate_opening_portfolio(context, AS_OF, n_fixed=1, n_floating=2,
                                              n_deposits=2, funding_index=qs.MarketIndex.other("LOANFUNDING"))
        engine = DepositHedgingEngine(self.templates(("ESTR",)))
        with context:
            result = engine.hedge(context, book, engine.sensitivity_report(context, book))
        uncovered = result.risk.index.str.contains("FUNDINGSPREAD|EURIBOR3m")
        self.assertGreater(np.linalg.norm(result.risk.loc[uncovered, "before_pv01"]), 1)
        np.testing.assert_allclose(result.risk.loc[uncovered, "after_pv01"],
                                   result.risk.loc[uncovered, "before_pv01"], atol=1e-8)
        self.assertGreater(np.linalg.norm(result.risk.after_pv01), 1)

    def test_zero_risk_creates_no_hedge_tickets(self):
        asset = deposit()
        book = portfolio(asset, qs.Trade(asset.instrument, AS_OF, "PayShort"))
        engine = DepositHedgingEngine(self.templates(("ESTR",)))
        with make_context() as context:
            result = engine.hedge(context, book, engine.sensitivity_report(context, book))
        self.assertTrue(result.tickets.empty)
        self.assertEqual(result.portfolio.positions, book.positions)
        self.assertEqual(np.linalg.norm(result.risk.after_pv01), 0)

    def test_fixed_hedges_reconcile_to_combined_forecast_under_rate_scenarios(self):
        context = self.context()
        book, _ = generate_opening_portfolio(context, AS_OF, n_fixed=1, n_floating=1,
                                              n_deposits=2, funding_index=qs.MarketIndex.other("LOANFUNDING"))
        engine = DepositHedgingEngine(self.templates())
        forecaster = IncomeForecaster(AS_OF, date(2026, 6, 1))
        with context:
            result = engine.hedge(context, book, engine.sensitivity_report(context, book))
            hedges = ForecastPortfolio("HEDGES", result.portfolio.positions[len(book.positions):])
            contracts = [p.trade.instrument.to_dict() for p in hedges.positions]
            hedge_income = []
            for shift in (0, 0.01):
                with qs.Scenario(shocks=[qs.Shock("ESTR", shift, qs.ScenarioType.Absolute)]):
                    original = forecaster.forecast(context, book).monthly
                    incremental = forecaster.forecast(context, hedges).monthly
                    combined = forecaster.forecast(context, result.portfolio).monthly
                    np.testing.assert_allclose(combined.nii, original.nii + incremental.nii, atol=1e-7)
                    np.testing.assert_allclose(combined.net_principal_cashflow,
                                               original.net_principal_cashflow + incremental.net_principal_cashflow,
                                               atol=1e-7)
                    hedge_income.append(incremental.nii.to_numpy())
            np.testing.assert_allclose(hedge_income[0], hedge_income[1], atol=1e-8)
            self.assertEqual([p.trade.instrument.to_dict() for p in hedges.positions], contracts)


if __name__ == "__main__":
    unittest.main()
