# Equity and FX options

To value a European equity or FX option you need:

- the spot price;
- carry: dividends for equities, the second currency's rates for FX;
- a volatility market;
- a discount curve.

QuantSupport has closed-form pricers for both asset classes, and a Monte Carlo
pricer for equity options.

## FX options

Consider an option on a base currency, priced in a quote currency. Let spot
\\(S\\) be the number of quote-currency units per base unit.
`FxEuropeanOptionPricer` uses Garman–Kohlhagen:

\\[
F=S\frac{P_{\rm base}(0,T)}{P_{\rm quote}(0,T)},\qquad
C=P_{\rm quote}(0,T)
\left[F\Phi(d_1)-K\Phi(d_2)\right].
\\]

- The trade gives the notional and side.
- The value is in the quote currency.
- You must provide an explicit discount policy. The pricer needs both discount
  curves it selects, the spot rate for the pair, and a matching FX volatility
  surface.

For USD/CLP under USD collateral, the curves can be SOFR for USD and
`Collateral(CLP, USD)` for CLP. A surface stored for the reverse pair is used
with inverted strikes.

Sensitivities include the curves, the volatility pillars used, and the spot
rate if it is recorded as an AD input.

The [FX forward pricer](cross-currency.md#fx-forwards-the-implemented-convention)
uses a different discount ratio. Do not expect forward–option parity between
the two pricers.

## Equity carry and European valuation

For an equity with spot \\(S\\), dividend discount curve \\(P_q\\), and rate
curve \\(P_r\\), the forward is \\(F=SP_q/P_r\\). With no dividends,
\\(P_q=1\\). The Black formula is

\\[
C=P_r(0,T)\left[F\Phi(d_1)-K\Phi(d_2)\right],\qquad
d_1=\frac{\log(F/K)+\tfrac12\sigma^2T}{\sigma\sqrt T},
\quad d_2=d_1-\sigma\sqrt T.
\\]

The instrument gives the expiry, strike, index, currency, and call or put
type. The closed-form pricer reads volatility from the index's surface at the
resolved strike. It does not convert between delta and strike, so use a surface
quoted by strike.

`EquityForwardPricer` uses one curve to compute the forward and the discount
policy's curve to discount the payment.

- Spot is the equity fixing on the context's evaluation date, which may differ
  from the trade date.
- No dividend curve means zero dividends.
- A missing spot is an error.

The forward and value are

\\[
F=S\frac{P_{\rm dividend}(0,T)}{P_{\rm forward}(0,T)},\qquad
V=N\\,s\\,(F-K)P_{\rm payment}(0,T).
\\]

## Monte Carlo with stored paths

`BlackMCEuropeanOptionPricer` reads a configured set of stored paths from the
market. It takes the stored date closest to expiry and averages the payoffs.
For a call:

\\[
\widehat V=N P_d(0,T)\frac1n\sum_p
\max(S^p_{\text{nearest grid date}}-K,0)
\\]

- For relative and ATM strikes, the forward is estimated as the average
  simulated price on that date.
- Because the pricer uses the nearest grid date, the choice of grid affects the
  result, separately from sampling error.

The paths are generated in `f64` and are held fixed during pricing. The
sensitivities therefore include discount-curve pillars only, with no delta or
vega from the paths.

For path sensitivities, use [model-backed scripting and
exposure](../simulation/monte-carlo.md). They differentiate through path
generation for supported factors.
