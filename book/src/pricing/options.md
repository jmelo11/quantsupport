# Equity and FX options

Valuing a European equity or currency option requires spot, carrying costs,
a volatility market and a discount curve. These inputs determine the forward,
the distribution of the payoff and its present value. QuantSupport provides
closed-form pricers for both asset classes and a separate Monte Carlo route
for equity options.

## FX options

For an option on base currency priced in quote currency, let spot \\(S\\)
mean quote units per base unit. The Garman–Kohlhagen representation used by
`FxEuropeanOptionPricer` is

\\[
F=S\frac{P_{\rm base}(0,T)}{P_{\rm quote}(0,T)},\qquad
C=P_{\rm quote}(0,T)
\left[F\Phi(d_1)-K\Phi(d_2)\right].
\\]

The trade contributes notional and side. The call value is expressed in the
quote currency. Both policy-resolved discount curves, pair spot and a compatible
FX surface are required; an explicit policy is mandatory.

For USD/CLP under USD collateral this can mean SOFR for USD and
`Collateral(CLP, USD)` for CLP. A reciprocal surface is oriented through inverse
strike mapping. Curves, active volatility pillars and recorded spot inputs
participate in risk. The standalone
[FX-forward implementation](cross-currency.md#fx-forwards-the-implemented-convention)
uses a different discount ratio; the book does not assume parity between those
implementations.

## Equity carry and European valuation

For an equity with spot \\(S\\), a dividend discount curve \\(P_q\\) and
rate curve \\(P_r\\), the standard forward is \\(F=SP_q/P_r\\). A zero-dividend
assumption uses \\(P_q=1\\). The Black expression is

\\[
C=P_r(0,T)\left[F\Phi(d_1)-K\Phi(d_2)\right],\qquad
d_1=\frac{\log(F/K)+\tfrac12\sigma^2T}{\sigma\sqrt T},
\quad d_2=d_1-\sigma\sqrt T.
\\]

The instrument supplies expiry, strike, index, currency and call/put type.
The closed-form pricer reads the associated surface; its current query uses
the resolved strike directly, so arbitrary smile-type metadata does not
provide an automatic delta-to-strike conversion.

`EquityForwardPricer` separately distinguishes forwarding and
policy-selected payment discounting. Its spot is the equity fixing on the
context evaluation date, not necessarily the trade date. An absent dividend
curve means zero dividends; a missing required spot is an error. For the
implemented forward,

\\[
F=S\frac{P_{\rm dividend}(0,T)}{P_{\rm forward}(0,T)},\qquad
V=N\\,s\\,(F-K)P_{\rm payment}(0,T).
\\]

This distinguishes carrying costs from the curve used to discount the payment.

## Materialized Monte Carlo valuation

`BlackMCEuropeanOptionPricer` reads a configured path set from the market,
selects the stored date closest to expiry and averages intrinsic payoffs:

\\[
\widehat V=N P_d(0,T)\frac1n\sum_p
\max(S^p_{\text{nearest grid date}}-K,0)
\\]

for the call form. Relative/ATM strike resolution uses the sample terminal
mean as a forward proxy. Grid choice therefore matters independently of
sampling error.

The stored paths are generated numerically in `f64`, with their generation
inputs held fixed during pricing. The current sensitivity handler therefore
reports discount-curve pillars and omits path-generation delta and vega.
[Model-backed scripting and exposure](../simulation/monte-carlo.md) use a
separate workflow that retains differentiated path construction for supported
factors.
