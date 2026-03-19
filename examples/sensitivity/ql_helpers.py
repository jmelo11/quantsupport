import QuantLib as ql


def make_schedule(start, end, frequency, calendar):
    """Schedule matching quantsupport: NullCalendar, Unadjusted, Backward."""
    return ql.Schedule(
        start, end,
        frequency,
        calendar,
        ql.Unadjusted,
        ql.Unadjusted,
        ql.DateGeneration.Backward,
        False,
    )


def make_vanilla_swap(start, maturity, fixed_rate, notional_val,
                      forecast_handle, discount_handle, calendar, day_count,
                      ibor_name="SimpleSOFR", ccy=None):
    """
    Build a VanillaSwap (Receiver = receive fixed, pay float).
    Uses simple forward rates from the forecast curve (NOT daily-compounded OIS).
    """
    if ccy is None:
        ccy = ql.USDCurrency()
    fixed_schedule = make_schedule(
        start, maturity, ql.Period(ql.Semiannual), calendar)
    float_schedule = make_schedule(
        start, maturity, ql.Period(ql.Quarterly), calendar)

    ibor_index = ql.IborIndex(
        ibor_name, ql.Period(3, ql.Months),
        0, ccy, calendar, ql.Unadjusted, False, day_count,
        forecast_handle,
    )

    swap = ql.VanillaSwap(
        ql.VanillaSwap.Receiver,
        notional_val,
        fixed_schedule,
        fixed_rate,
        day_count,
        float_schedule,
        ibor_index,
        0.0,
        day_count,
    )

    engine = ql.DiscountingSwapEngine(discount_handle)
    swap.setPricingEngine(engine)
    return swap


def compute_dv01(swap, handles_to_bump, fx_divisor=1.0):
    """Central-difference DV01: bump each pillar +/-1bp."""
    bump = 1e-4
    base = swap.NPV() / fx_divisor
    results = {}
    for name, sq in handles_to_bump.items():
        orig = sq.value()
        sq.setValue(orig + bump)
        up = swap.NPV() / fx_divisor
        sq.setValue(orig - bump)
        dn = swap.NPV() / fx_divisor
        sq.setValue(orig)
        results[name] = (up - dn) / 2.0
    return base, results
