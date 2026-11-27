# Cross-currency contracts

A cross-currency contract exchanges payments in two currencies. To value it you
need:

- a projection curve for each floating leg;
- discount curves that match the collateral agreement;
- an FX rate to convert between the currencies.

Cross-currency quotes are used to calibrate the collateral-adjusted curves.

For example, for a USD/CLP swap collateralized in USD:

- SOFR projects and discounts the USD leg;
- ICP projects the CLP coupons;
- a separate USD-collateralized CLP curve discounts the CLP payments.

## Swaps and collateral curves

The fixed–floating and floating–floating cross-currency builders create two
legs in different currencies, with principal exchanged at the start and at the
end.

You can make the notionals equal at today's spot rate. For a USD→CLP spot
\\(S\\) quoted in CLP per USD, the CLP principal is the USD principal times
\\(S\\). Always state which direction the spot rate is quoted in, both when you
supply it and when you read results.

A USD CSA policy uses:

- SOFR for USD payments;
- `Collateral(CLP, USD)` for CLP payments. This is not the local ICP
  projection curve.

The collateral curve can be calibrated from FX forward quotes and
cross-currency swaps. An FX store supplies the spot rate used for the starting
notionals.

Under this policy, a CLP payment converted to USD contributes

\\[
PV_{\rm USD}=C_{\rm CLP}\\,S_{\rm CLP\to USD}
\\,P_{\rm CLP|USD}(0,T).
\\]

Currency conversion is done by each pricer, not by the context. Check the rule
of the pricer you use. The payment selection and the zero-based
`FairRate(index)` leg selection described in [Swaps](swaps.md) also apply.

Sensitivities follow the curves each leg uses:

- USD projection and discounting give risk to SOFR quotes.
- CLP coupon projection gives risk to ICP quotes.
- Collateral discounting gives risk to the cross-currency and FX-forward
  quotes used to calibrate it.

Spot FX risk appears only if the spot rate is recorded as an AD input. Having
an FX store does not guarantee that a sensitivity report includes every spot
dependency.

## FX forwards: the implemented convention

`MakeFxForward` creates a future currency exchange. It can optionally be set up
as a non-deliverable forward (NDF).

`FxForwardPricer` needs an explicit discount policy. It supports value, fair
rate, and sensitivities. When the contract has a forward price \\(K\\), it
computes

\\[
F_{\rm impl}=S\frac{P_{\rm quote}(0,T)}{P_{\rm base}(0,T)},\qquad
V=N\\,s\\,(F_{\rm impl}-K)P_{\rm quote}(0,T)
\\]

**Caveat:** with spot quoted as quote currency per base currency, this formula
uses the reciprocal of the usual covered-interest-parity discount ratio. The
FX option pricer uses the usual base/quote ratio. Keep this in mind when you
compare the two pricers, reproduce forward values, or test put–call parity.

Forwards can be calibrated from outright rates or from forward points:

- `AnchorOutrightPrice` uses the outright quote.
- `AnchorForwardPoints` uses forward points, as configured.

The quote identifiers must match the units of your data feed. To reproduce a
vendor's forward, check the spot direction, quote units, collateral curves, and
settlement rules together.
