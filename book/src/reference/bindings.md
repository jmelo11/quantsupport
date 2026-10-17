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
the book reads these signatures and docstrings without loading the native
extension. Keep matching Rust documentation for runtime `help()`.

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

## Build the Book

Python 3.11 or later, Rust, mdBook 0.5.2, and mdbook-mermaid 0.17.0 are required.
From the repository root:

```sh
python3 -m venv .venv-docs
.venv-docs/bin/python -m pip install -r book/requirements.txt
cargo install mdbook --version 0.5.2 --locked
cargo install mdbook-mermaid --version 0.17.0 --locked
mdbook-mermaid install .
PATH="$PWD/.venv-docs/bin:$PATH" mdbook build
```

On Windows, activate `.venv-docs\Scripts\Activate.ps1` before `mdbook build`.
The first build compiles the portable Excel catalog exporter. Excel and the
Python native extension are not required to build documentation.

`book/bindings.py` is an mdBook preprocessor: it adds the five Python domain
chapters and the Excel category chapters in memory. Generated references use
the book's sidebar, theme, and search index; no generated Markdown is committed.
Python docstrings are parsed by `docstring-parser` and signatures by the standard
AST parser. Test the generator with:

```sh
.venv-docs/bin/python -m unittest discover -s book/tests -v
```

## Publish and Download

The documentation workflow validates the complete book on pull requests and
deploys it to [GitHub Pages](https://jmelo11.github.io/quantsupport/) on pushes
to `main` or manual runs on `main`. The Pages environment allows deployment
from `main` only.

The Excel chapter links to the XLL attached by the release workflow. Publishing
a new release updates the stable download link automatically. This keeps
versioned binaries in Releases instead of duplicating them in the book.