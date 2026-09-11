# Examples

Every example is a workspace package and uses the local QuantSupport crate.

| Package              | Workflow                                                | Command                                           |
| -------------------- | ------------------------------------------------------- | ------------------------------------------------- |
| `valuation`          | Flat-curve swap NPV, cashflows, fair rate, and AAD risk | `cargo run -p valuation`                          |
| `bootstrap`          | JSON-driven USD/CLP dependent curve bootstrap           | `cargo run -p bootstrap`                          |
| `sensitivity`        | Multi-curve and cross-currency pillar risk              | `cargo run -p sensitivity`                        |
| `evaluator`          | Type-erased evaluation across products                  | `cargo run -p evaluator`                          |
| `volatilitysurface`  | Build and query a caplet volatility surface             | `cargo run -p volatilitysurface`                  |
| `hullwhite`          | Calibration, option pricing, simulation, and plots      | `cargo run -p hullwhite`                          |
| `pfe`                | Multi-currency LGM exposure simulation                  | `cargo run -p pfe --release`                      |
| `cva`                | Netting-set XVA, exposure, and AAD sensitivities        | `cargo run -p cva --release`                      |
| `scripting_examples` | Script/native valuation and XVA comparisons             | `cargo run -p scripting_examples --bin valuation` |

Build all packages with `cargo build --workspace`. Run library and doctests with `cargo test -p quantsupport`, or the complete workspace with `cargo test --workspace`.

Python users should open `bindings/python/examples/tour.ipynb` after installing the extension with maturin.
