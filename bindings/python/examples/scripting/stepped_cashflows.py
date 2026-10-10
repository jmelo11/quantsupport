"""Build dated script events for the tutorial's six-year semiannual note.

This intentionally narrow translator keeps string assembly out of the bond
valuation narrative. It models the non-callable underlying only. It is not a
general schedule engine or an issuer-exercise algorithm.
"""

import quantsupport as qs


def underlying_script(bond):
    """Translate the tutorial contract's coupons and amortization to events.

The note uses unadjusted semiannual dates, Actual/365 fixed accruals, and
SOFR's Actual/360 floating accruals. Rates reset at accrual start. Any issuer
calls on the supplied bond are deliberately omitted from the returned script.
"""
    terms = bond.to_dict()
    start = qs.Date.parse(terms["start_date"])
    if (str(start + "6Y") != terms["maturity_date"]
            or terms["payment_frequency"] != "Semiannual"
            or terms["coupon_day_counter"] != "Actual365"
            or terms["currency"] != "USD"
            or terms["side"] != "LongReceive"):
        raise ValueError("This translator only supports the tutorial's six-year semiannual USD note")

    dates = [start + qs.Period(6 * period, qs.TimeUnit.Months) for period in range(13)]
    coupons = terms["coupons"]
    percentages = terms["notional_percentages"]
    events = []
    for period, (accrual_start, end) in enumerate(zip(dates, dates[1:])):
        coupon = coupons[min(period, len(coupons) - 1)]
        outstanding = percentages[min(period, len(percentages) - 1)]
        next_outstanding = percentages[min(period + 1, len(percentages) - 1)] if period < 11 else 0.0
        principal = terms["notional"] * (outstanding - next_outstanding)
        if "Fixed" in coupon:
            rate = str(coupon["Fixed"]["rate"])
            day_counter = "Actual365"
        else:
            floating = coupon["Floating"]
            if floating["index"] != "SOFR" or floating["cap"] is None or floating["floor"] is None:
                raise ValueError("The tutorial floating coupon requires SOFR with a cap and floor")
            rate = (
                f'max(min(RateIndex("SOFR", "{accrual_start}", "{end}") '
                f'+ {floating["spread"]}, {floating["cap"]}), {floating["floor"]})'
            )
            day_counter = "Actual360"
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
