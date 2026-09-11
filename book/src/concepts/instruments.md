# Instruments and Trades

QuantSupport models a contract at three levels: **cashflows** grouped into **legs**, an **instrument** that owns one or more legs (or an option payoff), and a **trade** that adds the economic position. This chapter documents the building blocks and lists every instrument in the library with its builder and trade type.

## Cashflows

```rust,ignore
pub trait Cashflow<T: Scalar> {
    fn amount(&self) -> Result<T>;
    fn payment_date(&self) -> Date;
}

pub enum CashflowType<T: Scalar> {
    FixedRateCoupon(FixedRateCoupon<T>),            // notional × (compound(rate, accrual) − 1)
    FloatingRateCoupon(FloatingRateCoupon<T>),      // notional × (fixing + spread) × accrual
    OptionEmbeddedCoupon(OptionEmbeddedCoupon<T>),  // floating coupon with caplet/floorlet strikes
    Redemption(SimpleCashflow<f64>),                // principal repayment
    Disbursement(SimpleCashflow<f64>),              // principal paid out at start (loans/bonds)
    ConstantAmount(SimpleCashflow<f64>),
    OptionEmbeddedCashflow(OptionEmbeddedCashflow<T>),
}
```

`FixedRateCoupon` carries an `InterestRate<T>` (rate + `RateDefinition` = day counter, compounding, frequency); its `amount()` is `notional × (compound_factor − 1)`. `FloatingRateCoupon` stores fixing/accrual dates, the forward index and a spread; its amount is undefined until a fixing is supplied by the pricer, which is why `amount()` returns `Result`. `CashflowType<f64>` and `CashflowType<DualFwd>` convert into each other with `.into()`, so instruments built in `f64` can be priced with AAD.

## Legs

`Leg<T>` groups cashflows that share a currency, side and set of indices:

| Field                                                         | Meaning                                                              |
| ------------------------------------------------------------- | -------------------------------------------------------------------- |
| `id: usize`                                                   | leg identifier used by pricers (`CashflowsTable` rows, XVA `leg_id`) |
| `cashflows: Vec<CashflowType<T>>`                             | ordered cashflows                                                    |
| `currency: Currency`                                          | payment currency                                                     |
| `discount_index: Option<MarketIndex>`                         | explicit discount curve override                                     |
| `forward_index: Option<MarketIndex>`                          | index fixing floating coupons                                        |
| `spread: Option<T>`, `interest_rate: Option<InterestRate<T>>` | floating spread / fixed rate                                         |
| `side: Side`                                                  | `PayShort` or `LongReceive`                                          |
| `is_linear: bool`                                             | `false` when option-embedded coupons are present                     |
| `asset_class: AssetClass`                                     | `FixedIncome, InterestRate, Equity, Fx, Credit, Other`               |
| `first_payment_date`, `last_payment_date`                     | used by bootstrappers to order pillars                               |

### `MakeLeg`

Every multi-leg instrument builder delegates to `MakeLeg<T>`:

```rust,ignore
let leg = MakeLeg::<DualFwd>::default()
    .with_start_date(Date::new(2024, 1, 1))
    .with_end_date(Date::new(2025, 1, 1))        // or .with_tenor(Period::from_str("1Y")?)
    .with_notional(100_000.0)
    .with_rate(InterestRate::from_rate_definition(DualFwd::new(0.05),
        RateDefinition::new(DayCounter::Actual360, Compounding::Simple, Frequency::Annual)))
    .with_rate_type(RateType::Fixed)              // or RateType::Floating + with_forward_index / with_spread
    .with_side(Side::PayShort)
    .with_currency(Currency::USD)
    .with_payment_frequency(Frequency::Semiannual)
    .with_calendar(Some(Calendar::NullCalendar))
    .with_business_day_convention(Some(BusinessDayConvention::ModifiedFollowing))
    .with_date_generation_rule(Some(DateGenerationRule::Backward))
    .with_discount_index(Some(MarketIndex::SOFR))
    .bullet()
    .build()?;
```

Payment structures (`PaymentStructure`):

| Method                 | Structure                                                                              | Notes                               |
| ---------------------- | -------------------------------------------------------------------------------------- | ----------------------------------- |
| `.bullet()`            | coupons + single redemption at maturity                                                | default for swaps                   |
| `.equal_redemptions()` | principal amortised in equal amounts, coupons on outstanding notional                  |                                     |
| `.equal_payments()`    | constant coupon + principal instalments                                                | fixed legs only (error on floating) |
| `.zero()`              | one payment at maturity                                                                | forces `Frequency::Once`            |
| `.other()`             | custom `with_disbursements(HashMap<Date,f64>)` / `with_redemptions(HashMap<Date,f64>)` | forces `Frequency::OtherFrequency`  |

Optional extras: `with_first_coupon_date`, `with_end_of_month`, `with_leg_id`, `with_asset_class`, `with_caplet_strike`/`with_floorlet_strike` (turns a floating leg into option-embedded coupons; not allowed on fixed legs). `build()` fails with `ValueNotSetErr("Rate type")` and similar messages when a required field is missing, and with `InvalidValueErr` for inconsistent combinations.

## Instruments and trades

An instrument holds the legs and static terms; a trade wraps it with `trade_date`, `notional` and `Side`:

```rust,ignore
pub struct Swap<T: Scalar> { fixed_leg, floating_leg, forward_index, currency, ... }
impl<T: Scalar> Swap<T> {
    pub fn fixed_leg(&self) -> &Leg<T>;
    pub fn floating_leg(&self) -> &Leg<T>;
    pub fn forward_index(&self) -> MarketIndex;
    pub const fn currency(&self) -> Currency;
}

pub struct SwapTrade<T: Scalar> { instrument: Swap<T>, trade_date: Date, notional: f64, side: Side }
impl<T: Scalar> SwapTrade<T> {
    pub const fn new(instrument: Swap<T>, trade_date: Date, notional: f64, side: Side) -> Self;
    pub const fn notional(&self) -> f64;
}
```

`Side::LongReceive` means the trade receives the fixed leg (for swaps) / owns the instrument; `Side::PayShort` is the mirror. `Side::sign()` returns `+1.0`/`-1.0` and pricers multiply by it. Trades implement `Instrument` (`identifier()`), `Discountable` (asset class, currency, optional discount index) and, for exposure simulation, `IntoContingentClaims`.

### Catalogue

| Asset class  | Instrument                    | Builder                           | Trade                              | Deterministic pricer                                             |
| ------------ | ----------------------------- | --------------------------------- | ---------------------------------- | ---------------------------------------------------------------- |
| Rates        | `Swap` (fixed vs float)       | `MakeSwap`                        | `SwapTrade`                        | `DiscountedCashflowPricer`                                       |
| Rates        | `BasisSwap` (float vs float)  | `MakeBasisSwap`                   | `BasisSwapTrade`                   | `DiscountedCashflowPricer`                                       |
| Rates        | `FixFloatCrossCurrencySwap`   | `MakeFixFloatCrossCurrencySwap`   | `FixFloatCrossCurrencySwapTrade`   | `DiscountedCashflowPricer`                                       |
| Rates        | `FloatFloatCrossCurrencySwap` | `MakeFloatFloatCrossCurrencySwap` | `FloatFloatCrossCurrencySwapTrade` | `DiscountedCashflowPricer`                                       |
| Rates        | `CapFloor`                    | `MakeCapFloor`                    | `CapFloorTrade`                    | `ClosedFormBlackCapPricer`, `ClosedFormHullWhiteCapPricer`       |
| Rates        | `CapletFloorlet`              | — (from quotes)                   | `CapletFloorletTrade`              | `ClosedFormBlackCapletPricer`, `ClosedFormHullWhiteCapletPricer` |
| Rates        | `EuropeanSwaption`            | `MakeSwaption`                    | `EuropeanSwaptionTrade<DualFwd>`   | `ClosedFormHullWhiteSwaptionPricer`                              |
| Rates        | `RateFutures`                 | `MakeRateFutures`                 | `RateFuturesTrade`                 | `RateFuturesPricer`                                              |
| Fixed income | `FixedRateBond`               | `MakeFixedRateBond`               | `FixedRateBondTrade`               | `DiscountedCashflowPricer`                                       |
| Fixed income | `FloatingRateNote`            | `MakeFloatingRateNote`            | `FloatingRateNoteTrade`            | `DiscountedCashflowPricer`                                       |
| Fixed income | `FixedRateDeposit`            | `MakeFixedRateDeposit`            | `FixedRateDepositTrade`            | `DiscountedCashflowPricer`                                       |
| FX           | `FxForward`                   | `MakeFxForward`                   | `FxForwardTrade`                   | `FxForwardPricer`                                                |
| FX           | `FxOption`                    | `MakeFxOption`                    | `FxOptionTrade`                    | `FxOptionPricer` (Garman–Kohlhagen)                              |
| Equity       | `EquityForward`               | `MakeEquityForward`               | `EquityForwardTrade`               | — (claims / scripting)                                           |
| Equity       | `EquityEuropeanOption`        | —                                 | `EquityEuropeanOptionTrade`        | `BlackEuropeanOptionPricer`, `BlackMCEuropeanOptionPricer`       |
| Equity       | `Futures`                     | `MakeFutures`                     | `FuturesTrade`                     | — (claims / scripting)                                           |
| Credit       | `CreditDefaultSwap`           | —                                 | `CdsTrade`                         | `CdsPricer`                                                      |
| Any          | `ScriptedProduct`             | script text                       | —                                  | `ScriptEngine` (Monte Carlo)                                     |

The generic parameter `T` on rate/fixed-income instruments is `f64` or `DualFwd`; build in `f64` when you do not need rate sensitivities and convert with `.into()` when you do. FX, equity, cap/floor and credit instruments are non-generic and always price in `DualFwd` internally.

### Builder conventions

All `Make*` builders follow the same pattern as `MakeSwap` (see [Your First Swap](../getting-started/first-swap.md)):

- `Make*::new()` / `default()` then chained `with_*` setters that take ownership.
- `build()` returns `Result<Instrument>`; missing mandatory fields produce `QSError::ValueNotSetErr("<Field>")`.
- Defaults are conservative: `Calendar::NullCalendar`, `BusinessDayConvention::Unadjusted`, `DateGenerationRule::Backward`, `spread = 0.0`, `Side::LongReceive`.
- Cross-currency builders take two currencies, two notionals (or an FX rate to derive one) and per-leg indices; `MakeFxForward` and `MakeFxOption` take a `currency pair`, strike/forward rate and settlement date; `MakeCapFloor` takes an index, strike, cap/floor flag and schedule parameters; `MakeSwaption` wraps a `MakeSwap` plus expiry and settlement type.

The per-product chapters under [Pricing](../pricing/overview.md) show each builder with its required fields and the requests its pricer supports.

## Contingent claims

For simulation-based pricing every trade is decomposed into `ContingentClaim`s—atomic payments with a payment date, currency, leg id, side and a `ClaimEvaluationStrategy` (fixed amount, forward-rate coupon, option payoff, scripted payoff, …). `IntoContingentClaims::into_contingent_claims(&self, trade_id: &str) -> Result<Vec<ContingentClaim>>` performs the decomposition; the XVA engine, the scripting engine and `LgmMarketModel` all consume claims rather than instruments. See [Exposure](../simulation/exposure.md).
