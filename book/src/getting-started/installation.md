# Installation

QuantSupport can be used as a Rust crate, a Python package or an Excel add-in.
Choose the interface that fits your application. The Python package includes
a compiled Rust extension; the Excel add-in is distributed as a Windows XLL.

## Rust applications

QuantSupport uses the Rust 2021 edition. A stable toolchain installed through
[rustup](https://rustup.rs) supplies the compiler and Cargo. Add the published
crate to an application with:

```sh
cargo add quantsupport
```

A local application can instead use a path dependency during development:

```toml
[dependencies]
quantsupport = { path = "../quantsupport" }
```

The optional `plot` feature enables plotting support; it is not necessary for
core pricing. Applications loading JSON add their own `serde_json` dependency.
The library owns serializable financial types, while the application owns file
locations and I/O.

For the source checkout:

```sh
git clone https://github.com/jmelo11/quantsupport
cd quantsupport
cargo build -p quantsupport
cargo run -p valuation-swaps
```

The workspace includes the library, Rust examples, benchmarks, and both binding
packages. Run `cargo test -p quantsupport` to check the native library and its
doctests. Contributor checks also include Clippy and documentation builds.

## Python and Excel

Install the published Python package with:

```sh
python -m pip install quantsupport
```

For a manual download, the [Python packages
page](https://jmelo11.github.io/quantsupport/python/downloads.html) provides
folder ZIPs and wheels by operating system, architecture and Python version.
Folder ZIPs contain only QuantSupport; install dependencies separately with
`python -m pip install pandas`.

The package supports Python 3.9 and later and includes pandas reporting.
Optional typing tools are available with `quantsupport[typing]`. To use local
source changes, build the extension using the [binding development
instructions](https://github.com/jmelo11/quantsupport/blob/main/bindings/python/README.md).

Excel requires 64-bit Windows desktop Excel.
[Installing the add-in](../excel/installation.md) provides the released XLL and
platform-specific steps. Excel for macOS and the browser cannot load it.

The [first valuation](first-swap.md) establishes a numerical reference in Rust.
The [Python](../python/getting-started.md) and
[Excel](../excel/workbook.md) chapters explain how their execution and reporting
models differ while using the same native financial objects.
