import QuantLib as ql
import pandas as pd

from data_loading import qs_cashflows_df, qs_curve_df


def display_dv01(label, base_npv, dv01_dict, qs_dict=None, rs_npv=None):
    """Print DV01 + raw-exposure table with quantsupport comparison."""
    print("═" * 95)
    print(f"  {label}")
    npv_str = f"  NPV = {base_npv:>14.2f} USD"
    if rs_npv is not None:
        npv_str += f"  (quantsupport: {rs_npv:>14.2f} USD)"
    print(npv_str)
    print("═" * 95)

    if qs_dict:
        print(
            f"  {'Pillar':<45} {'QL DV01':>10} {'QS DV01':>10} {'QS Exposure':>14} {'Diff':>10}")
    else:
        print(f"  {'Pillar':<45} {'DV01 (USD/bp)':>16}")
    print(f"  {'-'*91}")

    total_ql = 0.0
    total_rs = 0.0
    for name, dv01 in dv01_dict.items():
        total_ql += dv01
        if qs_dict:
            rs = qs_dict.get(name, {})
            rs_dv01 = rs.get("dv01", 0.0) if isinstance(rs, dict) else rs
            rs_exp = rs.get("exposure", 0.0) if isinstance(
                rs, dict) else rs * 1e4
            total_rs += rs_dv01
            print(
                f"  {name:<45} {dv01:>10.2f} {rs_dv01:>10.2f} {rs_exp:>14.4f} {dv01-rs_dv01:>10.2f}")
        else:
            print(f"  {name:<45} {dv01:>16.2f}")
    print(f"  {'-'*91}")
    if qs_dict:
        print(
            f"  {'TOTAL':<45} {total_ql:>10.2f} {total_rs:>10.2f} {'':>14} {total_ql-total_rs:>10.2f}")
    else:
        print(f"  {'TOTAL':<45} {total_ql:>16.2f}")
    print()


def compare_discount_factors(ql_curve, qs_curve_name, rust_curves,
                             reference_date, day_count, title):
    """Compare QL and QS discount factors at standard tenors."""
    qs_df = qs_curve_df(rust_curves, qs_curve_name)
    print(f"\n{'─'*80}")
    print(f"  Discount Factor Comparison: {title}")
    print(f"{'─'*80}")
    print(f"  {'Date':<12} {'YF':>8} {'QL DF':>16} {'QS DF':>16} {'Diff':>14}")
    print(f"  {'-'*68}")
    for _, row in qs_df.iterrows():
        date = ql.Date(row["date"], "%Y-%m-%d")
        yf_ql = day_count.yearFraction(reference_date, date)
        df_ql = ql_curve.discount(date)
        df_qs = row["discount_factor"]
        diff = df_ql - df_qs
        print(
            f"  {row['date']:<12} {yf_ql:>8.4f} {df_ql:>16.10f} {df_qs:>16.10f} {diff:>14.2e}")
    print()


def display_cashflows(rust_products, label):
    """Display the cashflow details from the Rust output."""
    df = qs_cashflows_df(rust_products, label)
    if df.empty:
        print("  No cashflows available.")
        return
    coupons = df[df["cashflow_type"].isin(
        ["FixedRateCoupon", "FloatingRateCoupon"])].copy()
    if coupons.empty:
        print("  No coupon cashflows.")
        return
    coupons["rate_pct"] = coupons["rate"].apply(
        lambda x: f"{x*100:.4f}%" if x is not None else "—")
    cols = ["payment_date", "cashflow_type", "side", "currency", "notional",
            "rate_pct", "year_fraction", "amount", "discount_factor"]
    print(coupons[cols].to_string(index=False,
          float_format=lambda x: f"{x:,.6f}"))
