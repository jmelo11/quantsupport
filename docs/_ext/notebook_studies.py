"""Publish canonical computational studies without executing or duplicating them."""

import json
from pathlib import Path
import re
from urllib.parse import urlsplit

from sphinx.errors import ExtensionError


STUDIES = {
    "Curve construction": [
        ("curves", "bindings/python/examples/bootstrap/curves.ipynb"),
    ],
    "Swaps and collateral currencies": [
        ("first-swap", "bindings/python/examples/valuation/swaps/tour.ipynb"),
        ("par-swap", "bindings/python/examples/valuation/swaps/par_swap.ipynb"),
        ("currency-conversion", "bindings/python/examples/valuation/swaps/currency_conversion.ipynb"),
    ],
    "Fixed income and loans": [
        ("fixed-rate-bond", "bindings/python/examples/valuation/bonds/fixed_rate_bond.ipynb"),
        ("callable-bond", "bindings/python/examples/valuation/bonds/stepped_coupon_bond.ipynb"),
        ("loans", "bindings/python/examples/valuation/loans/loans.ipynb"),
    ],
    "Rate options and volatility": [
        ("caps-and-caplets", "bindings/python/examples/valuation/caps/caps_and_caplets.ipynb"),
        ("curves-and-volatility", "bindings/python/examples/volatilitysurface/curves_and_volatility.ipynb"),
    ],
    "Short-rate simulation": [
        ("simulations", "bindings/python/examples/hullwhite/simulations.ipynb"),
    ],
    "Portfolio risk and benchmarking": [
        ("portfolio", "bindings/python/examples/sensitivity/portfolio.ipynb"),
        ("portfolio-aggregation", "bindings/python/examples/sensitivity/portfolio_aggregation.ipynb"),
        ("portfolio-risk", "bindings/python/examples/sensitivity/portfolio_risk.ipynb"),
        ("sensitivity-benchmark", "examples/sensitivity/bechmark/sensitivity.ipynb"),
    ],
    "Scripted payoffs": [
        ("scripted-cashflows", "bindings/python/examples/scripting/scripted_cashflows.ipynb"),
    ],
    "Exposure and XVA": [
        ("xva", "bindings/python/examples/cva/xva.ipynb"),
    ],
}


def publication_links(text, source, root):
    destinations = {str((root / path).resolve()): slug
                    for studies in STUDIES.values() for slug, path in studies}

    def replace(match):
        target = urlsplit(match[2])
        if target.scheme or target.netloc or not target.path:
            return match[0]
        local = (source.parent / target.path).resolve()
        if str(local) in destinations:
            destination = destinations[str(local)] + ".ipynb"
        else:
            try:
                relative = local.relative_to(root.resolve()).as_posix()
            except ValueError as error:
                raise ExtensionError(f"Study link escapes repository: {source}: {match[2]}") from error
            if not local.exists():
                raise ExtensionError(f"Missing study companion: {source}: {match[2]}")
            destination = f"https://github.com/jmelo11/quantsupport/blob/main/{relative}"
        if target.fragment:
            destination += "#" + target.fragment
        return match[1] + destination + match[3]

    return re.sub(r"(!?\[[^\]]*\]\()([^)]+)(\))", replace, text)


def generate(app):
    root = Path(app.confdir).parent
    output = Path(app.srcdir) / "python" / "studies" / "notebooks"
    output.mkdir(parents=True, exist_ok=True)
    expected = {f"{slug}.ipynb" for studies in STUDIES.values() for slug, _ in studies}
    for previous in output.glob("*.ipynb"):
        if previous.name not in expected:
            previous.unlink()
    for studies in STUDIES.values():
        for slug, relative in studies:
            source = root / relative
            if not source.is_file():
                raise ExtensionError(f"Missing official computational study: {relative}")
            notebook = json.loads(source.read_text(encoding="utf-8"))
            source_url = f"https://github.com/jmelo11/quantsupport/blob/main/{relative}"
            companions = source_url.rsplit("/", 1)[0].replace("/blob/", "/tree/")
            introduction = {
                "cell_type": "markdown", "metadata": {}, "id": "publication-source",
                "source": [
                    f"**Computational study.** [Canonical notebook]({source_url}) · "
                    f"[Companion files and setup]({companions}). "
                    f"{{download}}`Download this notebook <{slug}.ipynb>`.\n\n",
                    "This page renders saved outputs; the documentation build does not execute it. "
                    "Run the canonical notebook from its own directory with the checkout-built "
                    "binding to reproduce the computation. Each study records its market inputs "
                    "and numerical assumptions.\n",
                ],
            }
            cells = notebook.get("cells", [])
            for cell in cells:
                if cell["cell_type"] == "markdown":
                    cell["source"] = publication_links("".join(cell["source"]), source, root)
                # Editor viewers also save HTML/text; publish those portable
                # representations without changing the canonical notebook.
                for result in cell.get("outputs", []):
                    data = result.get("data", {})
                    if "text/html" in data or "text/plain" in data:
                        data.pop("application/vnd.microsoft.datawrangler.viewer.v0+json", None)
            cells.insert(1 if cells and cells[0]["cell_type"] == "markdown" else 0, introduction)
            notebook.setdefault("metadata", {}).setdefault("mystnb", {})["execution_mode"] = "off"
            destination = output / f"{slug}.ipynb"
            text = json.dumps(notebook, ensure_ascii=False, indent=1) + "\n"
            if not destination.exists() or destination.read_text(encoding="utf-8") != text:
                destination.write_text(text, encoding="utf-8")


def setup(app):
    app.connect("builder-inited", generate)
    return {"version": "1", "parallel_read_safe": True, "parallel_write_safe": True}
