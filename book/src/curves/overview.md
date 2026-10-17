# Discount factors and rate conventions

A rate curve answers a dated question: how much is a payment worth, or what
rate is implied over an interval? `InterestRatesTermStructure<T>` supplies
discount factors and forward rates independently of how the curve is
represented. A calibrated nodal curve, a flat curve and a composite funding
curve can therefore serve the same downstream valuation.

Write \\(P(0,t)\\) for the discount factor from reference date zero to date
\\(t\\). For an interval with year fraction \\(\tau\\),

\\[
P(t_1,t_2)=\frac{P(0,t_2)}{P(0,t_1)},\qquad
L(t_1,t_2)=\frac{P(0,t_1)/P(0,t_2)-1}{\tau}.
\\]

The second expression is a simple annualized forward rate. The equivalent
continuously compounded forward is
\\(-\log(P(t_1,t_2))/\tau\\). Both describe the same accumulation; their numbers
need not agree. A curve's interpolation clock and a coupon's accrual basis
must also be distinguished when translating between them.

## Rate definitions

`RateDefinition` combines day counter, compounding and frequency.
`InterestRate` uses that definition to translate a rate into compound and
discount factors. For year fraction \\(\tau\\), principal conventions are

\\[
C_{\rm simple}=1+r\tau,\qquad
C_{\rm periodic}=(1+r/m)^{m\tau},\qquad
C_{\rm continuous}=e^{r\tau}.
\\]

Mixed simple/periodic conventions additionally switch behavior according to
their interval rule. Frequency matters for periodic compounding even when
payment dates follow another schedule. The first-swap example illustrates why
identical displayed percentages under different conventions are not equal
financial prices.

Day counters include ACT/360, ACT/365, 30/360 variants, Actual/Actual and
Business/252. They are explicit mathematical choices, not interchangeable
labels. Calendar adjustment determines dates; day count determines the
fraction between them.

Names do not establish general convention conformity. The current
`ActualActual` calculation handles same-year and adjacent-year intervals,
but omits one complete intervening year when endpoint years differ by at
least two. `Thirty360US` independently adjusts February month-ends after its
31st-day adjustments, differing from common US/NASD rules. Do not rely on
these implementations as general ISDA or US Bond Basis calculations;
validate the actual dates and contractual convention. The verified first
valuation instead uses ACT/360.

## Curve representations

`DiscountTermStructure<T>` stores dated discount factors. Its first factor is
one at the reference date, and date/value vectors must agree. Pricing values
are interpolated on year fractions. With log-linear interpolation,

\\[
\log P(t)=(1-w)\log P(t_i)+w\log P(t_{i+1}),
\quad w=\frac{t-t_i}{t_{i+1}-t_i},
\\]

so the instantaneous forward is constant between adjacent nodes. Linear and
cubic-spline choices describe different interpolation models. Extrapolation
is a separate permission: log-linear continuation extends the terminal
forward, while linear/cubic choices use their implemented linear
extrapolation. Disabled extrapolation makes out-of-domain queries errors.

`FlatForwardTermStructure<T>` represents the term structure with one rate
and rate definition. An explicit `ZeroRateTermStructure` interpolates zero
rates rather than discount factors; tenor-based zero curves resolve their
pillars from the reference date. Interpolating rates and interpolating
discount factors are not the same construction.

A spread representation can express a continuously compounded adjustment:

\\[
s(t)=-\frac{\log(P_{\rm target}(t)/P_{\rm base}(t))}{t},
\qquad P_s(t)=e^{-s(t)t}.
\\]

`CompositeTermStructure` multiplies component factors,
\\(P=P_bP_s\\). Matching reference dates and day counters keep the component
clocks consistent. This is useful for separating benchmark and funding inputs,
and is supported through serializable curve configurations as well as direct
construction.

## Pricing nodes and risk pillars

A node is a quantity used to evaluate the curve. A risk pillar is an
independent input named in a sensitivity report. They coincide for some
hand-built curves but not for a calibrated curve: its nodes are solved discount
factors, whereas its pillars can be deposit, swap or bond quotes.

`Pillars<DualFwd>` exposes labels, values and tape-registration operations.
Calibration supplies a Jacobian to connect pricing nodes to independent market
inputs. A derivative labeled `OIS_USD_SOFR_5Y` is therefore a derivative with
respect to that quoted contract, not necessarily a five-year zero rate or a
discount-factor node. The next chapter derives that connection.
