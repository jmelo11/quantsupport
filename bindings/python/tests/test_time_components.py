"""Contracts for the public native time component wrappers."""

import ast
import datetime
import inspect
from pathlib import Path
import unittest

import quantsupport as qs
from quantsupport import time


class TimeComponentTests(unittest.TestCase):
    def test_public_exports(self):
        for name in ("Schedule", "IMM"):
            self.assertIs(getattr(qs, name), getattr(time, name))
            self.assertIn(name, qs.__all__)
            self.assertIn(name, time.__all__)

    def test_calendar_variants_and_markets(self):
        fixed = {
            "NullCalendar": "NullCalendar", "WeekendsOnly": "WeekendsOnly",
            "TARGET": "TARGET", "UnitedKingdom": "UnitedKingdom(Settlement)",
            "Japan": "Japan", "Chile": "Chile(SSE)",
        }
        for attribute, name in fixed.items():
            calendar = getattr(qs.Calendar, attribute)
            self.assertEqual(calendar.name(), name)
            self.assertEqual(qs.Calendar(name).name(), name)
        for market in ("Settlement", "LiborImpact", "Nyse", "GovernmentBond",
                       "Nerc", "FederalReserve", "Sofr"):
            calendar = qs.Calendar.UnitedStates(market=getattr(qs.UnitedStatesMarket, market))
            self.assertEqual(calendar.name(), f"UnitedStates({market})")
        for market in ("Settlement", "Exchange"):
            calendar = qs.Calendar.Brazil(market=getattr(qs.BrazilMarket, market))
            self.assertEqual(calendar.name(), f"Brazil({market})")
        self.assertEqual(qs.Calendar.UnitedStates().name(), "UnitedStates(Sofr)")
        self.assertEqual(qs.Calendar.Brazil().name(), "Brazil(Settlement)")

    def test_override_isolation_and_copied_sorted_lists(self):
        base = qs.Calendar.TARGET
        custom = base.with_added_holiday("2026-01-06").with_added_holiday("2026-01-05")
        self.assertTrue(base.is_business_day("2026-01-05"))
        self.assertFalse(custom.is_business_day("2026-01-05"))
        self.assertEqual(custom.added_holidays, [qs.Date(2026, 1, 5), qs.Date(2026, 1, 6)])
        custom.added_holidays.clear()
        self.assertEqual(len(custom.added_holidays), 2)
        self.assertEqual(qs.Calendar.TARGET.added_holidays, [])
        reopened = base.with_removed_holiday(datetime.date(2026, 1, 3)).with_removed_holiday("2026-01-01")
        self.assertTrue(reopened.is_business_day("2026-01-03"))
        self.assertTrue(reopened.is_business_day("2026-01-01"))
        self.assertFalse(base.is_business_day("2026-01-03"))
        self.assertEqual(reopened.removed_holidays, [qs.Date(2026, 1, 1), qs.Date(2026, 1, 3)])
        reopened.removed_holidays.clear()
        self.assertEqual(len(reopened.removed_holidays), 2)
        self.assertTrue(reopened.is_weekend("2026-01-03"))
        conflict = custom.with_removed_holiday("2026-01-05")
        self.assertTrue(conflict.is_holiday("2026-01-05"))
        self.assertIn(qs.Date(2026, 1, 5), conflict.removed_holidays)

    def test_month_end_and_weekends(self):
        calendar = qs.Calendar.TARGET
        self.assertEqual(calendar.end_of_month("2026-05-01"), qs.Date(2026, 5, 29))
        self.assertTrue(calendar.is_end_of_month("2026-05-29"))
        self.assertTrue(calendar.is_end_of_month("2026-05-30"))
        self.assertFalse(calendar.is_end_of_month("2026-05-28"))
        self.assertEqual(calendar.with_added_holiday("2026-05-29").end_of_month("2026-05-15"), qs.Date(2026, 5, 28))
        self.assertEqual(calendar.with_removed_holiday("2026-05-31").end_of_month("2026-05-15"), qs.Date(2026, 5, 31))
        self.assertTrue(calendar.is_weekend("2026-05-30"))
        self.assertFalse(calendar.is_weekend("2026-05-29"))
        self.assertFalse(qs.Calendar.NullCalendar.is_weekend("2026-05-30"))
        self.assertEqual(qs.Calendar.NullCalendar.end_of_month("2026-05-15"), qs.Date(2026, 5, 31))

    def test_joint_holiday_and_weekend_rules(self):
        modified = qs.Calendar.Japan.with_removed_holiday("2026-05-04")
        parts = [qs.Calendar.UnitedKingdom, modified]
        both = qs.Calendar.JointCalendar(calendars=parts)
        either = qs.Calendar.JointCalendar(calendars=parts, rule=qs.JointCalendarRule.JoinBusinessDays)
        self.assertTrue(both.is_holiday("2026-05-04"))
        self.assertTrue(either.is_business_day("2026-05-04"))
        self.assertEqual(both.removed_holidays, [])
        opened = both.with_removed_holiday("2026-05-04")
        self.assertTrue(opened.is_business_day("2026-05-04"))
        self.assertTrue(both.is_holiday("2026-05-04"))
        self.assertTrue(qs.Calendar.Japan.is_holiday("2026-05-04"))
        self.assertTrue(opened.with_added_holiday("2026-05-04").is_holiday("2026-05-04"))
        self.assertEqual(opened.with_added_holiday("2026-05-04").removed_holidays, [])
        for rule, weekend in ((qs.JointCalendarRule.JoinHolidays, True),
                              (qs.JointCalendarRule.JoinBusinessDays, False)):
            joint = qs.Calendar.JointCalendar(calendars=[qs.Calendar.NullCalendar, qs.Calendar.Japan], rule=rule)
            self.assertEqual(joint.is_weekend("2026-01-03"), weekend)
        with self.assertRaises(qs.QuantSupportError):
            qs.Calendar.JointCalendar(calendars=[qs.Calendar.TARGET])

    def test_calendar_validation(self):
        for method in (qs.Calendar.TARGET.end_of_month, qs.Calendar.TARGET.is_end_of_month,
                       qs.Calendar.TARGET.with_added_holiday, qs.Calendar.TARGET.with_removed_holiday):
            for date in ("1900-01-01", "2200-01-01", "invalid"):
                with self.subTest(method=method, date=date), self.assertRaises(qs.QuantSupportError):
                    method(date)

    def test_schedule_defaults_and_sequence(self):
        schedule = qs.Schedule("2026-01-15", "2026-07-15", "3M")
        expected = [qs.Date(2026, 1, 15), qs.Date(2026, 4, 15), qs.Date(2026, 7, 15)]
        self.assertEqual(schedule.dates, expected)
        self.assertEqual(list(schedule), expected)
        self.assertEqual(len(schedule), 3)
        self.assertEqual(schedule[-1], expected[-1])
        self.assertEqual(schedule[0], expected[0])
        self.assertEqual(schedule.is_regular, [True, True])
        self.assertEqual(schedule.calendar.name(), "NullCalendar")
        self.assertEqual(schedule.tenor, qs.Period.parse("3M"))
        self.assertEqual(schedule.rule, qs.DateGenerationRule.Backward)
        self.assertEqual(schedule.convention, qs.BusinessDayConvention.Unadjusted)
        self.assertEqual(schedule.termination_date_convention, qs.BusinessDayConvention.Unadjusted)
        self.assertFalse(schedule.end_of_month)
        self.assertIsNone(schedule.first_date)
        self.assertIsNone(schedule.next_to_last_date)
        schedule.dates.clear()
        schedule.is_regular.clear()
        self.assertEqual(list(schedule), expected)
        self.assertEqual(len(schedule.is_regular), 2)
        for index in (3, -4):
            with self.assertRaises(IndexError):
                schedule[index]
        with self.assertRaises(AttributeError):
            schedule.end_of_month = True
        zero = qs.Schedule("2026-01-15", "2026-07-15")
        self.assertEqual(zero.dates, [expected[0], expected[-1]])
        self.assertEqual(zero.rule, qs.DateGenerationRule.Zero)
        self.assertEqual(zero.tenor, qs.Period(0, qs.TimeUnit.Years))

    def test_schedule_adjustments_and_end_of_month(self):
        calendar = qs.Calendar.WeekendsOnly.with_added_holiday("2026-03-31")
        schedule = qs.Schedule("2026-01-31", "2026-05-31", frequency=qs.Frequency.Monthly,
                               calendar=calendar, convention=qs.BusinessDayConvention.ModifiedFollowing,
                               termination_date_convention=qs.BusinessDayConvention.Following,
                               rule=qs.DateGenerationRule.Forward, end_of_month=True)
        self.assertEqual([str(date) for date in schedule],
                         ["2026-01-30", "2026-02-27", "2026-03-30", "2026-04-30", "2026-06-01"])
        self.assertTrue(schedule.end_of_month)
        self.assertEqual(schedule.calendar.added_holidays, [qs.Date(2026, 3, 31)])
        self.assertEqual(schedule.calendar.with_added_holiday("2026-04-30").added_holidays,
                         [qs.Date(2026, 3, 31), qs.Date(2026, 4, 30)])
        self.assertEqual(len(schedule.calendar.added_holidays), 1)
        independent = qs.Schedule("2026-01-31", "2026-02-28", "1M", calendar=qs.Calendar.WeekendsOnly,
                                  convention=qs.BusinessDayConvention.Following)
        self.assertEqual(str(independent[-1]), "2026-02-28")

    def test_schedule_stubs_and_special_rules(self):
        schedule = qs.Schedule("2026-01-10", "2026-12-20", "3M", rule=qs.DateGenerationRule.Forward,
                               first_date="2026-02-15", next_to_last_date="2026-11-15")
        self.assertEqual([str(date) for date in schedule],
                         ["2026-01-10", "2026-02-15", "2026-05-15", "2026-08-15", "2026-11-15", "2026-12-20"])
        self.assertEqual(schedule.is_regular, [False, True, True, True, False])
        self.assertEqual(schedule.first_date, qs.Date(2026, 2, 15))
        self.assertEqual(schedule.next_to_last_date, qs.Date(2026, 11, 15))
        for rule in (qs.DateGenerationRule.ThirdWednesday, qs.DateGenerationRule.ThirdWednesdayInclusive):
            schedule = qs.Schedule("2026-01-15", "2026-07-15", "1M", rule=rule)
            dates = schedule.dates if rule == qs.DateGenerationRule.ThirdWednesdayInclusive else schedule.dates[1:-1]
            self.assertTrue(all(qs.IMM.is_imm_date(date, False) for date in dates))
        for rule in (qs.DateGenerationRule.Twentieth, qs.DateGenerationRule.TwentiethIMM,
                     qs.DateGenerationRule.OldCDS, qs.DateGenerationRule.CDS, qs.DateGenerationRule.CDS2015):
            schedule = qs.Schedule("2026-01-15", "2026-07-15", "3M", rule=rule)
            self.assertTrue(all(date.day == 20 for date in schedule.dates[1:]))

    def test_schedule_rejects_unsafe_and_inconsistent_inputs(self):
        bad = [dict(tenor="-1M"), dict(tenor="2147483647Y"),
               dict(tenor="1M", frequency=qs.Frequency.Monthly),
               dict(frequency=qs.Frequency.OtherFrequency), dict(end_of_month=True),
               dict(tenor="1M", rule=qs.DateGenerationRule.CDS, end_of_month=True),
               dict(tenor="1M", first_date="2025-01-01"),
               dict(tenor="1M", first_date="2026-06-01", next_to_last_date="2026-03-01"),
               dict(tenor="1M", rule=qs.DateGenerationRule.ThirdWednesday, first_date="2026-02-01")]
        for kwargs in bad:
            with self.subTest(kwargs=kwargs), self.assertRaises(qs.QuantSupportError):
                qs.Schedule("2026-01-01", "2026-12-01", **kwargs)
        for start, end in (("2026-01-01", "2026-01-01"), ("2027-01-01", "2026-01-01"),
                           ("1900-01-01", "2026-01-01"), ("2026-01-01", "2200-01-01")):
            with self.assertRaises(qs.QuantSupportError):
                qs.Schedule(start, end, "1M")
        with self.assertRaises(qs.QuantSupportError):
            qs.Schedule("2026-01-03", "2026-01-04", "1D", calendar=qs.Calendar.WeekendsOnly,
                        convention=qs.BusinessDayConvention.Following,
                        termination_date_convention=qs.BusinessDayConvention.Following)

    def test_imm_roundtrip_and_cycles(self):
        reference = datetime.date(2026, 1, 1)
        for month in range(1, 13):
            date = qs.IMM.next_date(qs.Date(2026, month, 1), False)
            code = qs.IMM.code(date)
            self.assertTrue(qs.IMM.is_imm_date(date, False))
            self.assertTrue(qs.IMM.is_imm_code(code.lower(), False))
            self.assertEqual(qs.IMM.date(code.lower(), reference), date)
            self.assertEqual(qs.IMM.date(code, date), date)
        self.assertEqual(qs.IMM.next_date(reference), qs.Date(2026, 3, 18))
        self.assertEqual(qs.IMM.next_code(reference), "H6")
        self.assertEqual(qs.IMM.next_date("2026-03-18"), qs.Date(2026, 6, 17))
        self.assertEqual(qs.IMM.next_date("2026-03-18", False), qs.Date(2026, 4, 15))
        self.assertEqual(qs.IMM.next_date_with_code("H6", reference), qs.Date(2026, 6, 17))
        self.assertEqual(qs.IMM.next_code_with_code("H6", reference), "M6")
        self.assertEqual(qs.IMM.next_code_with_code("H6", reference, False), "J6")
        self.assertEqual(qs.IMM.date("H6", "2026-03-19"), qs.Date(2036, 3, 19))
        self.assertFalse(qs.IMM.is_imm_code("F6"))
        self.assertFalse(qs.IMM.is_imm_date("2026-01-21"))

    def test_imm_malformed_inputs_and_bounds(self):
        for code in ("", "H", "H26", "A6", "H-", "\u00e9", "\u00e96", "H\u00e9", "\U0001f600", "H\uff16"):
            self.assertFalse(qs.IMM.is_imm_code(code, False))
            for method in (qs.IMM.date, qs.IMM.next_date_with_code, qs.IMM.next_code_with_code):
                with self.subTest(code=code, method=method), self.assertRaises(qs.QuantSupportError):
                    method(code, "2026-01-01")
        with self.assertRaises(qs.QuantSupportError):
            qs.IMM.code("2026-03-19")
        with self.assertRaises(qs.QuantSupportError):
            qs.IMM.next_date("9999-12-31")
        with self.assertRaises(qs.QuantSupportError):
            qs.IMM.date("H6", "9990-01-01")
        for method in (qs.IMM.next_date_with_code, qs.IMM.next_code_with_code):
            with self.assertRaises(qs.QuantSupportError):
                method("Z9", "9989-12-31")
        self.assertEqual(qs.IMM.next_date("9998-12-31"), qs.Date(9999, 3, 17))

    def test_calendar_list_and_count_stub_contracts(self):
        calendar = qs.Calendar.TARGET.with_added_holiday("2026-01-05")
        self.assertEqual(calendar.holiday_list("2026-01-03", "2026-01-06"), [qs.Date(2026, 1, 5)])
        self.assertEqual(calendar.business_day_list("2026-01-03", "2026-01-06"), [qs.Date(2026, 1, 6)])
        self.assertEqual(calendar.business_days_between("2026-01-03", "2026-01-06", True, True), 1)
        self.assertEqual(qs.Calendar.WeekendsOnly.holiday_list("2026-01-03", "2026-01-06", True), [])
        self.assertEqual(qs.Calendar.WeekendsOnly.business_day_list("2026-01-03", "2026-01-06"), [])

    def test_new_stub_callable_documentation(self):
        stub = Path(__file__).parents[1] / "python" / "quantsupport" / "time" / "__init__.pyi"
        tree = ast.parse(stub.read_text())
        for node in tree.body:
            if isinstance(node, ast.ClassDef) and node.name in ("Schedule", "IMM"):
                self.assertTrue(ast.get_docstring(node))
                for method in node.body:
                    if isinstance(method, ast.FunctionDef):
                        doc = ast.get_docstring(method)
                        self.assertIsNotNone(doc, f"{node.name}.{method.name}")
                        for argument in (*method.args.posonlyargs, *method.args.args, *method.args.kwonlyargs):
                            if argument.arg not in ("cls", "self"):
                                self.assertIn(f"{argument.arg} :", doc)
        self.assertIn("frequency", inspect.signature(qs.Schedule).parameters)


if __name__ == "__main__":
    unittest.main()