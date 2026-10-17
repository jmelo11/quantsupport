# Cashflows, contracts and positions

A valuation begins with obligations, not curves. QuantSupport represents many
obligations as cashflows grouped into legs. Instruments collect the contractual
terms; trades supply the held position. This hierarchy lets the same contract
participate in deterministic pricing and, where supported, conversion into
simulation claims.

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

The generic cashflow report preserves underlying amounts before leg-side
multiplication. A table of positive coupon amounts is therefore not a table of
positive portfolio receipts. Specialized reporting, such as projected loan
receipts, has its own explicit semantics.

## Schedules are economic inputs

`MakeSchedule` and product builders convert start/maturity dates, payment
frequency, calendar, business-day convention and generation rule into dated
obligations. Front and back stubs, end-of-month behavior and termination-date
adjustment can change amounts as well as payment dates.

Several clocks must remain distinct. A coupon accrues over one interval, fixes
on a specified date or observation schedule, and pays on another date. A term
index's observation tenor need not equal coupon frequency: a monthly loan can
retain annual EURIBOR resets. OIS coupons instead compound weighted daily
overnight observations. Payment lag is not lookback, and publication lag is
not either of them.

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

`Make*` builders validate required terms and construct the instrument. Their
defaults are conveniences, not universal market standards. `MakeSwap` produces
regular single-fixing coupons; `MakeOvernightIndexSwap` starts from index-specific
overnight conventions. Bond and loan builders have distinct principal,
schedule and optionality responsibilities.

A trade adds date, side and position sizing as supported by the product.
Bond positions additionally carry a clean/dirty price or spread anchor.
Discounting remains a pricing policy rather than a general contractual promise
embedded in every instrument.

For simulation, supported trades implement `IntoContingentClaims`. A claim
is one scheduled payment with evaluation instructions and stable trade/leg
identity. This is an extension of the contractual representation, not proof
that every feature of every instrument is implemented by every exposure
method. Nonstandard overnight conventions, for example, are rejected by the
legacy claim converter rather than silently approximated.
