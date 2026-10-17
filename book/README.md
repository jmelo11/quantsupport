# Build the QuantSupport Book

The Book is a connected, implementation-grounded treatment of the whole
library, with independent Rust, Python and Excel overviews. It is not an API
catalog. Sphinx separately provides binding guides and declaration references;
optional Book reference links use their stable published URLs.

To build just the walkthroughs, install Rust, mdBook 0.5.2, and
mdbook-mermaid 0.17.0. From the repository root:

```sh
cargo install mdbook --version 0.5.2 --locked
cargo install mdbook-mermaid --version 0.17.0 --locked
mdbook-mermaid install .
mdbook build
```

Open `book/html/index.html`, or run `mdbook serve` to preview the Book.
The book alone needs no Python environment, Sphinx, native Python extension,
or Excel installation.

For the complete site, follow the [API build guide](../docs/README.md): install
`docs/requirements.txt`, build mdBook first, then run
`sphinx-build -W -b html docs book/html/api`. Rebuild Sphinx after each mdBook
build because mdBook replaces its output tree.

## Validate changes

```sh
python -m unittest discover -s book/tests -v
cargo +stable build -p quantsupport
RUSTUP_TOOLCHAIN=stable mdbook test -L target/debug/deps
mdbook build
```

Use the same compiler toolchain for the Cargo build and mdBook's rustdoc
process. The explicit stable selection also avoids experimental nightly
artifact layouts that no longer place dependency libraries under `debug/deps`.

Runnable Rust fences include hidden `extern crate quantsupport` declarations
because mdBook's rustdoc harness does not add Cargo's `--extern` flags.
`rust,ignore` is reserved for explicitly introduced fragments whose market,
trade or other inputs belong to the surrounding application. Do not use it to
hide failures in complete examples.

The standard-library tests check SUMMARY coverage, local links and anchors,
redirects for consolidated chapters, independent interface chapters,
math delimiters and the runnable-snippet convention. No
binding-reference generator runs inside mdBook. Python declarations and Excel
registration metadata are documented by the Sphinx tooling, not a second
copied generator under `book/`.

The publication workflow builds mdBook then Sphinx. Pull requests validate;
pushes to `main` and manual runs on `main` deploy the combined GitHub Pages
tree. The Excel installation chapter retains the stable release-asset download.
