# Discount factors and rate conventions

Interest-rate curves provide the discount factors used to value future
payments and the forward rates used to project floating coupons. QuantSupport
exposes these queries through `InterestRatesTermStructure<T>`, so a pricer can
use a calibrated curve, a flat curve or a composite funding curve through the
same interface.

A curve's rate conventions determine how discount factors translate into
quoted rates. Understanding these conventions is useful both when constructing
a market and when interpreting a forward-rate query.

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
Business/252. The day counter measures the accrual fraction between dates,
while the calendar determines any business-day adjustment to those dates.

`ActualActual` splits at calendar-year boundaries and includes every complete
intervening year. `Thirty360US` adjusts February
month-ends after its 31st-day adjustments, differing from common US/NASD rules.
Validate these conventions against the required dates; their API references
describe the implemented rules. The examples here use ACT/360 or ACT/365.

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

`FlatForwardTermStructure<T>` represents the term structure with one rate and
rate definition. An explicit `ZeroRateTermStructure` interpolates zero rates,
and tenor-based zero curves resolve pillar dates from the reference date. Choose
the representation and interpolator together, since they determine the implied
shape between pillars.

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

A calibrated curve has discount-factor nodes used for interpolation and
market quote pillars used for sensitivity reporting. Calibration connects
the two: changing a deposit, swap or bond quote changes the solved discount
factors and hence the value of a trade. On some directly constructed curves,
the pricing nodes themselves are the independent risk inputs.

`Pillars<DualFwd>` exposes labels, values and tape-registration operations.
Calibration supplies a Jacobian to connect pricing nodes to independent market
inputs. A derivative labeled `OIS_USD_SOFR_5Y` is therefore a derivative with
respect to that quoted contract, not necessarily a five-year zero rate or a
discount-factor node. The next chapter derives that connection.
