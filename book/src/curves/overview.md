# Discount factors and rate conventions

Interest-rate curves give two things:

- discount factors, used to value future payments;
- forward rates, used to project floating coupons.

All curves implement `InterestRatesTermStructure<T>`. A pricer can therefore
use a calibrated curve, a flat curve, or a composite funding curve in the same
way.

A curve's rate conventions decide how its discount factors convert to quoted
rates. You need them both when you build a market and when you read a forward
rate from a curve.

Write \\(P(0,t)\\) for the discount factor from the reference date to date
\\(t\\). For a period with year fraction \\(\tau\\),

\\[
P(t_1,t_2)=\frac{P(0,t_2)}{P(0,t_1)},\qquad
L(t_1,t_2)=\frac{P(0,t_1)/P(0,t_2)-1}{\tau}.
\\]

\\(L\\) is the simple annualized forward rate. The continuously compounded
forward over the same period is \\(-\log(P(t_1,t_2))/\tau\\). Both describe the
same growth, but the numbers are different.

When you convert between them, also keep apart the curve's time measure (used
for interpolation) and the coupon's accrual basis.

## Rate definitions

A `RateDefinition` is a day counter, a compounding method, and a frequency.
`InterestRate` uses it to turn a rate into compound and discount factors. For
a year fraction \\(\tau\\), the main methods are

\\[
C_{\rm simple}=1+r\tau,\qquad
C_{\rm periodic}=(1+r/m)^{m\tau},\qquad
C_{\rm continuous}=e^{r\tau}.
\\]

- `SimpleThenCompounded` uses simple compounding for periods up to one
  compounding period (\\(1/m\\) years) and periodic compounding for longer
  periods. `CompoundedThenSimple` does the reverse.
- For periodic compounding, the frequency matters even if payments follow a
  different schedule.

The [first valuation](../getting-started/first-swap.md) shows that the same
percentage under two conventions gives two different prices.

Available day counters include ACT/360, ACT/365, 30/360 variants,
Actual/Actual, and Business/252. The day counter measures the accrual fraction
between two dates. The calendar decides how those dates are adjusted for
holidays.

Two day counters differ from some common definitions:

- `ActualActual` splits the period at calendar-year boundaries and counts each
  full year in between.
- `Thirty360US` adjusts February month-ends after its rules for the 31st. This
  differs from the common US/NASD rule.

Check these conventions against your contract dates. The API reference
describes the exact rules. The examples in this Book use ACT/360 or ACT/365.

## Curve representations

`DiscountTermStructure<T>` stores discount factors on dates.

- The first factor is one, at the reference date.
- The date and value lists must have the same length.
- Values are interpolated on year fractions.

With log-linear interpolation,

\\[
\log P(t)=(1-w)\log P(t_i)+w\log P(t_{i+1}),
\quad w=\frac{t-t_i}{t_{i+1}-t_i},
\\]

so the instantaneous forward rate is constant between two nodes. Linear and
cubic-spline interpolation give different shapes.

Extrapolation is a separate setting:

- Log-linear extrapolation extends the last forward rate.
- Linear and cubic interpolators extrapolate linearly.
- If extrapolation is disabled, a query outside the curve's dates is an error.

Other representations:

- `FlatForwardTermStructure<T>` uses a single rate and rate definition.
- `ZeroRateTermStructure` interpolates zero rates. Tenor-based zero curves
  compute their pillar dates from the reference date.

Choose the representation and interpolator together, because together they set
the curve shape between pillars.

A spread curve can be written as a continuously compounded adjustment:

\\[
s(t)=-\frac{\log(P_{\rm target}(t)/P_{\rm base}(t))}{t},
\qquad P_s(t)=e^{-s(t)t}.
\\]

`CompositeTermStructure` multiplies the discount factors of its components,
\\(P=P_bP_s\\). Its components should have the same reference date and day
counter. A composite curve is useful to keep a benchmark curve and a funding
spread separate. You can build it directly or from a serializable curve
configuration.

## Pricing nodes and risk pillars

A calibrated curve has two sets of points:

- **nodes**: the discount factors used for interpolation;
- **pillars**: the market quotes used to report sensitivities.

Calibration links them. If a deposit, swap, or bond quote changes, the solved
discount factors change, and so does the value of a trade. For some curves
built directly, the nodes themselves are the risk inputs.

`Pillars<Recorded>` gives the labels and values of the pillars and registers
them as AD inputs. Calibration provides a Jacobian that links the nodes to the
market quotes.

So a sensitivity labelled `OIS_USD_SOFR_5Y` is a sensitivity to that quoted
contract. It is not necessarily a sensitivity to a five-year zero rate or to a
discount-factor node. The next chapter explains how this link is computed.
