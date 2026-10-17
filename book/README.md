# Build the QuantSupport Book

mdBook contains the Rust, Python, and Excel walkthroughs. The separate
[Sphinx API reference](https://jmelo11.github.io/quantsupport/api/index.html)
provides per-class Python and per-function Excel pages under `/api/` in the
same GitHub Pages deployment. Each site has its own navigation and search.
The Excel installation chapter links to the latest released `quantsupport.xll`.

To build just the walkthroughs, install Rust, mdBook 0.5.2, and
mdbook-mermaid 0.17.0. From the repository root:

```sh
cargo install mdbook --version 0.5.2 --locked
cargo install mdbook-mermaid --version 0.17.0 --locked
mdbook-mermaid install .
mdbook build
```

Open `book/html/index.html`, or run `mdbook serve` to preview the walkthroughs.
The book alone needs no Python environment, Sphinx, native Python extension,
or Excel installation.

For the complete site, follow the [API build guide](../docs/README.md): install
`docs/requirements.txt`, build mdBook first, then run
`sphinx-build -W -b html docs book/html/api`. Rebuild Sphinx after each mdBook
build because mdBook replaces its output tree.

See [Binding Development](src/reference/bindings.md) for visibility conventions
and the publication workflow. Pull requests validate both sites; pushes to
`main` and manual runs on `main` deploy the combined tree through GitHub Pages.
