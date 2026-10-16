"""Load the loan example's shared Rust/Python market inputs."""

from dataclasses import dataclass
from pathlib import Path

import quantsupport as qs


@dataclass
class LoanMarketInputs:
    quotes: qs.QuoteStore
    curves: list[qs.CurveConfiguration]
    credit_curves: list[qs.CreditCurveConfiguration]
    csa_policy: qs.SingleCurveCSADiscountPolicy


def load_loan_market() -> LoanMarketInputs:
    """Read illustrative EUR inputs; market construction stays in the notebook."""
    repository = Path(__file__).resolve().parents[5]
    directory = repository / "examples" / "valuation" / "loans" / "data"
    return LoanMarketInputs(
        quotes=qs.QuoteStore.from_json(path=str(directory / "loan_quotes.json")),
        curves=qs.CurveConfiguration.from_json(path=str(directory / "loan_curves.json")),
        credit_curves=qs.CreditCurveConfiguration.from_json(path=str(directory / "loan_credit_curves.json")),
        csa_policy=qs.SingleCurveCSADiscountPolicy(
            collateral_currency=qs.Currency.EUR, collateral_index=qs.MarketIndex.ESTR,
        ),
    )
