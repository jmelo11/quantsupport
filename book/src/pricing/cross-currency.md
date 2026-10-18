# Cross-currency contracts

A cross-currency contract exchanges payments in two currencies. Valuation
requires curves for projecting each leg, discount curves consistent with the
collateral agreement, and an FX rate for converting between currencies.
Cross-currency market quotes help calibrate the collateral-adjusted curves.

For a USD-collateralized USD/CLP swap, for example, SOFR can provide USD
projection and discounting, ICP can project CLP coupons, and a separate
USD-collateralized CLP curve discounts those payments.

## Swaps and collateral curves

Fixed–floating and floating–floating cross-currency builders create two currency
legs with initial/final principal exchanges. Notionals can be chosen to be
spot-equivalent at inception: for USD→CLP spot \\(S\\) quoted in CLP per USD,
CLP principal is USD principal times \\(S\\). State this orientation explicitly
when supplying spot or interpreting results.

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

For spot expressed as quote currency per base currency, this implementation
uses the reciprocal of the usual covered-interest-parity discount ratio.
The FX-option pricer uses the usual base/quote ratio. This discrepancy affects
comparisons between the two pricers and must be accounted for when
reproducing forward values or testing option parity.

An outright forward rate and forward points are different calibration
strategies. `AnchorOutrightPrice` uses the outright quote;
`AnchorForwardPoints` uses the configured points interpretation. Their market
identities and native implementation must match the feed's units. Replicating
a vendor forward requires validating orientation, quote units, collateral
curves and settlement behavior together.
