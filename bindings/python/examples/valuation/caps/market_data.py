"""File loading for the notebooks; pricing stays in the notebook cells.

Paths are relative to this module, never the shell's working directory. The
markets are the repository's illustrative 2026-02-24 Hull-White and bootstrap
snapshots, not live feeds. This module is example code, not part of the installed
binding.
"""

from dataclasses import dataclass
from pathlib import Path

import quantsupport as qs


EXAMPLES = Path(__file__).resolve().parents[2]
MARKET_DATA = EXAMPLES.parents[2] / "examples" / "hullwhite" / "data"


@dataclass
class MarketInputs:
    """Unconstructed inputs; a PricingContext turns them into market objects."""

    quotes: qs.QuoteStore
    curves: list[qs.CurveConfiguration]
    volatility_surfaces: list[qs.VolatilitySurfaceConfiguration]
    csa_policy: qs.SingleCurveCSADiscountPolicy


def load_market():
    """Load fresh typed inputs from the shared USD/SOFR example data."""
    return MarketInputs(
        quotes=qs.QuoteStore.from_json(str(MARKET_DATA / "quotes.json")),
        curves=qs.CurveConfiguration.from_json(str(MARKET_DATA / "curve_specs.json")),
        volatility_surfaces=qs.VolatilitySurfaceConfiguration.from_json(
            str(MARKET_DATA / "vol_specs.json")
        ),
        csa_policy=qs.SingleCurveCSADiscountPolicy(qs.Currency.USD, qs.MarketIndex.SOFR),
    )
