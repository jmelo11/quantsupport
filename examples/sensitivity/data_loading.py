import json
import os

import pandas as pd


def load_rust_results(json_path=None):
    """Load Rust sensitivity results from JSON file.

    Returns (rust_products, rust_curves) dictionaries keyed by label/name.
    """
    if json_path is None:
        json_path = os.path.join(os.path.dirname(
            __file__), "data", "rust_results.json")
    with open(json_path) as f:
        rust_data = json.load(f)

    rust_products = {p["label"]: p for p in rust_data["products"]}
    rust_curves = {c["name"]: c for c in rust_data["curves"]}
    return rust_products, rust_curves


def qs_sens_dict(rust_products, label, as_dv01=True):
    """Return {pillar: dv01_or_exposure} for a Rust product."""
    prod = rust_products[label]
    if as_dv01:
        return {s["pillar"]: s["dv01"] for s in prod["sensitivities"]}
    return {s["pillar"]: s["exposure"] for s in prod["sensitivities"]}


def qs_npv(rust_products, label):
    return rust_products[label]["npv"]


def qs_cashflows_df(rust_products, label):
    """Return a pandas DataFrame of cashflow details for a Rust product."""
    cfs = rust_products[label]["cashflows"]
    return pd.DataFrame(cfs)


def qs_curve_df(rust_curves, name):
    """Return a pandas DataFrame of curve nodes (date, yf, df) from Rust."""
    nodes = rust_curves[name]["nodes"]
    return pd.DataFrame(nodes)


def qs_sens_full(rust_products, label):
    """Return {pillar: {dv01, exposure}} for a Rust product."""
    prod = rust_products[label]
    return {
        s["pillar"]: {"dv01": s["dv01"], "exposure": s["exposure"]}
        for s in prod["sensitivities"]
    }
