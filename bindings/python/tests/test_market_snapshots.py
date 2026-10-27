"""Typed instants and reproducible historical market selection."""

import json
from pathlib import Path

from datetime import datetime, timezone
import os
import tempfile
import unittest

import quantsupport as qs


IDENTIFIER = "FixedRateDeposit_USD_SOFR_6M"


def market(day, rate, *, only_mid=False):
    observation = {"identifier": IDENTIFIER, "mid": rate}
    if not only_mid:
        observation.update(bid=rate - 0.01, ask=rate + 0.01)
    return qs.PricingContext(
        quotes=qs.QuoteStore.from_dict({
            "reference_date": f"2025-01-{day:02d}", "quotes": [observation],
        }),
        curves=[qs.CurveConfiguration(market_index="SOFR", quotes=[IDENTIFIER])],
    )


class DateTimeTests(unittest.TestCase):
    def test_components_conversions_ordering_and_precision(self):
        instant = qs.DateTime(2025, 1, 15, 10, 30, 45)
        self.assertEqual(instant.date, qs.Date(2025, 1, 15))
        self.assertEqual((instant.year, instant.month, instant.day), (2025, 1, 15))
        self.assertEqual((instant.hour, instant.minute, instant.second), (10, 30, 45))
        self.assertEqual(qs.DateTime.from_date(instant.date, 10, 30, 45), instant)
        self.assertEqual(qs.DateTime.from_datetime(instant.to_datetime()), instant)
        self.assertEqual(instant.to_datetime(), datetime(2025, 1, 15, 10, 30, 45, tzinfo=timezone.utc))
        self.assertEqual(hash(instant), hash(qs.DateTime.parse(str(instant))))
        self.assertLess(instant, qs.DateTime(2025, 1, 16))
        precise = qs.DateTime.parse("2025-01-16T00:30:45.123456789+02:00")
        self.assertEqual(precise.date, qs.Date(2025, 1, 15))
        self.assertEqual(precise.nanosecond, 123456789)
        self.assertEqual(qs.DateTime.parse(str(precise)), precise)
        self.assertEqual(precise.to_datetime().microsecond, 123456)
        self.assertEqual(qs.DateTime.parse("2025-01-15 10:30:45 +0000", "%Y-%m-%d %H:%M:%S %z"), instant)
        self.assertEqual(instant.to_str("%Y-%m-%d %H:%M"), "2025-01-15 10:30")

    def test_to_datetime_truncates_fractional_seconds_in_utc(self):
        for fraction, nanosecond in [
            ("", 0),
            (".123", 123000000),
            (".123456", 123456000),
            (".123456789", 123456789),
            (".000000001", 1),
            (".000001999", 1999),
            (".999999999", 999999999),
        ]:
            with self.subTest(fraction=fraction):
                instant = qs.DateTime.parse(f"2025-01-16T00:30:45{fraction}+02:00")
                expected = datetime(2025, 1, 15, 22, 30, 45, nanosecond // 1000, tzinfo=timezone.utc)
                converted = instant.to_datetime()
                self.assertEqual(converted, expected)
                self.assertIs(converted.tzinfo, timezone.utc)
                self.assertEqual(qs.DateTime.from_datetime(converted).nanosecond, nanosecond // 1000 * 1000)
                self.assertEqual(instant.nanosecond, nanosecond)

    def test_invalid_dates_and_naive_inputs_fail(self):
        for components in [(2025, 2, 29), (2025, 1, 15, 24)]:
            with self.assertRaises(qs.QuantSupportError):
                qs.DateTime(*components)
        for value in ["2025-01-15", "2025-01-15T10:00:00", datetime(2025, 1, 15, 10)]:
            with self.assertRaises(qs.QuantSupportError):
                qs.SnapshotSelector(value)


class MarketSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.store = qs.MarketSnapshotStore()
        for day, hour, rate in [(15, 10, 0.04), (15, 14, 0.06), (16, 10, 0.08)]:
            self.store.insert(qs.MarketSnapshot.from_context(
                qs.DateTime(2025, 1, day, hour), market(day, rate),
            ))

    def test_typed_timestamps_selection_ties_and_duplicates(self):
        selector = qs.SnapshotSelector(qs.DateTime.parse("2025-01-15T15:00:00+02:00"))
        self.assertIsInstance(selector.timestamp, qs.DateTime)
        self.assertEqual(selector.timestamp, qs.DateTime(2025, 1, 15, 13))
        self.assertEqual(selector.rule, qs.SnapshotSelectionRule.Previous)
        self.assertEqual(self.store.select(selector).timestamp, qs.DateTime(2025, 1, 15, 10))
        closest = qs.SnapshotSelector(selector.timestamp, rule=qs.SnapshotSelectionRule.Closest)
        self.assertEqual(self.store.select(closest).timestamp, qs.DateTime(2025, 1, 15, 14))
        tie = qs.SnapshotSelector(qs.DateTime(2025, 1, 15, 12), rule="Closest")
        self.assertEqual(self.store.select(tie).timestamp, self.store.timestamps[0])
        self.assertTrue(all(isinstance(instant, qs.DateTime) for instant in self.store.timestamps))
        for selection in [qs.SnapshotSelector(selector.timestamp, rule="Exact"), qs.SnapshotSelector(qs.DateTime(2025, 1, 14))]:
            with self.assertRaises(qs.QuantSupportError):
                self.store.select(selection)
        with self.assertRaises(qs.QuantSupportError):
            self.store.insert(qs.MarketSnapshot.from_context(
                "2025-01-15T12:00:00+02:00", market(15, 0.99),
            ))
        self.assertAlmostEqual(self.store.select(selector).quotes.value(IDENTIFIER), 0.04)

    def test_json_and_dictionary_roundtrip(self):
        with tempfile.TemporaryDirectory() as directory:
            path = os.path.join(directory, "markets.json")
            self.store.to_json(path)
            restored = qs.MarketSnapshotStore.from_dict(json.loads(Path(path).read_text(encoding="utf-8")))
        self.assertEqual(restored.timestamps, self.store.timestamps)
        payload = self.store.to_dict()
        self.assertIsInstance(payload, dict)
        self.assertEqual(dict(self.store), payload)
        self.assertEqual(qs.MarketSnapshotStore.from_dict(payload).to_dict(), payload)
        self.assertEqual(qs.MarketSnapshotStore.from_dict(payload["snapshots"]).to_dict(), payload)
        payload["snapshots"][0]["timestamp"] = self.store.timestamps[0]
        snapshot = qs.MarketSnapshot.from_dict(payload["snapshots"][0])
        self.assertEqual(snapshot.timestamp, self.store.timestamps[0])
        with self.assertRaises(qs.QuantSupportError):
            qs.MarketSnapshotStore.from_dict([snapshot.to_dict(), snapshot.to_dict()])

    def test_sides_dates_fixings_and_cache_independence(self):
        discounts = []
        fixings = qs.FixingStore()
        fixings.add(index="SOFR", date="2025-01-14", value=0.03)
        for level in [qs.Level.Bid, qs.Level.Mid, qs.Level.Ask]:
            selector = qs.SnapshotSelector(qs.DateTime(2025, 1, 15, 10), rule="Exact", level=level)
            with self.store.pricing_context(selector, fixings=fixings) as context:
                self.assertEqual(context.quote_level, level)
                self.assertEqual(context.evaluation_date, qs.Date(2025, 1, 15))
                self.assertIsInstance(context.snapshot_timestamp, qs.DateTime)
                self.assertEqual(context.snapshot_timestamp, selector.timestamp)
                self.assertAlmostEqual(context.fixings.fixing(index="SOFR", date="2025-01-14"), 0.03)
                discounts.append(context.curve("SOFR").discount_factor("2025-07-15"))
        self.assertGreater(discounts[0], discounts[1])
        self.assertGreater(discounts[1], discounts[2])
        selector = qs.SnapshotSelector(qs.DateTime(2025, 1, 16, 10))
        projected = self.store.pricing_context(selector, anchor_fixings=False)
        self.assertFalse(projected.anchor_fixings)
        self.assertEqual(projected.snapshot_timestamp, self.store.select(selector).timestamp)
        with self.store.pricing_context(qs.SnapshotSelector(qs.DateTime(2025, 1, 16, 10))) as context:
            self.assertEqual(context.evaluation_date, qs.Date(2025, 1, 16))
            self.assertLess(context.curve("SOFR").discount_factor("2025-07-15"), discounts[-1])
            context.quote_level = "Bid"
            self.assertIsNone(context.snapshot_timestamp)

    def test_missing_side_does_not_fall_back_to_mid(self):
        store = qs.MarketSnapshotStore()
        store.insert(qs.MarketSnapshot.from_context(qs.DateTime(2025, 1, 15, 10), market(15, 0.04, only_mid=True)))
        selector = qs.SnapshotSelector(qs.DateTime(2025, 1, 15, 10), level="Ask")
        with store.pricing_context(selector) as context:
            with self.assertRaisesRegex(qs.QuantSupportError, "ask"):
                context.curve("SOFR")

    def test_zero_rate_quotes_roundtrip(self):
        context = qs.PricingContext(
            quotes=qs.QuoteStore.from_dict({"reference_date": "2025-01-15", "quotes": [
                {"identifier": f"ZeroRate_USD_SOFR_{pillar}", "mid": rate}
                for pillar, rate in [("0D", 0.02), ("1Y", 0.03)]
            ]}),
            curves=[qs.CurveConfiguration(
                market_index="SOFR", quotes=["ZeroRate_USD_SOFR_0D", "ZeroRate_USD_SOFR_1Y"],
            )],
        )
        snapshot = qs.MarketSnapshot.from_context(qs.DateTime(2025, 1, 15, 10), context)
        store = qs.MarketSnapshotStore.from_dict([snapshot.to_dict()])
        with context, store.pricing_context(qs.SnapshotSelector(snapshot.timestamp)) as historical:
            self.assertAlmostEqual(
                context.curve("SOFR").discount_factor("2025-07-15"),
                historical.curve("SOFR").discount_factor("2025-07-15"),
            )

    def test_trade_valuation_uses_selected_historical_market(self):
        position = qs.Trade(qs.Swap(
            "historical-swap", "2025-01-17", "2026-01-17", 1000.0, 0.05, "USD", "SOFR",
        ), "2025-01-15")
        prices = []
        for day in [15, 16]:
            selector = qs.SnapshotSelector(qs.DateTime(2025, 1, day, 10), rule="Exact")
            with self.store.pricing_context(selector) as context:
                result = context.evaluate(position, [qs.Request.Value])
                self.assertEqual(result.evaluation_date, qs.Date(2025, 1, day))
                prices.append(result.price)
        self.assertNotAlmostEqual(prices[0], prices[1])


if __name__ == "__main__":
    unittest.main()
