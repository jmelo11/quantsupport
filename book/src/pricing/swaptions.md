# Swaptions

A European swaption grants the right to enter an underlying swap at one exercise date. `MakeSwaption` constructs the option and underlying schedule; `SwaptionType` determines payer or receiver orientation.

QuantSupport provides `ClosedFormHullWhiteSwaptionPricer`. It consumes the relevant curves and a Hull-White model, making it suitable for workflows where one calibrated rates model prices several optional products.

Key inputs are:

- exercise date and underlying swap dates;
- strike and payer/receiver type;
- fixed and floating conventions;
- discount and forecast curves;
- calibrated model volatility and mean reversion.

For market-quote calibration, swaption volatility is usually indexed by option expiry, underlying tenor, and strike or moneyness. Represent that data with a volatility cube and preserve its quoting convention.

Validate a swaption implementation with intrinsic-value limits, payer/receiver parity where applicable, and convergence as model volatility approaches zero.
