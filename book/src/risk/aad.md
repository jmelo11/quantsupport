# Automatic Differentiation

QuantSupport uses automatic differentiation (AD) to propagate derivatives through curves, pricing formulas, simulations, and XVA. The main scalar types are:

- `Fwd<T>` for nestable forward mode;
- `Fwd1`, `Fwd2`, and higher-order aliases;
- `Dual<T>` for mixed modes;
- `DualFwd`, the common scalar for market quote sensitivities;
- `Tape` for reverse accumulation.

If a bootstrapped curve node depends on quote \(q_i\) and a trade value depends on that node, AD applies the full chain rule:

\[
\frac{\partial V}{\partial q_i}
=\sum_j\frac{\partial V}{\partial z_j}
\frac{\partial z_j}{\partial q_i}.
\]

This is why `Request::Sensitivities` reports calibration-quote risk rather than only zero-node risk.

The tape has a lifecycle. Start recording before constructing differentiable market objects, evaluate while they remain alive, then stop or rewind at the workflow boundary. The Python `PricingContext` manager handles this lifecycle. In Rust, prefer the high-level pricing and XVA workflows unless implementing a specialized AD algorithm.

AD derivatives are local derivatives. Use [Scenario Analysis](scenarios.md) for finite market moves and nonlinear effects.
