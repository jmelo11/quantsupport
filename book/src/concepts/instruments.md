# Cashflows, contracts and positions

QuantSupport represents many financial contracts as cashflows grouped into
legs. An instrument collects contractual terms, including payment amounts,
dates and observation rules. A trade adds the position held in that instrument.
Pricers use these objects for valuation, and supported trades can also be
converted into claims for exposure simulation.

## Amounts and signs

A fixed coupon combines notional \\(N\\), a rate \\(r\\), and a rate definition.
Its general amount is \\(N(C(r,\tau)-1)\\), where \\(C\\) is the compound factor.
Only simple compounding reduces this to \\(Nr\tau\\). A floating coupon combines
an observed or projected index rate with spread and accrual conventions.
Optional coupon variants retain a cap, floor or more general payoff expression.
Redemptions and disbursements represent principal movements separately.

A leg shares currency, side, projection identity and optional discount
identity. Its side is `LongReceive` or `PayShort`, with signs \\(+1\\) and \\(-1\\).
For a regular swap, leg zero is fixed and leg one is floating; the builder
assigns opposite directions. Product-specific pricers and claim converters
apply position direction according to their own contract/trade conventions.
Do not multiply signs twice because both an instrument and a trade expose
direction-related fields.

Cashflow reports show signed amounts: receipts are positive and payments are
negative. The generic report applies leg direction and reverses disbursements,
so an initial advance has the opposite sign to principal repayment. The loan
pricer reports projected signed receipts after prepayment and credit adjustments.

## Schedules are economic inputs

`MakeSchedule` and product builders convert start/maturity dates, payment
frequency, calendar, business-day convention and generation rule into dated
obligations. Front and back stubs, end-of-month behavior and termination-date
adjustment can change amounts as well as payment dates.

A coupon has an accrual interval, an observation date or schedule, and a
payment date. These dates serve different purposes. For example, a loan can
pay monthly while resetting against EURIBOR annually. An OIS coupon compounds
daily overnight observations with the appropriate accrual weights. Its
observation rules determine which fixings enter the coupon, while payment lag
determines when the resulting amount is paid.

`DayCounter` converts dates into accrual fractions. Curve interpolation can use
a different day counter from the coupon; the equivalent projected rate must
still be expressed on the coupon's basis. Calendar adjustment also differs from
day-count measurement. Neither can be inferred from a displayed rate alone.

## Calendars

Generic builders often default to a null calendar and unadjusted dates;
market-specific builders may apply presets. SOFR OIS uses its index calendar,
while SONIA and TONAR use London and Japanese settlement holidays. Canonical
serialized forms include `UnitedKingdom(Settlement)` and `Japan`.

Joint calendars encode which markets must be open. `JoinHolidays` requires all
components to be business days; `JoinBusinessDays` requires at least one.
Canonical nested definitions round-trip through JSON. Local holiday overrides
participate in calculations, but serialization stores the calendar definition,
not an independently complete record of mutable overrides.

Holiday rules are historical models too. London recurring rules follow the
regime since 1971; Japan's public-holiday rules begin in July 1948. Future
Japanese equinox dates remain estimates until announced, and exceptional
closures require updates or explicit local overrides.

## Builders and positions

`Make*` builders validate required terms and construct the instrument. Check
their defaults against the contract's market conventions. `MakeSwap` produces
regular single-fixing coupons; `MakeOvernightIndexSwap` starts from index-specific
overnight conventions. Bond and loan builders have distinct principal,
schedule and optionality responsibilities.

A trade adds date, side and position sizing as supported by the product.
Bond positions additionally carry a clean/dirty price or spread anchor.
Discounting remains a pricing policy rather than a general contractual promise
embedded in every instrument.

For simulation, supported trades implement `IntoContingentClaims`. Each claim
represents a scheduled payment with evaluation instructions and stable trade
and leg identifiers. Conversion checks whether the exposure implementation
supports the contract's features. The legacy claim converter, for example,
rejects nonstandard overnight conventions.
