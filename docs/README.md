# Build the API Reference

Sphinx builds the [API reference](https://jmelo11.github.io/quantsupport/api/index.html)
with a page per public Python class and per Excel worksheet function. mdBook
remains the walkthrough site at the deployment root. The combined Pages tree
contains the Sphinx site under `/api/`, with its own navigation and search.

## Requirements

- Python 3.11 or later and the packages in `docs/requirements.txt`.
- The stable Rust toolchain and Cargo, available on `PATH`.
- mdBook 0.5.2 and mdbook-mermaid 0.17.0 for the combined site.

Sphinx AutoAPI reads the packaged Python `.pyi` signatures and docstrings.
The Excel extension runs the portable Rust `function_catalog` tool in
`bindings/excel/tools/function_catalog.rs` to
generate a page for each registered function, including aliases and parameter
help. Neither the native Python extension nor Windows or Excel is required.
The first API build compiles the catalog exporter.

Domain indexes contain focused topic pages rather than flat class/function
lists. `docs/_ext/api_navigation.py` assigns each public symbol to a subsection;
the build rejects missing or duplicated Python assignments and unclassified
Excel functions. Update these assignments when adding binding components.
The API's component-coverage page distinguishes direct wrappers from configured
workflows and remaining Rust-only APIs.

## Combined Build

Run from the repository root:

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
`sphinx-build` in place of the `.venv-docs/bin/` commands.

Always build mdBook **before** running
`sphinx-build -W -b html docs book/html/api`: mdBook replaces its output tree.
Open `book/html/index.html` for the walkthroughs and `book/html/api/index.html`
for the reference. The book's API entry chapters use relative links that work
in both the combined local build and the published site.
To serve the complete tree locally, run:

```sh
.venv-docs/bin/python -m http.server --directory book/html 8000
```

The local walkthroughs are at `http://localhost:8000/` and the API reference is
at `http://localhost:8000/api/`.

## Standalone Build

For API-only work, after installing the requirements:

```sh
.venv-docs/bin/sphinx-build -W -b html docs docs/_build/html
```

Open `docs/_build/html/index.html`. API-internal links work in this standalone
output, but relative links back to the book only work in the combined tree,
where the book sits one level above `/api/`.

Generated output in `docs/_build/`, `docs/autoapi/`,
`docs/excel/functions/`, `docs/python/sections/`, and `book/html/` is ignored and
should not be committed.

## Publication

The [Pages workflow](../.github/workflows/pages.yml) installs
`docs/requirements.txt`, tests the API generators with
`python -m unittest discover -s docs/tests -v`, builds mdBook, then builds Sphinx
with `-W` so warnings fail validation. Pull requests targeting `main` validate
both sites without deploying. Pushes to `main` and manual runs on `main`
upload and deploy the combined `book/html` artifact; the reference is served
at `https://jmelo11.github.io/quantsupport/api/`.
