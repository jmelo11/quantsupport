"""Symbol-level API reference for the Python package and Excel add-in."""

from pathlib import Path
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "docs" / "_ext"))

project = "QuantSupport API"
author = "QuantSupport contributors"
release = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
extensions = ["autoapi.extension", "sphinx.ext.napoleon", "excel_api"]
from python_api import constructor_signature, declaration_source, method_signature, prepare_class_docstrings, public_signature
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
html_title = f"QuantSupport {release} API"
html_baseurl = "https://jmelo11.github.io/quantsupport/api/"
html_show_sourcelink = False
exclude_patterns = ["_build", "_templates/**", "README.md", "requirements.txt", "tests/**"]


def skip_internal(app, what, name, obj, skip, options):
    if name.rsplit(".", 1)[-1].startswith("_"):
        return True
    return None


def setup(app):
    app.connect("autoapi-skip-member", skip_internal)
    app.connect("autodoc-process-docstring", prepare_class_docstrings, priority=100)
    app.connect("builder-inited", generate_python_navigation)


def prepare_templates(environment):
    environment.filters["constructor_signature"] = constructor_signature
    environment.filters["declaration_source"] = declaration_source
    environment.filters["public_signature"] = public_signature
    environment.filters["method_signature"] = method_signature
    environment.filters["python_sections"] = python_sections
    environment.filters["domain_title"] = DOMAIN_TITLES.__getitem__


autoapi_prepare_jinja_env = prepare_templates