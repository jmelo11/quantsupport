"""File loading for the notebooks; pricing stays in the notebook cells.

Paths are relative to this module, never the shell's working directory. The
market is the repository's illustrative 2026-02-24 Hull-White snapshot, not a
live feed. This module is example code, not part of the installed binding.
"""

from dataclasses import dataclass
import json
from pathlib import Path

import quantsupport as qs


EXAMPLES = Path(__file__).resolve().parents[1]
MARKET_DATA = EXAMPLES.parents[2] / "examples" / "hullwhite" / "data"


@dataclass
class MarketInputs:
    """Unconstructed inputs; a PricingContext turns them into market objects."""

    quotes: qs.QuoteStore
    curves: list
    credit_curves: list
    volatility_surfaces: list
    csa_policy: qs.SingleCurveCSADiscountPolicy


def load_market():
    """Load shared USD/SOFR inputs and this example's illustrative CDS market."""
    quotes = json.loads(
        (MARKET_DATA / "quotes.json").read_text(encoding="utf-8"))
    credit_quotes = read_configuration("cva", "credit_quotes.json")
    if quotes["reference_date"] != credit_quotes["reference_date"]:
        raise ValueError("Rate and credit quotes must share a reference date")
    quotes["quotes"].extend(credit_quotes["quotes"])
    return MarketInputs(
        quotes=qs.QuoteStore.from_dict(quotes),
        curves=qs.CurveConfiguration.from_json(
            str(MARKET_DATA / "curve_specs.json")),
        credit_curves=qs.CreditCurveConfiguration.from_json(
            str(EXAMPLES / "cva" / "data" / "credit_curves.json")
        ),
        volatility_surfaces=qs.VolatilitySurfaceConfiguration.from_json(
            str(MARKET_DATA / "vol_specs.json")
        ),
        csa_policy=qs.SingleCurveCSADiscountPolicy(
            qs.Currency.USD, qs.MarketIndex.SOFR),
    )


def read_configuration(scope, filename):
    """Read an editable native configuration from one example scope."""
    return json.loads((EXAMPLES / scope / "data" / filename).read_text(encoding="utf-8"))


def load_pricers(filename):
    """Load the selected bond engine and its calibrated rate-model assignment."""
    return qs.PricerConfiguration.from_json(str(EXAMPLES / "valuation" / "data" / filename))
