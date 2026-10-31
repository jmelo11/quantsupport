"""Independent binding guides, computational studies, and exact API references."""

from pathlib import Path
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "docs" / "_ext"))

project = "QuantSupport"
author = "QuantSupport contributors"
release = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
extensions = ["autoapi.extension", "sphinx.ext.napoleon", "excel_api", "myst_nb", "notebook_studies"]
from python_api import class_declaration, construction_note, constructor_signature, declaration_source, method_signature, prepare_class_docstrings, public_annotation, public_signature
from api_navigation import DOMAIN_TITLES, generate_python_navigation, python_sections

autoapi_dirs = [str(ROOT / "bindings" / "python" / "python" / "quantsupport")]
autoapi_template_dir = "_templates/autoapi"
autoapi_file_patterns = ["*.pyi"]
autoapi_own_page_level = "class"
autoapi_options = ["members", "undoc-members", "show-inheritance", "show-module-summary"]
autoapi_python_class_content = "class"
autoapi_member_order = "groupwise"
autoapi_add_toctree_entry = False
napoleon_numpy_docstring = True
napoleon_google_docstring = False
napoleon_use_param = True
napoleon_use_rtype = True
html_theme = "furo"
html_title = f"QuantSupport {release}"
html_baseurl = "https://jmelo11.github.io/quantsupport/api/"
html_show_sourcelink = False
html_static_path = ["_static"]
html_css_files = ["notebook-tables.css"]
exclude_patterns = ["_build", "_templates/**", "README.md", "requirements.txt", "tests/**"]
nb_execution_mode = "off"
myst_enable_extensions = ["dollarmath", "amsmath"]

PUBLIC_PROTOCOLS = {
    "__enter__", "__exit__", "__iter__", "__len__", "__getitem__",
    "__add__", "__radd__", "__sub__", "__rsub__", "__mul__", "__rmul__",
    "__truediv__", "__rtruediv__", "__neg__",
}


def skip_internal(app, what, name, obj, skip, options):
    member = name.rsplit(".", 1)[-1]
    if member in PUBLIC_PROTOCOLS:
        return False
    if member.startswith("_"):
        return True
    return None


def setup(app):
    app.connect("autoapi-skip-member", skip_internal)
    app.connect("autodoc-process-docstring", prepare_class_docstrings, priority=100)
    app.connect("builder-inited", generate_python_navigation)


def prepare_templates(environment):
    environment.filters["constructor_signature"] = constructor_signature
    environment.filters["class_declaration"] = class_declaration
    environment.filters["construction_note"] = construction_note
    environment.filters["declaration_source"] = declaration_source
    environment.filters["public_signature"] = public_signature
    environment.filters["public_annotation"] = public_annotation
    environment.filters["method_signature"] = method_signature
    environment.filters["python_sections"] = python_sections
    environment.filters["domain_title"] = DOMAIN_TITLES.__getitem__


autoapi_prepare_jinja_env = prepare_templates
