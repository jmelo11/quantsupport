# Binding Development

## Python Categories and Visibility

Rust wrappers live in `bindings/python/src/{common,time,market,instruments,pricing,xva}`.
Python imports and their stubs live in
`bindings/python/python/quantsupport/{time,market,instruments,pricing,risk}`.
The package root re-exports supported names for compatibility.

Use `pub(crate)` for Rust wrapper types, fields, and shared helpers. Rust
visibility does not control Python visibility: `#[pymethods]` exposes methods,
`#[getter]` exposes properties, and `#[setter]` permits assignment. Contractual
product inputs remain read-only; context inputs are explicitly mutable.

When adding an API, update its domain `__init__.py`, `__init__.pyi`, root
re-exports, and `__all__`. Document the stub's classes and public members;
Sphinx AutoAPI reads these signatures and docstrings without loading the
native extension and gives each public class its own page. Keep matching
Rust documentation for runtime `help()`.

Binding CI checks the installed package against its stubs. Regression tests
also verify domain/root export parity and public member documentation.

## Excel Categories and Visibility

Worksheet functions live in `bindings/excel/src/functions` by domain.
`runtime` owns internal handles and tabular helpers. `documentation` owns
parameter help and the portable catalog exporter. Only worksheet exports and
Excel lifecycle entry points require public Rust visibility.

Register functions with `#[xll_bindgen(name = ..., category = ..., help = ...)]`
and add parameter guidance in `documentation/function_help.rs`. The catalog
exporter parses these attributes with `syn`, expands the local worksheet macros,
and calls the same parameter-help function as the XLL. Windows tests compare
the generated catalog against the registered inventory.

## Build the Documentation

Python 3.11 or later, Rust, mdBook 0.5.2, and mdbook-mermaid 0.17.0 are required.
From the repository root:

```sh
python3 -m venv .venv-docs
.venv-docs/bin/python -m pip install -r docs/requirements.txt
cargo install mdbook --version 0.5.2 --locked
cargo install mdbook-mermaid --version 0.17.0 --locked
mdbook-mermaid install .
.venv-docs/bin/python -m unittest discover -s docs/tests -v
mdbook build
.venv-docs/bin/sphinx-build -W -b html docs book/html/api
```

On Windows, activate `.venv-docs\Scripts\Activate.ps1` and use `python` and
`sphinx-build` for the environment commands above. The Sphinx build compiles
the portable Excel catalog exporter; Excel and the native Python extension
are not required.

mdBook builds the walkthroughs, and Sphinx builds the
[API reference](https://jmelo11.github.io/quantsupport/api/index.html) under
`book/html/api`. Build in that order: a subsequent mdBook build replaces its
output tree. Sphinx provides separate navigation and search for the per-class
[Python reference](https://jmelo11.github.io/quantsupport/api/autoapi/quantsupport/index.html)
and per-function [Excel reference](https://jmelo11.github.io/quantsupport/api/excel/index.html).
Generated API sources and HTML are not committed. See the
[API build guide](https://github.com/jmelo11/quantsupport/blob/main/docs/README.md)
for standalone builds and the combined-tree link requirements.

## Publish and Download

The documentation workflow tests the API generators and builds both mdBook
and Sphinx with warnings treated as errors for Sphinx. Pull requests validate
the combined tree without deploying. Pushes to `main` and manual runs on
`main` deploy it to [GitHub Pages](https://jmelo11.github.io/quantsupport/),
with the API reference under `/api/`. The Pages environment allows deployment
from `main` only.

The Excel chapter links to the XLL attached by the release workflow. Publishing
a new release updates the stable download link automatically. This keeps
versioned binaries in Releases instead of duplicating them in the book.
