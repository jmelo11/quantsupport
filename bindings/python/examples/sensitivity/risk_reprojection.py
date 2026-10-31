"""Compare deposit, OIS and post-bootstrap zero-rate risk for one EUR swap.

Run with a binding built from this checkout:
    python bindings/python/examples/sensitivity/risk_reprojection.py
"""

import quantsupport as qs


def main():
    reference = qs.Date(2026, 1, 15)
    observations = [
        {"identifier": "FixedRateDeposit_EUR_ESTR_1Y", "mid": 0.03},
        {"identifier": "FixedRateDeposit_EUR_ESTR_2Y", "mid": 0.04},
    ]
    quotes = qs.QuoteStore.from_dict({
        "reference_date": str(reference), "quotes": observations,
    })
    curves = [qs.CurveConfiguration(
        "ESTR", [record["identifier"] for record in observations],
        day_counter="Actual365", interpolator="LogLinear", enable_extrapolation=True,
    )]
    context = qs.PricingContext(
        quotes, curves, csa_policy=qs.SingleCurveCSADiscountPolicy("EUR", "ESTR"),
    )
    swap = qs.Swap(
        identifier="EUR-ESTR-18M", start_date=reference + "2D",
        maturity_date=reference + "18M", notional=1_000_000,
        fixed_rate=0.035, currency="EUR", market_index="ESTR",
    )
    trade = qs.Trade(swap, reference, qs.Side.LongReceive)
    requests = [qs.Request.Value, qs.Request.Sensitivities]

    with context:
        native = context.evaluate(trade, requests)

        # The reporting OIS contracts do not need observations in the quote store.
        projected = context.reproject_risk(["OIS_EUR_ESTR_1Y", "OIS_EUR_ESTR_2Y"])

        # Omit dates to use the bootstrapped curve's future nodes. To choose a
        # grid, pass dates=[reference + "1Y", reference + "2Y"] to ZeroRateRisk.
        zeros = context.with_zero_rate_risk([qs.ZeroRateRisk(
            "ESTR", day_counter="Actual365", compounding="Continuous", frequency="Annual",
        )])

    report("Original deposit quote risk", native)

    # Each view owns its market snapshot and outlives the original context scope.
    for label, view in [("Reprojected OIS quote risk", projected),
                        ("Post-bootstrap zero-rate risk", zeros)]:
        with view:
            print(f"\n{label}: implied reporting quotes")
            for pillar, value in zip(view.target_labels, view.target_values):
                print(f"  {pillar}: {100 * value:.6f}%")
            result = view.evaluate(trade, requests)
            assert result.price == native.price
            assert set(result.sensitivities["pillar"]) == set(view.target_labels)
            report(label, result)

    print("\nVerified: all three risk bases produce exactly the same NPV.")


def report(label, result):
    print(f"\n{label}: NPV = {result.price:.6f} EUR")
    table = result.sensitivities.copy()
    # All three bases here use annual decimal rates: +1 bp means +0.0001.
    table["+1 bp P&L (EUR)"] = table["value"] * 1e-4
    print(table.to_string(index=False, float_format=lambda value: f"{value:.6f}"))


if __name__ == "__main__":
    main()
