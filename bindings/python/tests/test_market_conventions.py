"""New conventions survive public exports and historical snapshot selection."""
import ast
from pathlib import Path
import tempfile
import unittest

import quantsupport as qs
from quantsupport import time

NEW_INDICES = (
    "FedFunds", "CDI", "DESTR", "KOFR", "ZARONIA",
    "BBSW1m", "BBSW2m", "BBSW3m", "BBSW4m", "BBSW6m",
    "BKBM1m", "BKBM2m", "BKBM3m", "BKBM4m", "BKBM6m",
    "NIBOR1m", "NIBOR3m", "NIBOR6m", "NIBOR12m",
    "SHIBOR1d", "SHIBOR1w", "SHIBOR2w", "SHIBOR1m", "SHIBOR3m",
    "SHIBOR6m", "SHIBOR12m", "EURIBOR1w",
)
MARKETS = {
    "Australia": ("Settlement", "ASX"), "Canada": ("Settlement", "TSX"),
    "NewZealand": ("Wellington", "Auckland"), "SouthKorea": ("Settlement", "KRX"),
    "China": ("SSE", "IB"),
}


class MarketConventionTests(unittest.TestCase):
    def test_public_calendars_market_enums_and_stub_declarations(self):
        for country, variants in MARKETS.items():
            enum_name = f"{country}Market"
            enum = getattr(qs, enum_name)
            self.assertIs(enum, getattr(time, enum_name))
            self.assertIn(enum_name, qs.__all__)
            self.assertIn(enum_name, time.__all__)
            for variant in variants:
                calendar = getattr(qs.Calendar, country)(market=getattr(enum, variant))
                canonical = f"{country}({variant})"
                self.assertEqual(calendar.name(), canonical)
                self.assertEqual(qs.Calendar(canonical).name(), canonical)
                self.assertEqual(str(enum.parse(variant.lower())), variant)
        for country in ("Switzerland", "Norway", "Sweden", "Denmark", "SouthAfrica"):
            self.assertEqual(getattr(qs.Calendar, country).name(), country)
        stubs = Path(__file__).resolve().parents[1] / "python/quantsupport"
        declarations = ast.parse((stubs / "time/__init__.pyi").read_text())
        classes = {node.name for node in declarations.body if isinstance(node, ast.ClassDef)}
        self.assertTrue({f"{country}Market" for country in MARKETS} <= classes)
        declarations = ast.parse((stubs / "market/__init__.pyi").read_text())
        index = next(node for node in declarations.body if isinstance(node, ast.ClassDef) and node.name == "MarketIndex")
        constants = {node.target.id for node in index.body if isinstance(node, ast.AnnAssign)}
        self.assertTrue(set(NEW_INDICES) <= constants)
        for name in NEW_INDICES:
            self.assertEqual(qs.MarketIndex.parse(name.lower()), getattr(qs.MarketIndex, name))

    def test_holidays_working_weekends_and_schedule_adjustment(self):
        self.assertFalse(qs.Calendar.Canada().is_business_day("2026-09-30"))
        self.assertTrue(qs.Calendar.Canada(market=qs.CanadaMarket.TSX).is_business_day("2026-09-30"))
        self.assertFalse(qs.Calendar.China().is_business_day("2026-02-14"))
        calendar = qs.Calendar.China(market=qs.ChinaMarket.IB)
        self.assertTrue(calendar.is_business_day("2026-02-14"))
        self.assertFalse(calendar.with_added_holiday("2026-02-14").is_business_day("2026-02-14"))
        self.assertTrue(calendar.is_business_day("2026-02-14"))
        self.assertEqual(qs.Calendar.Sweden.adjust("2026-06-19"), qs.Date(2026, 6, 22))

    def test_snapshot_select_restores_every_added_index(self):
        store = qs.MarketSnapshotStore()
        for day, rate in ((15, 0.03), (16, 0.06)):
            context = qs.PricingContext(
                quotes=qs.QuoteStore.from_dict({"reference_date": f"2026-01-{day}", "quotes": [
                    {"identifier": f"ZeroRate_USD_{name}_{tenor}", "mid": rate}
                    for name in NEW_INDICES for tenor in ["0D", "2Y"]
                ]}),
                curves=[qs.CurveConfiguration(
                    market_index=getattr(qs.MarketIndex, name),
                    quotes=[f"ZeroRate_USD_{name}_{tenor}" for tenor in ["0D", "2Y"]],
                ) for name in NEW_INDICES],
            )
            store.insert(qs.MarketSnapshot.from_context(qs.DateTime(2026, 1, day, 10), context))
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / "markets.json")
            store.to_json(path)
            restored = qs.MarketSnapshotStore.from_json(path)
        discounts = []
        for day in (15, 16):
            selector = qs.SnapshotSelector(qs.DateTime(2026, 1, day, 10), rule=qs.SnapshotSelectionRule.Exact)
            selected = restored.select(selector)
            self.assertEqual(selected.timestamp, selector.timestamp)
            self.assertEqual({config["market_index"] for config in selected.to_dict()["curves"]}, set(NEW_INDICES))
            with restored.pricing_context(selector) as historical:
                values = [historical.curve(getattr(qs.MarketIndex, name)).discount_factor("2027-01-15") for name in NEW_INDICES]
                self.assertTrue(all(0 < value < 1 for value in values))
                self.assertEqual(historical.evaluation_date, qs.Date(2026, 1, day))
                discounts.append(values)
        self.assertTrue(all(later < earlier for earlier, later in zip(*discounts)))

    def test_snapshot_preserves_calendar_conventions_and_bootstraps(self):
        fields = [""] * 23
        fields[5] = "JointCalendar(JoinHolidays,Canada(Settlement),Switzerland)"
        fields[8] = "Canada(Settlement)"
        identifier = "OIS_CAD_CORRA_1Y_" + "_".join(fields)
        context = qs.PricingContext(
            quotes=qs.QuoteStore.from_dict({
                "reference_date": "2026-01-15",
                "quotes": [{"identifier": identifier, "mid": 0.03}],
            }),
            curves=[qs.CurveConfiguration(market_index=qs.MarketIndex.CORRA, quotes=[identifier])],
            csa_policy=qs.SingleCurveCSADiscountPolicy(
                collateral_currency=qs.Currency.CAD, collateral_index=qs.MarketIndex.CORRA,
            ),
        )
        snapshot = qs.MarketSnapshot.from_context(qs.DateTime(2026, 1, 15, 10), context)
        archive = qs.MarketSnapshotStore.from_dict([snapshot.to_dict()])
        selector = qs.SnapshotSelector(snapshot.timestamp)
        self.assertEqual(archive.select(selector).to_dict()["curves"][0]["quotes"], [identifier])
        with archive.pricing_context(selector) as historical:
            self.assertTrue(0 < historical.curve(qs.MarketIndex.CORRA).discount_factor("2027-01-15") < 1)


if __name__ == "__main__":
    unittest.main()
