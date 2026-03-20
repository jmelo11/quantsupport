import QuantLib as ql
import math
from scipy.optimize import brentq


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


def build_collateral_curve(reference_date, calendar, day_count,
                           sofr_curve, fx_spot_val,
                           fx_fwd_specs, xccy_specs,
                           all_quote_handles):
    """
    Bootstrap the Collateral(CLP,USD) discount curve.

    Short end (FX forward points): df_clp = df_sofr × spot / (spot + pts).
    Long end (fixed-float xccy swaps): sequential 1-D root-finding.

    Parameters
    ----------
    fx_fwd_specs : list of (name, ql.Period)
    xccy_specs   : list of (name, ql.Period)
    all_quote_handles : dict  name → ql.SimpleQuote
    """
    spot = fx_spot_val

    dates = [reference_date]
    dfs = [1.0]

    # ── FX forward implied DFs ────────────────────────────────────
    for name, tenor in fx_fwd_specs:
        pts = all_quote_handles[name].value()
        date = calendar.advance(reference_date, tenor)
        df_sofr = sofr_curve.discount(date)
        df_clp = df_sofr * spot / (spot + pts)
        dates.append(date)
        dfs.append(df_clp)

    # ── XCcy swap pillars ─────────────────────────────────────────
    for name, tenor in xccy_specs:
        rate = all_quote_handles[name].value()
        maturity = calendar.advance(reference_date, tenor)
        start = reference_date

        fixed_sched = make_schedule(start, maturity,
                                    ql.Period(ql.Semiannual), calendar)
        float_sched = make_schedule(start, maturity,
                                    ql.Period(ql.Quarterly), calendar)

        # Pre-compute the USD floating leg PV (known, independent of df_clp)
        usd_pv = 0.0
        for i in range(len(float_sched) - 1):
            s, e = float_sched[i], float_sched[i + 1]
            yf = day_count.yearFraction(s, e)
            fwd = sofr_curve.forwardRate(s, e, day_count, ql.Simple).rate()
            usd_pv -= fwd * yf * sofr_curve.discount(e)
        # USD notional exchange: +1 at start, -1 at maturity (per unit)
        usd_pv += sofr_curve.discount(start) - sofr_curve.discount(maturity)

        def residual(log_df, _dates=dates, _dfs=dfs):
            df_T = math.exp(log_df)
            trial_dates = list(_dates) + [maturity]
            trial_dfs = list(_dfs) + [df_T]
            trial_curve = ql.DiscountCurve(trial_dates, trial_dfs, day_count)
            trial_curve.enableExtrapolation()

            # CLP fixed coupons (per-unit-USD, converted to USD)
            clp_pv = 0.0
            for i in range(len(fixed_sched) - 1):
                s, e = fixed_sched[i], fixed_sched[i + 1]
                yf = day_count.yearFraction(s, e)
                clp_pv += rate * yf * trial_curve.discount(e)
            # CLP notional exchange (per-unit-USD)
            clp_pv += -trial_curve.discount(start) + \
                trial_curve.discount(maturity)

            return clp_pv + usd_pv

        log_df_T = brentq(residual, math.log(0.01), math.log(2.0), xtol=1e-14)
        dates.append(maturity)
        dfs.append(math.exp(log_df_T))

    curve = ql.DiscountCurve(dates, dfs, day_count)
    curve.enableExtrapolation()
    return curve


def compute_xccy_npv(sofr_curve, icp_curve, collateral_curve,
                     schedule_q, clp_notional, usd_notional, spread,
                     fx_spot_val, day_count, start_date, maturity):
    """
    Float-float xccy swap NPV:  receive CLP ICP+spread, pay USD SOFR.
    CLP flows discounted with Collateral(CLP,USD) and converted to USD.
    USD flows discounted with SOFR.

    Sign convention (matching quantsupport): both disbursement and redemption
    carry positive amounts; the leg side (+1 receive, -1 pay) determines sign.
    """
    spot = fx_spot_val
    pv = 0.0

    # ── CLP leg (receive, side = +1) ──────────────────────────────
    # Notional at start and maturity (both positive for receiver)
    pv += clp_notional * collateral_curve.discount(start_date) / spot
    pv += clp_notional * collateral_curve.discount(maturity) / spot
    # Floating coupons: ICP + spread
    for i in range(len(schedule_q) - 1):
        s, e = schedule_q[i], schedule_q[i + 1]
        yf = day_count.yearFraction(s, e)
        fwd = icp_curve.forwardRate(s, e, day_count, ql.Simple).rate()
        coupon = clp_notional * (fwd + spread) * yf
        pv += coupon * collateral_curve.discount(e) / spot

    # ── USD leg (pay, side = -1) ──────────────────────────────────
    # Notional at start and maturity (both negative for payer)
    pv -= usd_notional * sofr_curve.discount(start_date)
    pv -= usd_notional * sofr_curve.discount(maturity)
    # Floating coupons: SOFR
    for i in range(len(schedule_q) - 1):
        s, e = schedule_q[i], schedule_q[i + 1]
        yf = day_count.yearFraction(s, e)
        fwd = sofr_curve.forwardRate(s, e, day_count, ql.Simple).rate()
        coupon = usd_notional * fwd * yf
        pv -= coupon * sofr_curve.discount(e)

    return pv
