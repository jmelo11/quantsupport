# Cashflows, contracts and positions

QuantSupport represents many contracts as cashflows grouped into legs.

- An **instrument** holds the contract terms: payment amounts, dates, and the
  rates or prices it observes.
- A **trade** adds the position you hold in that instrument.

Pricers value trades. Supported trades can also be converted into claims for
exposure simulation.

## Amounts and signs

A fixed coupon is defined by a notional \\(N\\), a rate \\(r\\), and a rate
definition. Its amount is \\(N(C(r,\tau)-1)\\), where \\(C\\) is the compound
factor. With simple compounding this is \\(Nr\tau\\).

A floating coupon uses an observed or projected index rate, plus a spread and
accrual conventions. Coupons with optionality also carry a cap, a floor, or a
more general payoff expression. Principal movements are separate cashflows:
redemptions pay principal back, and disbursements pay it out.

All cashflows in a leg share the same currency, side, projection index, and
optional discount index. The side is `LongReceive` (sign \\(+1\\)) or
`PayShort` (sign \\(-1\\)).

In a regular swap, leg zero is fixed and leg one is floating. The builder gives
the two legs opposite directions.

Both the instrument and the trade have direction fields. Each pricer and claim
converter applies the position direction according to its own conventions. If
you compute values yourself, apply the sign only once.

Cashflow reports show signed amounts: receipts are positive and payments are
negative.

- The generic report applies the leg direction and flips the sign of
  disbursements. An initial loan advance therefore has the opposite sign to
  the principal repayments.
- The loan pricer reports projected receipts after prepayment and credit
  adjustments.

## Schedules are economic inputs

`MakeSchedule` and the product builders turn a start date, maturity date,
payment frequency, calendar, business-day convention, and generation rule into
dated payments. Front and back stubs, end-of-month rules, and adjustment of the
termination date can change the amounts as well as the payment dates.

Each coupon has three kinds of date:

- an **accrual period**, which sets the accrual fraction;
- an **observation date** or schedule, which sets the fixings used;
- a **payment date**, which sets when the amount is paid.

These can differ. For example, a loan can pay monthly but reset against
EURIBOR once a year. An OIS coupon compounds daily overnight fixings, each with
its accrual weight. Its observation rules decide which fixings are used, and
its payment lag decides when the amount is paid.

`DayCounter` turns dates into accrual fractions. A curve can interpolate with a
different day counter from the coupon, but the projected rate is always
expressed on the coupon's basis. Calendar adjustment and day counting are
separate conventions, and you cannot read either from a quoted rate.

## Calendars

Generic builders usually default to a null calendar and unadjusted dates.
Market-specific builders may use presets:

- SOFR OIS uses the SOFR index calendar.
- SONIA uses London settlement holidays (`UnitedKingdom(Settlement)`).
- TONAR uses Japanese holidays (`Japan`).

A joint calendar combines several markets:

- `JoinHolidays`: a day is a business day only if every market is open.
- `JoinBusinessDays`: a day is a business day if at least one market is open.

Nested calendar definitions can be saved to and loaded from JSON. Holidays that
you add or remove at run time are used in calculations, but they are not saved
when the calendar is serialized.

Holiday rules follow history:

- London recurring holidays follow the rules in force since 1971.
- Japanese public holidays start in July 1948.
- Future Japanese equinox holidays are estimates until they are announced.

Exceptional closures need a library update or a local override.

## Builders and positions

Each `Make*` maker stores the contract inputs. It has two build methods:

- `MakeInstrument::build` uses only the terms you supplied.
- `build_using` also uses a reference date and market observations from a
  market data provider.

Both return a concrete instrument that you can use in trades and pricers. If
the maker needs market data you have not supplied, `build` returns
`UnresolvedProductErr`. See
[Building instruments explicitly](instrument-resolution.md).

Makers check the required terms and build the instrument. Check their defaults
against the market conventions of your contract.

- `MakeSwap` produces regular coupons with one fixing each.
- `MakeOvernightIndexSwap` starts from the overnight conventions of the chosen
  index.
- Bond and loan builders have their own principal, schedule, and optionality
  settings.

A trade adds the trade date, side, and position size, as supported by the
product. A bond trade also carries a clean price, dirty price, or spread used
to anchor its valuation. The discount curve is chosen by the pricer, not stored
as part of the contract.

For simulation, supported trades implement `IntoContingentClaims`. Each claim
is one scheduled payment, with instructions for valuing it and stable trade and
leg identifiers. Conversion fails if the exposure engine does not support a
feature of the contract. For example, the legacy claim converter rejects
non-standard overnight conventions.
