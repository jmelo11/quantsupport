# Build the QuantSupport Book

The Book contains the Rust guide, shared financial concepts and implementation
notes for the native library.

```sh
cargo install mdbook --version 0.5.2 --locked
cargo install mdbook-mermaid --version 0.17.0 --locked
mdbook-mermaid install .
mdbook build
python -m unittest discover -s tools/tests -p test_book.py -v
RUSTUP_TOOLCHAIN=stable python tools/book/check_rust.py
```

Open `book/html/index.html`, or run `mdbook serve` to preview it.
