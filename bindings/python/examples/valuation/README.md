# Valuation examples

Choose a product family. Each folder owns its notebooks, market loader and any
product-specific model settings. The Rust examples use the same organization.

| Product | Walkthroughs | Rust command |
| --- | --- | --- |
| [Swaps](swaps/README.md) | First valuation, par coupon and currency conversion | `cargo run -p valuation-swaps` |
| [Bonds](bonds/README.md) | Fixed-rate bond analytics and callable stepped-coupon bonds | `cargo run -p valuation-bonds` |
| [Caps and caplets](caps/README.md) | Black option value and curve/volatility quote risk | `cargo run -p valuation-caps` |
| [Loans](loans/README.md) | Repayment schedules, resets, prepayment, fair rates, credit and risk | `cargo run -p valuation-loans` |

Follow the [shared setup](../README.md#install-and-run). From the repository
root, execute one product folder or all valuation examples:

```bash
python bindings/python/examples/run_notebooks.py valuation/loans
python bindings/python/examples/run_notebooks.py valuation
```

A folder selection includes its notebooks recursively. Every notebook starts
with a fresh kernel in its own folder, so loaders resolve their inputs locally.
Use `--output-dir /tmp/quantsupport-notebooks` to save executed copies.

Next: [portfolio sensitivity](../sensitivity/README.md) or
[scripted payoffs](../scripting/README.md).
