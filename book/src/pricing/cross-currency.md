# Cross-currency contracts

Cross-currency valuation separates four roles: each index projects its own
coupons, a collateral agreement selects discount curves, spot connects payment
currencies, and cross-currency instruments calibrate collateral adjustments.
Conflating any two of these roles can produce plausible numbers in the wrong
currency or with the wrong market risk.

## Swaps and collateral curves

Fixed–floating and floating–floating cross-currency builders create two
currency legs with initial/final principal exchanges. Notionals can be chosen
to be spot-equivalent at inception: for USD→CLP spot \\(S\\) quoted in CLP per
USD, CLP principal is USD principal times \\(S\\). A symbol such as
`fx_clpusd` is not a reliable substitute for explicitly stating these units.

A USD CSA policy selects SOFR for USD payments and
`Collateral(CLP, USD)` for CLP payments. The latter is not the local CLP ICP
projection curve. Calibration can use FX forward quotes and cross-currency
swaps, with an FX store supplying consistent inception notionals.

For a CLP payment converted to USD under that policy, the implemented
discount-and-convert contribution has the form

\\[
PV_{\rm USD}=C_{\rm CLP}\\,S_{\rm CLP\to USD}
\\,P_{\rm CLP|USD}(0,T).
\\]

The pricer's actual currency rule must be checked for each product; this is not
a universal context-level reporting conversion. Generic DCF payment filtering
and `FairRate` limitations described in [Swaps](swaps.md) still apply.

Risk follows the construction graph. USD projection/discounting reaches SOFR
quotes; CLP coupon projection reaches ICP; collateral discounting reaches
cross-currency and FX-forward calibration inputs. Spot risk also requires
recorded spot inputs. The existence of a spot store alone does not guarantee
that every particular sensitivity report includes all spot dependencies.

## FX forwards: the implemented convention

`MakeFxForward` creates a future currency exchange, with optional
non-deliverable settlement configuration. `FxForwardPricer` requires an
explicit discount policy and supports value, fair rate and sensitivities.
Its current implementation computes

\\[
F_{\rm impl}=S\frac{P_{\rm quote}(0,T)}{P_{\rm base}(0,T)},\qquad
V=N\\,s\\,(F_{\rm impl}-K)P_{\rm quote}(0,T)
\\]

when a contractual forward price is present.

This ratio is stated deliberately. For spot expressed as quote currency per
base currency, the usual covered-interest-parity formula uses the reciprocal
discount-factor ratio. The library's FX-option pricer uses that usual
base/quote ratio, whereas the standalone forward implementation above does
not. Do not assume the two pricers are interchangeable representations of one
market convention, or silently substitute a textbook expression while
explaining the implemented forward results.

An outright forward rate and forward points are different calibration
strategies. `AnchorOutrightPrice` uses the outright quote;
`AnchorForwardPoints` uses the configured points interpretation. Their market
identities and native implementation must match the feed's units. Replicating
a vendor forward requires validating orientation, quote units, collateral
curves and settlement behavior together.
