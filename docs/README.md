# Build the guides, computational studies, and reference

Sphinx builds the Python and Excel user guides, rendered notebooks and API
references. The site can be read independently or published under `/api/`
alongside the QuantSupport Book.

## Standalone build

From the repository root:

```sh
python3 -m venv .venv-docs
.venv-docs/bin/python -m pip install -r docs/requirements.txt
.venv-docs/bin/python -m unittest discover -s docs/tests -v
.venv-docs/bin/sphinx-build -W -b html docs docs/_build/html
```

Open `docs/_build/html/index.html`. On Windows activate
`.venv-docs\Scripts\Activate.ps1` and use `python`/`sphinx-build`.

Requirements are Python 3.11 or later, the pinned documentation packages, and
stable Rust/Cargo on `PATH`. The native Python extension, Windows, Excel,
JupyterLab, notebook execution, and Pandoc are **not** required for this build.
The first build compiles the portable Excel catalog exporter.

## Authored and generated sources

- `docs/python/guide/` and `docs/excel/guide/` contain authored financial
  workflows and explanations.
- AutoAPI reads packaged `.pyi` declarations/docstrings in
  `bindings/python/python/quantsupport`. `docs/_ext/python_api.py` prepares
  public signatures, constructors/factories, and source links.
- Excel names, argument order/types, aliases, and flags come from registration
  attributes. `bindings/excel/src/documentation/function_help.rs` supplies
  parameter/result/error descriptions. `bindings/excel/tools/function_catalog.rs` exports
  this metadata portably; `docs/_ext/excel_api.py` renders the reference.
- `docs/_ext/api_navigation.py` assigns every Python class and Excel function
  to a focused reference topic and rejects unclassified additions.
- `docs/_ext/notebook_studies.py` lists all official computational studies.
  It copies canonical notebooks into an ignored source directory at build
  time, rewrites notebook cross-links to local study pages, and links companion
  files to their canonical repository locations. It does not execute notebooks
  or modify their sources. `docs/python/studies/index.rst` organizes financial
  topics; update both manifest and toctrees when adding a study.

MyST-NB is a declared rendering dependency, not a new validation runner.
`nb_execution_mode = "off"` renders stored Markdown, equations, pandas tables,
and static images. A notebook lacking saved output cannot acquire results
during this documentation build. The downloaded notebook still needs its
companion files and runtime environment; every study links that directory.

Generated `docs/_build/`, `docs/autoapi/`, `docs/excel/functions/`,
`docs/python/sections/`, `docs/python/studies/notebooks/`, and `book/html/`
must not be committed or edited as authored sources.

## Validation and notebook execution

The existing unittest suite validates reference inventories/signatures and
builds Sphinx strictly. It also checks local guide navigation/anchors, official
study coverage/rendering, formula argument positions, alias/protocol/factory
presentation, and executable guide examples. Native guide execution tests run when
a `quantsupport` package is available; otherwise they report an explicit skip.
Use a checkout-built binding rather than an older installed wheel for those
checks. All temporary rendered test output stays under `docs/_build/`.

Execute the main notebook sequence separately in an environment with the
checkout-built package and `bindings/python/examples/requirements.txt`:

```sh
python -m bindings.python.examples.run_notebooks
python -m bindings.python.examples.run_notebooks --write
```

The runner uses fresh kernels and verifies their actual interpreter, binding,
and required exports before executing cells. It rejects stale bindings and
stores sanitized `execution_environment` metadata with Python/package versions
and whether the kernel matched its caller. Inspect that provenance rather than
assuming a command's displayed environment name proves which kernel ran.
Without `--write`, sources remain unchanged;
use `--write` only when intentionally refreshing publication outputs. The
optional legacy sensitivity benchmark has additional external dependencies
and sits outside this runner's default directory. Install its declared optional
requirements and select all seventeen with:

```sh
python -m pip install "open-source-risk-engine==1.8.14.1" "curveengine @ git+https://github.com/jmelo11/curveengine.git@d7e000da626e08ff2be5d34561597065340b017e"
python -m bindings.python.examples.run_notebooks --include-benchmark
```

The `benchmark` selector runs only that study. Its rendered page identifies
the external prerequisites rather than claiming they are part of the core runtime.

## Combined publication

Install mdBook 0.5.2 and mdbook-mermaid 0.17.0 for the combined site:

```sh
cargo install mdbook --version 0.5.2 --locked
cargo install mdbook-mermaid --version 0.17.0 --locked
mdbook-mermaid install .
mdbook build
.venv-docs/bin/sphinx-build -W -b html docs book/html/api
.venv-docs/bin/python -m http.server --directory book/html 8000
```

Build mdBook **before** Sphinx: mdBook replaces its output tree. The Book is at
`http://localhost:8000/`; the Sphinx guides, studies, and reference are at
`http://localhost:8000/api/`. Existing generated Python-class and Excel-function
URLs are preserved.

The [Pages workflow](../.github/workflows/pages.yml) installs the declared
dependencies, runs documentation tests, and builds both sites with strict
warnings. Pull requests validate without deploying. Main-branch publication
uploads the combined `book/html` artifact to
<https://jmelo11.github.io/quantsupport/>.

Pushes to main also validate documentation, but automatic deployment waits for
the **Release** workflow to finish successfully. Pages resolves that run's
reserved version, builds the site and workbooks from its tagged source, and
downloads the XLL, XML, IntelliSense companion and ZIP from that same release.
Missing assets or a ZIP that disagrees with the standalone files fail with an
explicit error; files from an older release are never substituted.

For a manual redeployment, run **Documentation** on main using
`workflow_dispatch`. Its optional `release_tag` selects a published version;
blank selects the latest published release.

To publish documentation updates without a package release, include
`[skip release]` in the documentation commit message and manually run
**Documentation** with `documentation_ref` set to that commit's full SHA.
The workflow first checks out the selected published release, then restores
only `docs/` and `bindings/python/examples/` from the documentation revision.
The library version, packaged binding declarations, Excel catalog, workbooks
and release downloads continue to come from the selected release. A blank
`documentation_ref` keeps the existing release-only redeployment behaviour.

Release-triggered deployments queue independently of push/PR validation, with
`queue: max` preserving pending runs. The skipped Release triggered by a version
commit must not replace the deployment triggered by the successful release.
