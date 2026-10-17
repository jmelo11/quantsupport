# Valuation examples

Each product family has its own Cargo workspace package, source and configuration.
Run these commands from the repository root:

| Product | Package | Command | Python walkthroughs |
| --- | --- | --- | --- |
| [Swaps](swaps/README.md) | `valuation-swaps` | `cargo run -p valuation-swaps` | [Swaps](../../bindings/python/examples/valuation/swaps/README.md) |
| [Bonds](bonds/README.md) | `valuation-bonds` | `cargo run -p valuation-bonds` | [Bonds](../../bindings/python/examples/valuation/bonds/README.md) |
| [Caps and caplets](caps/README.md) | `valuation-caps` | `cargo run -p valuation-caps` | [Caps](../../bindings/python/examples/valuation/caps/README.md) |
| [Loans](loans/README.md) | `valuation-loans` | `cargo run -p valuation-loans` | [Loans](../../bindings/python/examples/valuation/loans/README.md) |

The [Python valuation catalog](../../bindings/python/examples/valuation/README.md)
uses the same product folders. Each notebook runs independently in a fresh kernel.
