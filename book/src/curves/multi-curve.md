# Multi-Curve Pricing

Modern interest-rate pricing separates the curve used to forecast an index from the curve used to discount collateralized cashflows. A SOFR swap may use SOFR for both roles, while a Term SOFR leg forecasts from its own curve and discounts on the collateral curve.

QuantSupport keys curves by `MarketIndex`. Curve configurations declare dependencies, and `BootstrapDiscountPolicy` selects the collateral curve during calibration. The context constructs dependencies before dependent curves.

For a floating coupon at dates \(t_1,t_2\), a simple forward is obtained from forecast-curve discount factors:

\[
F(t_1,t_2)=\frac{P_f(0,t_1)/P_f(0,t_2)-1}{\tau(t_1,t_2)}.
\]

The resulting coupon is discounted with \(P_d\), which may come from another curve. Cross-currency products add FX conversion and currency-specific collateral curves.

Avoid copying one curve under several indices unless that is the intended model. It hides basis risk. Run `cargo run -p sensitivity` to compare SOFR, Term SOFR, ICP, basis, and cross-currency sensitivities by market pillar.
