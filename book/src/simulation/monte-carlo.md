# Monte Carlo

Monte Carlo estimates expectations by evaluating payoffs over simulated market paths. QuantSupport provides model components, `PathGenerator`, `SimulationBuilder`, and generated simulation elements that can be stored in a `PricingContext`.

A simulation configuration fixes:

- model and calibrated parameters;
- simulation dates and time step;
- number of paths;
- random seed;
- requested market factors.

For discounted payoff \(X\), the estimator is

\[
\hat V=\frac{1}{N}\sum\_{i=1}^{N}D_iX_i.
\]

Its sampling error decreases at approximately \(N^{-1/2}\). Report path count and seed with results, and assess convergence across path counts rather than trusting one run.

Use deterministic seeds for regression tests. Avoid changing factor ordering accidentally: it changes the mapping of random numbers to markets and can alter otherwise unrelated results. Closed-form products should be used as controls for model and implementation validation.

Run `cargo run -p pfe` for a multi-factor rates and FX simulation workflow.
