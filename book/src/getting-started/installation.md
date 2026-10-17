# Installation

The three interfaces serve different environments. Rust is the native library,
Python loads a compiled extension, and the Excel add-in is a Windows XLL.
Installation should establish which of those environments will execute the
valuation, rather than imply that every interface is required.

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
packages. `cargo test -p quantsupport` checks the native library; building a
specific example is a smaller validation than building the whole workspace.
The repository forbids missing public documentation and enables strict Clippy
groups. Contributors should use its existing Cargo checks rather than infer
support solely from a successful example.

## Python and Excel

Install the published Python package with:

```sh
python -m pip install quantsupport
```

The package supports Python 3.9 and later and includes pandas reporting.
Optional typing tools are available with `quantsupport[typing]`. Building the
extension from a checkout uses the binding's existing development instructions;
an already installed wheel need not match unbuilt source changes.

Excel requires 64-bit Windows desktop Excel.
[Installing the add-in](../excel/installation.md) provides the released XLL and
platform-specific steps. Excel for macOS and the browser cannot load it.

The [first valuation](first-swap.md) establishes a numerical reference in Rust.
The [Python](../python/getting-started.md) and
[Excel](../excel/workbook.md) chapters explain how their execution and reporting
models differ while using the same native financial objects.
