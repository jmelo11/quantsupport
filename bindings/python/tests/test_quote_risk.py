"""Native position risk must agree with rebuilding under quote bumps."""

import json
from pathlib import Path
import unittest

import quantsupport as qs


class QuoteRiskTests(unittest.TestCase):
    def test_contract_terms_do_not_alias_market_pillars_on_a_fresh_tape(self):
        data = Path(__file__).resolve(
        ).parents[3] / "examples" / "hullwhite" / "data"
        quotes = qs.QuoteStore.from_dict(json.loads(Path(data / "quotes.json").read_text(encoding="utf-8")))
        curves = qs.CurveConfiguration.from_dict_list(json.loads(Path(data / "curve_specs.json").read_text(encoding="utf-8")))
        csa_policy = qs.SingleCurveCSADiscountPolicy(
            qs.Currency.USD, qs.MarketIndex.SOFR)
        ref = quotes.reference_date
        trades = [
            qs.Trade(qs.Swap(identifier="RISK-SWAP", start_date=ref, maturity_date=ref + "5Y",
                             notional=1_000_000, fixed_rate=0.04, currency=qs.Currency.USD,
                             market_index=qs.MarketIndex.SOFR), ref, qs.Side.PayShort),
            qs.Trade(qs.FixedRateBond(identifier="RISK-BOND", start_date=ref, maturity_date=ref + "5Y",
                                      notional=500_000, rate=0.045, currency=qs.Currency.USD,
                                      payment_frequency=qs.Frequency.Semiannual), ref,
                     pricing_anchor=qs.BondPricingAnchor.z_spread(0.0)),
        ]
        # The overnight pillar exposed the stale-node alias; the 5Y pillar
        # also verifies an economically material derivative independently.
        labels = ["FixedRateDeposit_USD_SOFR_1D", "OIS_USD_SOFR_5Y"]

        def context():
            return qs.PricingContext(quotes=quotes, curves=curves,
                                     csa_policy=csa_policy)

        def value(trade, label, bump):
            with context() as ctx, qs.Scenario(shocks=[qs.Shock(target=label, value=bump, shock_type=qs.ScenarioType.Absolute)]):
                return ctx.evaluate(trade, [qs.Request.Value]).price

        # Either trade may be the first valuation after __enter__, and a
        # portfolio must also work when both are priced in the same context.
        for order in [trades, list(reversed(trades))]:
            with context() as ctx:
                risks = {
                    trade.instrument.identifier: ctx.evaluate(
                        trade, [qs.Request.Value, qs.Request.Sensitivities])
                    .sensitivities.set_index("pillar")["value"]
                    for trade in order
                }
            for trade in order:
                for label in labels:
                    with self.subTest(first=order[0].instrument.identifier, trade=trade.instrument.identifier, pillar=label):
                        bump = 1e-5
                        finite_difference = (
                            value(trade, label, bump) - value(trade, label, -bump)) / (2 * bump)
                        self.assertAlmostEqual(
                            risks[trade.instrument.identifier][label], finite_difference,
                            delta=max(0.1, abs(finite_difference) * 1e-5),
                        )


if __name__ == "__main__":
    unittest.main()
