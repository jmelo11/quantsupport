"""Explicit, exhaustively checked topic navigation over public binding symbols."""

from collections import defaultdict
from pathlib import Path

from sphinx.errors import ExtensionError

from python_api import declarations


PYTHON_EXCEPTIONS = "QuantSupportError QuantSupportTypeError QuantSupportValueError QuantSupportStateError QuantSupportIOError QuantSupportNotImplementedError".split()


PYTHON_SECTIONS = {
    "time": {
        "Dates and Periods": "Date DateTime Period TimeUnit",
        "Calendars": "Calendar UnitedStatesMarket BrazilMarket AustraliaMarket CanadaMarket NewZealandMarket SouthKoreaMarket ChinaMarket JointCalendarRule",
        "Schedules and IMM": "Schedule IMM DateGenerationRule BusinessDayConvention",
        "Accrual and Frequency": "DayCounter Compounding Frequency",
    },
    "market": {
        "Currencies and Indices": "Currency MarketIndex",
        "Quotes, Fixings, and FX": "QuoteStore FixingStore FxStore Level",
        "Historical Snapshots": "SnapshotSelectionRule SnapshotSelector MarketSnapshot MarketSnapshotStore",
        "Curve Construction": "CurveConfiguration CreditCurveConfiguration Interpolator",
        "Yield Curves": "DiscountCurve ZeroRateTermStructure TenorBasedZeroRateTermStructure",
        "Volatility": "VolatilitySurface VolatilityCube VolatilitySurfaceConfiguration VolatilityCubeConfiguration VolatilityType SmileType",
        "Simulation": "Simulation SimulationConfiguration",
    },
    "instruments": {
        "Base Types and Direction": "Instrument Side",
        "Interest Rates": "InterestRate Swap BasisSwap CrossCurrencySwap FixFloatCrossCurrencySwap RateFutures",
        "Rate Options": "EuropeanSwaption SwaptionType CapFloor CapFloorType CapletFloorlet CapletFloorletType",
        "Fixed Income": "FixedRateBond FloatingRateNote FixedRateDeposit SteppedCouponBond CallScheduleEntry PaymentStructure",
        "Loans and Prepayment": "Loan LoanRepaymentStructure PrepaymentModelConfiguration",
        "Coupons and Payoffs": "FixedRateCoupon FloatingRateCoupon OptionEmbeddedCoupon PayoffOps",
        "Foreign Exchange": "FxForward FxOption",
        "Equity and Credit": "EquityForward EquityOption OptionType CreditDefaultSwap",
        "Scripting": "ScriptedProduct",
    },
    "pricing": {
        "Positions and Requests": "Trade Portfolio Request BondPricingAnchor",
        "Contexts and Results": "PricingContext EvaluationResults",
        "Pricers and Policies": "LoanPricer PricerConfiguration SingleCurveCSADiscountPolicy FixedIncomeDiscountPolicy",
        "Python Extension Interfaces": "Pricer MarketDataProvider Evaluator",
    },
    "risk": {
        "Scenarios and Shocks": "Scenario Shock ScenarioType",
        "Collateral and Netting": "CsaTerms NettingSet",
        "Exposure and XVA": "ExposureProfile XvaConfig XvaResult",
    },
}

DOMAIN_TITLES = {
    "time": "Time", "market": "Market Data", "instruments": "Instruments",
    "pricing": "Pricing", "risk": "Risk and XVA",
}


def slug(title):
    return title.lower().replace(",", "").replace(" ", "-")


def python_sections(module):
    domain = module.rsplit(".", 1)[-1]
    return [(title, f"/python/sections/{domain}/{slug(title)}")
            for title in PYTHON_SECTIONS.get(domain, {})]


def generate_python_navigation(app):
    public = {name for name in declarations() if "._" not in name}
    assigned = [f"quantsupport.{name}" for name in PYTHON_EXCEPTIONS]
    for domain, sections in PYTHON_SECTIONS.items():
        output = Path(app.srcdir) / "python/sections" / domain
        output.mkdir(parents=True, exist_ok=True)
        for title, names in sections.items():
            classes = [f"quantsupport.{domain}.{name}" for name in names.split()]
            assigned.extend(classes)
            lines = [title, "=" * len(title), "", ".. toctree::", "   :maxdepth: 1", ""]
            lines.extend(f"   {name.rsplit('.', 1)[-1]} </autoapi/{name.replace('.', '/')}>"
                         for name in classes)
            lines.extend(["", ".. autoapisummary::", ""])
            lines.extend(f"   {name}" for name in classes)
            if domain == "time" and title == "Calendars":
                lines.extend(["", "Native Calendar Variants", "------------------------", "",
                              "All native calendar variants are available through :py:class:`quantsupport.time.Calendar`.",
                              "Country calendars are values of this class, not separate Python classes.", "",
                              ".. list-table::", "   :header-rows: 1", "", "   * - Rust calendar", "     - Python declaration"])
                for native, expression in (
                    ("NullCalendar", "Calendar.NullCalendar"), ("WeekendsOnly", "Calendar.WeekendsOnly"),
                    ("TARGET", "Calendar.TARGET"), ("UnitedKingdom", "Calendar.UnitedKingdom"),
                    ("Japan", "Calendar.Japan"), ("Chile", "Calendar.Chile"),
                    ("UnitedStates", "Calendar.UnitedStates(market=UnitedStatesMarket.Sofr)"),
                    ("Brazil", "Calendar.Brazil(market=BrazilMarket.Settlement)"),
                    ("Australia", "Calendar.Australia(market=AustraliaMarket.Settlement)"),
                    ("Canada", "Calendar.Canada(market=CanadaMarket.Settlement)"),
                    ("NewZealand", "Calendar.NewZealand(market=NewZealandMarket.Wellington)"),
                    ("SouthKorea", "Calendar.SouthKorea(market=SouthKoreaMarket.Settlement)"),
                    ("China", "Calendar.China(market=ChinaMarket.IB)"),
                    ("Switzerland", "Calendar.Switzerland"),
                    ("Norway", "Calendar.Norway"),
                    ("Sweden", "Calendar.Sweden"),
                    ("Denmark", "Calendar.Denmark"),
                    ("SouthAfrica", "Calendar.SouthAfrica"),
                    ("JointCalendar", "Calendar.JointCalendar(calendars=[Calendar.TARGET, Calendar.Japan])"),
                ):
                    lines.extend([f"   * - {native}", f"     - ``{expression}``"])
                lines.extend(["", "Calendar Operations", "-------------------", "",
                              "* Business-day queries: :py:meth:`~quantsupport.time.Calendar.is_business_day`, :py:meth:`~quantsupport.time.Calendar.is_holiday`, and :py:meth:`~quantsupport.time.Calendar.is_weekend`.",
                              "* Date adjustment: :py:meth:`~quantsupport.time.Calendar.adjust` and :py:meth:`~quantsupport.time.Calendar.advance`.",
                              "* Business month-end: :py:meth:`~quantsupport.time.Calendar.end_of_month` and :py:meth:`~quantsupport.time.Calendar.is_end_of_month`.",
                              "* Date ranges: :py:meth:`~quantsupport.time.Calendar.business_days_between`, :py:meth:`~quantsupport.time.Calendar.holiday_list`, and :py:meth:`~quantsupport.time.Calendar.business_day_list`.",
                              "* Independent overrides: :py:meth:`~quantsupport.time.Calendar.with_added_holiday` and :py:meth:`~quantsupport.time.Calendar.with_removed_holiday` return new values; they do not mutate shared calendar attributes.", ""])
            (output / f"{slug(title)}.rst").write_text("\n".join(lines) + "\n", encoding="utf-8")
    if len(assigned) != len(set(assigned)) or set(assigned) != public:
        raise ExtensionError(f"Python navigation must cover each public class once; missing={public - set(assigned)}, unknown={set(assigned) - public}")


def excel_section(name):
    if name == "QS.LOAN.SETTINGS":
        return "Instruments", "Fixed Income"
    if name == "QS.LOAN.PRICER" or name.startswith("QS.HISTORY."):
        return "Pricing", "Valuation and Risk"
    if name == "QS.INTERPOLATE" or name.startswith("QS.INTERPOLATE."):
        return "Math", "Interpolation"
    if name.startswith("QS.CALENDAR."):
        return "Time", "Calendars"
    if name.startswith("QS.SCHEDULE"):
        return "Time", "Schedules"
    if name.startswith("QS.DATETIME.") or name in ("QS.DATE.ADVANCE", "QS.YEAR.FRACTION"):
        return "Time", "Dates and Accrual"
    if name == "QS.TRADE.CREATE":
        return "Instruments", "Positions"
    if name.startswith(("QS.TRADE.", "QS.INSTRUMENT.")):
        product = name.split(".", 2)[2]
        if product in ("FIXED.RATE.BOND", "FLOATING.RATE.NOTE", "FIXED.RATE.DEPOSIT", "LOAN"):
            return "Instruments", "Fixed Income"
        if product.startswith("FX."):
            return "Instruments", "Foreign Exchange"
        if product.startswith("EQUITY."):
            return "Instruments", "Equity"
        if product == "CDS":
            return "Instruments", "Credit"
        return "Instruments", "Interest Rates"
    if name in ("QS.SENSITIVITIES", "QS.BOND.SENSITIVITIES"):
        return "Risk", "Sensitivities"
    if name.startswith("QS.DISCOUNT.POLICY."):
        return "Pricing", "Discount Policies"
    if name == "QS.EVALUATE" or name.startswith(("QS.EVALUATE.", "QS.RESULT.", "QS.REQUEST.", "QS.PORTFOLIO.")):
        return "Pricing", "Valuation and Risk"
    if name.startswith("QS.BOND."):
        return "Pricing", "Bond Analytics"
    if name in ("QS.PRICE", "QS.FAIR.RATE", "QS.CASHFLOWS", "QS.SENSITIVITIES"):
        return "Pricing", "Valuation and Risk"
    if name.startswith("QS.CONTEXT."):
        return "Pricing", "Contexts and Constructed Objects"
    if name.startswith(("QS.SNAPSHOT.", "QS.SNAPSHOTS.")):
        return "Market Data", "Historical Snapshots"
    if name.startswith(("QS.QUOTES.", "QS.FIXINGS.", "QS.FX.")):
        return "Market Data", "Quotes, Fixings, and FX"
    if name.startswith(("QS.CURVE.", "QS.CREDIT.CURVE.", "QS.ELEMENTS.")):
        return "Market Data", "Curves and Construction"
    if name.startswith("QS.VOL."):
        return "Market Data", "Volatility"
    if name.startswith("QS.SIMULATION."):
        return "Market Data", "Simulation"
    if name.startswith("QS.PRICER."):
        return "Pricing", "Pricer Configuration"
    if name.startswith("QS.SCENARIOS."):
        return "Risk", "Scenarios"
    if name.startswith("QS.OBJECT."):
        return "System", "Object Lifecycle"
    if name.startswith("QS.ENGINE."):
        return "System", "Background Pricing"
    if name in ("QS.FUNCTIONS", "QS.FUNCTIONS.DETAILS", "QS.HELP", "QS.VERSION"):
        return "System", "Catalog and Help"
    raise ExtensionError(f"Uncategorized worksheet function: {name}")


def write_excel_navigation(output, records):
    groups = defaultdict(lambda: defaultdict(list))
    for record in records:
        domain, section = excel_section(record["name"])
        groups[domain][section].append(record["name"])
    index = ["Function Categories", "===================", "", ".. toctree::", "   :maxdepth: 2", ""]
    for domain, sections in sorted(groups.items()):
        domain_slug = f"section-{slug(domain)}"
        index.append(f"   {domain} <{domain_slug}>")
        lines = [domain, "=" * len(domain), "", ".. toctree::", "   :maxdepth: 1", ""]
        for title, functions in sorted(sections.items()):
            section_slug = f"{domain_slug}-{slug(title)}"
            lines.append(f"   {title} <{section_slug}>")
            content = [title, "=" * len(title), "", ".. toctree::", "   :maxdepth: 1", ""]
            content.extend(f"   {name} <{name.lower().replace('.', '-')}>" for name in sorted(functions))
            (output / f"{section_slug}.rst").write_text("\n".join(content) + "\n", encoding="utf-8")
        (output / f"{domain_slug}.rst").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (output / "index.rst").write_text("\n".join(index) + "\n", encoding="utf-8")
