# Build the QuantSupport Book

The Rust guides, Python API, and Excel worksheet reference share one mdBook site,
sidebar, theme, and search index. The Excel installation chapter links directly
to the latest released `quantsupport.xll`.

Python 3.11+, Rust, mdBook 0.5.2, and mdbook-mermaid 0.17.0 are required.
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
Open `book/html/index.html`, or run `mdbook serve` with the same environment.
No Sphinx installation, native Python extension, or Excel installation is needed.

The bindings preprocessor reads the categorized `.pyi` files and runs the
portable Rust Excel catalog exporter. It adds reference chapters in memory;
generated Markdown files are not committed. To test the generator:

```sh
.venv-docs/bin/python -m unittest discover -s book/tests -v
```

See [Binding Development](src/reference/bindings.md) for visibility conventions
and the publication workflow. Pull requests validate the full book; pushes to
`main` and manual runs on `main` deploy it through GitHub Pages.