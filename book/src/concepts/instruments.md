# Instruments and Trades

QuantSupport models a contract at three levels: **cashflows** grouped into **legs**, an **instrument** that owns one or more legs (or an option payoff), and a **trade** that adds the economic position. This chapter documents the building blocks and lists every instrument in the library with its builder and trade type.

## Cashflows

Cashflows are the smallest contractual units in the library. Every cashflow
knows when it pays. Its variant determines how the amount is obtained.
The trait and enum below show the common interface and the supported economic
rules.

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

The enum keeps product-specific amount logic typed. A pricer can therefore
visit fixed, floating, optional, and principal cashflows and retain the
information needed to resolve rates or option payoffs.

`FixedRateCoupon` carries an `InterestRate<T>`, whose `RateDefinition` provides the day counter, compounding, and frequency. Its `amount()` is `notional × (compound_factor − 1)`. `FloatingRateCoupon` stores fixing and accrual dates, the forward index, and a spread. Its `amount()` returns `Result` because the pricer must first supply a fixing or projection. `CashflowType<f64>` and `CashflowType<DualFwd>` convert into each other with `.into()`, allowing instruments built in `f64` to enter AAD pricing.

## Legs

`Leg<T>` groups cashflows that share a currency, side and set of indices:

The shared fields explain how a stream is projected, discounted, signed, and
reported. They also let a pricer prepare market data once for the complete
stream.

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

Together, these fields turn individual payments into a coherent side of a
contract. Multi-leg products then combine streams with complementary signs and
possibly different currencies or indices.

### `MakeLeg`

Every multi-leg instrument builder delegates to `MakeLeg<T>`. The builder is
shown in detail because its scheduling and payment choices also explain the
behavior of swap, bond, and cross-currency builders.

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

The chain separates contractual choices from schedule generation. Required
fields define the economics. Calendar and date-generation fields control
how those economics are placed on actual dates.

Payment structures (`PaymentStructure`):

| Method                 | Structure                                                                              | Notes                               |
| ---------------------- | -------------------------------------------------------------------------------------- | ----------------------------------- |
| `.bullet()`            | coupons + single redemption at maturity                                                | default for swaps                   |
| `.equal_redemptions()` | principal amortised in equal amounts, coupons on outstanding notional                  |                                     |
| `.equal_payments()`    | constant coupon + principal instalments                                                | fixed legs only (error on floating) |
| `.zero()`              | one payment at maturity                                                                | forces `Frequency::Once`            |
| `.other()`             | custom `with_disbursements(HashMap<Date,f64>)` / `with_redemptions(HashMap<Date,f64>)` | forces `Frequency::OtherFrequency`  |

Optional settings include `with_first_coupon_date`, `with_end_of_month`, `with_leg_id`, `with_asset_class`, `with_caplet_strike`, and `with_floorlet_strike`. Strike settings create option-embedded floating coupons. `build()` returns `ValueNotSetErr("Rate type")` and similar messages for a missing required field, and `InvalidValueErr` for an inconsistent combination.

## Instruments and trades

An instrument holds the legs and static terms. A trade wraps it with `trade_date`, `notional`, and `Side`:

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

For a swap, `Side::LongReceive` receives the fixed leg and `Side::PayShort` pays it. `Side::sign()` returns `+1.0` or `-1.0`, which pricers apply to value. Trades implement `Instrument`, `Discountable`, and `IntoContingentClaims` for exposure simulation.

### Catalogue

The catalogue connects each contract representation to its normal builder,
trade wrapper, and deterministic valuation route. It is a map for choosing the
next detailed pricing chapter.

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

Products without a deterministic pricer still participate in claim-based or
scripted workflows. The final column records the currently implemented route
for each contract.

The generic parameter `T` on rate and fixed-income instruments is `f64` or `DualFwd`. Use `f64` for value calculations and convert with `.into()` for AAD pricing. FX, equity, cap, floor, and credit instruments use `DualFwd` internally.

### Builder conventions

All `Make*` builders follow the same pattern as `MakeSwap` (see [Your First Swap](../getting-started/first-swap.md)):

- `Make*::new()` / `default()` then chained `with_*` setters that take ownership.
- `build()` returns `Result<Instrument>`. Missing mandatory fields produce `QSError::ValueNotSetErr("<Field>")`.
- Defaults are conservative: `Calendar::NullCalendar`, `BusinessDayConvention::Unadjusted`, `DateGenerationRule::Backward`, `spread = 0.0`, `Side::LongReceive`.
- Cross-currency builders take two currencies, two notionals or an FX rate, and per-leg indices. `MakeFxForward` and `MakeFxOption` take a currency pair, strike or forward rate, and settlement date. `MakeCapFloor` takes an index, strike, product direction, and schedule parameters. `MakeSwaption` wraps swap terms with expiry and settlement type.

The per-product chapters under [Pricing](../pricing/overview.md) show each builder with its required fields and the requests its pricer supports.

## Contingent claims

For simulation-based pricing, every trade is decomposed into `ContingentClaim` values. Each claim is an atomic payment with a payment date, currency, leg id, side, and a `ClaimEvaluationStrategy` such as a fixed amount, forward-rate coupon, option payoff, or scripted payoff. `IntoContingentClaims::into_contingent_claims(&self, trade_id: &str) -> Result<Vec<ContingentClaim>>` performs the decomposition. The XVA engine, scripting engine, and `LgmMarketModel` consume this common claim representation. See [Exposure](../simulation/exposure.md).

## From contract to valuation

The hierarchy now has a clear direction. Cashflows define amounts and dates,
legs organize related payments, instruments define a product, and trades add
the held position. Deterministic pricers consume trades directly. Simulation
workflows translate them into contingent claims. Both routes begin
from the same contractual definition, which keeps pricing and exposure aligned.
