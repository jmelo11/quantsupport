# Interest Rate Swaps

An interest-rate swap exchanges cashflows calculated under two rate conventions. Its valuation illustrates the central mechanics of the library: schedule construction, fixing lookup, forward projection, discount-policy selection, and quote-level sensitivity. This chapter develops those mechanics for fixed-versus-floating swaps, basis swaps, and related fixed-income instruments.

`Swap<T>` contains two `Leg` values. Leg zero is fixed and leg one is floating. `MakeSwap` constructs the schedules and coupons, `SwapTrade::new` adds trade metadata and direction, and `DiscountedCashflowPricer` performs valuation. [Your First Swap](../getting-started/first-swap.md) gives a complete introductory construction example.

## Valuation

The pricer values each future coupon in its own leg and applies the sign implied by the trade direction. At an evaluation date, the total present value is

\\[
\text{NPV} = \sum_{\text{legs}} \text{sign}(\text{leg}) \sum_i N\\,r_i\\,\tau_i\\,P_{d}(T_i)
\\]

- Fixed coupons: \\(r_i\\) from the `RateDefinition` (day counter, compounding, frequency).
- Floating coupons use a historical fixing when `accrual_start < evaluation_date`. Future coupons are projected from the forward curve of the leg's `market_index` with `forward_rate(start, end, Simple, frequency)`, after which the contractual spread is added.
- \\(P_d\\) is the discount factor of the curve selected by the discount policy (defaults to the leg's own index).

The formula separates projection from discounting. A floating coupon may obtain its rate from one curve and its present value from another curve chosen by the discount policy. This distinction is essential in a multi-curve market.

`Request::FairRate` returns the fixed rate that sets NPV to zero:

\\[
K^{\ast} = \frac{\text{PV}_{\text{float}}}{\text{Annuity}},\qquad \text{Annuity}=\sum_i N\\,\tau_i\\,P_d(T_i).
\\]

The fair rate is the floating-leg present value divided by the fixed-leg annuity. `Request::Cashflows` returns one `CashflowsTable` row per coupon, and `leg_indices()` identifies the fixed and floating legs.

## Fixings

Once a coupon has started accruing, its observed rate belongs to historical market data. `FixingStore` records those dated observations and can optionally interpolate gaps under an explicit rule. The following example supplies one SOFR fixing and attaches the store to a context:

```rust,ignore
let mut fixings = FixingStore::default();
fixings.add_fixing(&MarketIndex::SOFR, Date::new(2025, 5, 12), 0.0428);
fixings.fill_missing_fixings(Interpolator::Linear)?;   // optional gap filling
let ctx = PricingContext::new().with_fixing_store(fixings) /* ... */;
```

The equivalent JSON groups observations by index, for example `{"SOFR": [{"date": "2025-05-12", "rate": 0.0428}]}`. A seasoned swap whose current coupon began before the evaluation date returns `NotFoundErr` when the required fixing is absent. This failure prevents an observed coupon from being silently replaced with a forecast.

## Multi-curve swaps

The `examples/sensitivity` program demonstrates how projection and discounting combine across several indices. It bootstraps SOFR, Term SOFR 3M, ICP, and the collateralized CLP curve, then prices:

- a SOFR OIS swap,
- a Term SOFR swap projected on `TermSOFR3m` and discounted on SOFR through `SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD)`,
- an ICP (CLP) swap and cross-currency swaps.

The Term SOFR swap's sensitivity table contains both `BasisSwap_USD_SOFR_TermSOFR3m_*` and `OIS_USD_SOFR_*` rows. Its projected coupons depend on the basis curve, and that calibrated curve depends on SOFR through the implicit-function link described in [Curve Bootstrapping](../curves/bootstrapping.md).

## Basis swaps

A basis swap exchanges two floating indices and can carry a spread on either leg. It is both a trade in its own right and a common calibration instrument for projection curves. This example receives Term SOFR 3M plus its configured spread against SOFR:

```rust,ignore
let basis = MakeBasisSwap::<DualFwd>::default()
    .with_identifier("USD_SOFR_TSOFR3M_2Y".into())
    .with_start_date(rd).with_maturity_date(rd + Period::from_str("2Y")?)
    .with_notional(10_000_000.0)
    .with_currency(Currency::USD)
    .with_pay_market_index(MarketIndex::SOFR)
    .with_receive_market_index(MarketIndex::TermSOFR3m)
    .with_pay_spread(0.0).with_receive_spread(-0.0012)
    .with_pay_leg_frequency(Frequency::Quarterly)
    .with_receive_leg_frequency(Frequency::Quarterly)
    .build()?;
let trade = BasisSwapTrade::new(basis, rd, 10_000_000.0, Side::LongReceive);
```

The builder requires notional, dates, currency, both market indices, and an identifier. Spreads default to zero, both frequencies default to quarterly, and the side defaults to `LongReceive`. `DiscountedCashflowPricer` values the two generated legs under the chosen discount policy.

## Fixed-income instruments

The cashflow pricer also supports deposits, bonds, floating-rate notes, and listed rate futures through their respective trade wrappers. Their builders differ mainly in required conventions and default payment structures:

| Builder                   | Required                                                                         | Defaults                                                                          |
| ------------------------- | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `MakeFixedRateBond<T>`    | notional, start_date, maturity_date, rate, rate_definition, currency, identifier | units 100, side LongReceive, frequency Semiannual, `PaymentStructure::Bullet`     |
| `MakeFloatingRateNote<T>` | notional, start_date, maturity_date, forward_index, currency, identifier         | spread 0, units 100, frequency Quarterly, Bullet                                  |
| `MakeFixedRateDeposit<T>` | notional, start_date, maturity_date, rate, rate_definition, currency, identifier | units 100, single payment                                                         |
| `MakeRateFutures`         | identifier, market_index, start_date, end_date, futures_price                    | contract size 2500 and rate definition from the index. Uses `RateFuturesPricer`   |

`PaymentStructure` determines how principal is repaid and provides `Bullet`, `EqualPayments`, `EqualRedemptions`, `Zero`, and `Other`. A bond may also declare its own `discount_index`. `FixedIncomeDiscountPolicy` can give that issuer curve priority or select a configured risk-free curve.

## What to remember

Swap valuation is a sequence of explicit decisions. Contract schedules determine the cashflows, fixing dates determine whether rates are observed or projected, market indices choose projection curves, and the discount policy chooses present-value curves. Because all of those inputs retain their quote links, the resulting risk report explains both direct and cross-curve exposure.
