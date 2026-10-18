"""Build dated script events for the tutorial's six-year semiannual note.

This intentionally narrow translator keeps string assembly out of the bond
valuation narrative. It models the non-callable underlying only. It is not a
general schedule engine or an issuer-exercise algorithm.
"""

import quantsupport as qs


def underlying_script(bond):
    """Translate the tutorial contract's coupons and amortization to events.

    The note uses unadjusted semiannual dates, Actual/365 fixed accruals, and
    SOFR's Actual/360 floating accruals. Rates reset at accrual start. Issuer
    calls on the supplied bond are deliberately omitted from the script.
    """
    terms = bond.to_dict()
    start = qs.Date.parse(terms["start_date"])
    if (
        str(start + "6Y") != terms["maturity_date"]
        or terms["payment_frequency"] != "Semiannual"
        or terms["coupon_day_counter"] != "Actual365"
        or terms["currency"] != "USD"
    ):
        raise ValueError(
            "This translator only supports the tutorial's six-year semiannual USD note"
        )

    dates = [
        start + qs.Period(6 * period, qs.TimeUnit.Months)
        for period in range(13)
    ]
    coupons = terms["coupons"]
    percentages = terms["notional_percentages"]
    events = []
    for period, (accrual_start, end) in enumerate(zip(dates, dates[1:])):
        coupon = coupons[min(period, len(coupons) - 1)]
        outstanding = percentages[min(period, len(percentages) - 1)]
        next_outstanding = (
            percentages[min(period + 1, len(percentages) - 1)]
            if period < len(dates) - 2 else 0.0
        )
        principal = terms["notional"] * (outstanding - next_outstanding)
        if "FixedRateCoupon" in coupon:
            rate = str(coupon["FixedRateCoupon"]["rate"]["rate"])
            day_counter = terms["coupon_day_counter"]
        elif "OptionEmbeddedCoupon" in coupon or "FloatingRateCoupon" in coupon:
            kind = (
                "OptionEmbeddedCoupon"
                if "OptionEmbeddedCoupon" in coupon else "FloatingRateCoupon"
            )
            floating = coupon[kind]
            if floating["index"] != "SOFR":
                raise ValueError("The tutorial floating coupon requires SOFR")
            observation = f'RateIndex("SOFR", "{accrual_start}", "{end}")'
            payoff = (
                _payoff_expression(floating["payoff"], observation)
                if kind == "OptionEmbeddedCoupon" else observation
            )
            rate = f'({payoff} + {floating["spread"]})'
            day_counter = floating["day_counter"]
            if day_counter != "Actual360":
                raise ValueError("The tutorial floating coupon requires Actual360")
        else:
            raise ValueError("Unsupported tutorial coupon definition")
        initialization = "bond = 0; " if period == 0 else ""
        events.append({
            "date": str(accrual_start),
            "code": (
                f'{initialization}accrual = cvg("{accrual_start}", "{end}", "{day_counter}"); '
                f'rate = {rate}; '
                f'bond pays {terms["notional"] * outstanding} * rate * accrual '
                f'+ {principal} on "{end}";'
            ),
        })
    return qs.ScriptedProduct.from_dict({
        "name": "STEPPED-UNDERLYING", "currency": qs.Currency.USD,
        "result_variable": "bond", "events": events,
    })


def _payoff_expression(payoff, observation):
    """Render native PayoffOps nodes in the library's script syntax."""
    if payoff == "Index":
        return observation
    if not isinstance(payoff, dict) or len(payoff) != 1:
        raise ValueError(f"Unsupported payoff expression: {payoff!r}")
    operation, operands = next(iter(payoff.items()))
    if operation == "Const":
        return repr(operands)
    if operation not in ("Min", "Max", "Plus", "Minus", "Times"):
        raise ValueError(f"Unsupported payoff operation: {operation}")
    left, right = (_payoff_expression(node, observation) for node in operands)
    if operation in ("Min", "Max"):
        return f"{operation.lower()}({left}, {right})"
    symbol = {"Plus": "+", "Minus": "-", "Times": "*"}[operation]
    return f"({left} {symbol} {right})"
