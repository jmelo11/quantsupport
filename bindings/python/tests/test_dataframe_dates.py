"""Dated reports support pandas date operations, including missing and empty dates."""

import unittest

import pandas as pd
import quantsupport as qs


class DataFrameDateTests(unittest.TestCase):
    def test_custom_cashflows_and_summary_have_datetime_columns(self):
        result = qs.EvaluationResults("2026-01-15", "CUSTOM", cashflows=[
            dict(payment_date="2026-06-15", type="Redemption", amount=100.0, currency="EUR"),
        ])
        flows = result.cashflows
        self.assertEqual(flows.payment_date.dt.year.tolist(), [2026])
        self.assertEqual(flows.payment_date.iloc[0], pd.Timestamp("2026-06-15"))
        self.assertEqual(result.cashflow_summary.payment_date.dt.month.tolist(), [6])
        for column in ["accrual_start", "accrual_end"]:
            self.assertTrue(pd.api.types.is_datetime64_any_dtype(flows[column]))
            self.assertTrue(flows[column].isna().all())

    def test_empty_cashflows_keep_datetime_schema_and_absent_stays_none(self):
        self.assertIsNone(qs.EvaluationResults("2026-01-15", "ABSENT").cashflows)
        result = qs.EvaluationResults("2026-01-15", "EMPTY", cashflows=[])
        for column in ["payment_date", "accrual_start", "accrual_end"]:
            self.assertTrue(result.cashflows[column].dt.year.empty)
        self.assertTrue(result.cashflow_summary.payment_date.dt.year.empty)

    def test_fixing_dates_support_pandas_date_operations(self):
        fixings = qs.FixingStore()
        fixings.add("EURIBOR3m", "2026-01-15", 0.03)
        observations = fixings.to_dataframe("EURIBOR3m")
        self.assertEqual(observations.date.dt.day.tolist(), [15])
        self.assertEqual(observations.date.iloc[0], pd.Timestamp("2026-01-15"))
        self.assertEqual(observations.value.tolist(), [0.03])

    def test_loan_observation_dates_keep_datetime_dtypes_when_empty(self):
        for coupon in [qs.FixedRateCoupon(qs.InterestRate(0.03)),
                       qs.FloatingRateCoupon("EURIBOR3m", 0.01)]:
            with self.subTest(coupon=type(coupon).__name__):
                loan = qs.Loan("LOAN", "2026-01-15", "2027-01-15", 1000, [coupon], "EUR",
                               amortization_rate=0.04)
                observations = loan.coupon_fixing_requests()
                for column in ["fixing_date", "start_date", "end_date"]:
                    self.assertTrue(pd.api.types.is_datetime64_any_dtype(observations[column]))
                if isinstance(coupon, qs.FixedRateCoupon):
                    self.assertTrue(observations.empty)
                else:
                    self.assertEqual(observations.fixing_date.dt.year.iloc[0], 2026)
