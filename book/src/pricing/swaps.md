# Interest Rate Swaps

`Swap<T>` is two `Leg`s: leg 0 fixed, leg 1 floating. Build it with `MakeSwap` ([Your First Swap](../getting-started/first-swap.md) lists every builder field and default), wrap it in `SwapTrade::new(swap, trade_date, notional, side)` and price with `DiscountedCashflowPricer::<Swap<T>, SwapTrade<T>>::new()`.

## Valuation

For each coupon the pricer computes

\\[
\text{NPV} = \sum_{\text{legs}} \text{sign}(\text{leg}) \sum_i N\\,r_i\\,\tau_i\\,P_{d}(T_i)
\\]

- Fixed coupons: \\(r_i\\) from the `RateDefinition` (day counter, compounding, frequency).
- Floating coupons: if `accrual_start < evaluation_date` the rate is read from the `FixingStore` (`state.get_fixing(index, accrual_start)`), otherwise it is projected from the forward curve of the leg's `market_index` with `forward_rate(start, end, Simple, frequency)`; the `spread` is added afterwards.
- \\(P_d\\) is the discount factor of the curve selected by the discount policy (defaults to the leg's own index).

`Request::FairRate` returns the fixed rate that sets NPV to zero:

\\[
K^{\ast} = \frac{\text{PV}_{\text{float}}}{\text{Annuity}},\qquad \text{Annuity}=\sum_i N\\,\tau_i\\,P_d(T_i).
\\]

`Request::Cashflows` returns the `CashflowsTable` with one row per coupon (`leg_indices()` distinguishes fixed/floating).

## Fixings

```rust,ignore
let mut fixings = FixingStore::default();
fixings.add_fixing(&MarketIndex::SOFR, Date::new(2025, 5, 12), 0.0428);
fixings.fill_missing_fixings(Interpolator::Linear)?;   // optional gap filling
let ctx = PricingContext::new().with_fixing_store(fixings) /* ... */;
```

JSON: `{"SOFR": [{"date": "2025-05-12", "rate": 0.0428}, ...]}`. A seasoned swap whose current coupon started before the evaluation date fails with `NotFoundErr` if the fixing is missing.

## Multi-curve swaps

`examples/sensitivity` (`cargo run -p sensitivity`) bootstraps SOFR, TermSOFR3m, ICP and the CLP collateral curve, then prices:

- a SOFR OIS swap,
- a Term SOFR swap projected on `TermSOFR3m` and discounted on SOFR through `SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD)`,
- an ICP (CLP) swap and cross-currency swaps.

The sensitivity table for the Term SOFR swap contains both `BasisSwap_USD_SOFR_TermSOFR3m_*` and `OIS_USD_SOFR_*` rows because the basis curve depends on the SOFR curve through the IFT link described in [Curve Bootstrapping](../curves/bootstrapping.md).

## Basis swaps

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

Required: `notional`, `start_date`, `maturity_date`, `currency`, `pay_market_index`, `receive_market_index`, `identifier`. Defaults: spreads `0.0`, both frequencies `Quarterly`, side `LongReceive`. Priced with `DiscountedCashflowPricer::<BasisSwap<T>, BasisSwapTrade<T>>`.

## Fixed-income instruments

The same pricer handles the fixed-income builders:

| Builder                   | Required                                                                         | Defaults                                                                          |
| ------------------------- | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `MakeFixedRateBond<T>`    | notional, start_date, maturity_date, rate, rate_definition, currency, identifier | units 100, side LongReceive, frequency Semiannual, `PaymentStructure::Bullet`     |
| `MakeFloatingRateNote<T>` | notional, start_date, maturity_date, forward_index, currency, identifier         | spread 0, units 100, frequency Quarterly, Bullet                                  |
| `MakeFixedRateDeposit<T>` | notional, start_date, maturity_date, rate, rate_definition, currency, identifier | units 100, single payment                                                         |
| `MakeRateFutures`         | identifier, market_index, start_date, end_date, futures_price                    | contract_size 2500, rate definition from the index; priced by `RateFuturesPricer` |

`PaymentStructure` variants: `Bullet`, `EqualPayments`, `EqualRedemptions`, `Zero`, `Other`. Bonds may carry their own `discount_index`, honoured by `FixedIncomeDiscountPolicy`.
