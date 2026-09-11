# Rust API

This chapter maps the vocabulary used throughout the book to the traits and structs in the crate. Everything below is re-exported by `quantsupport::prelude`.

## The scalar parameter

```rust,ignore
pub trait Scalar: Copy + Add + Sub + Mul + Div + Neg + PartialOrd + From<f64> + ... {
    fn scalar(v: f64) -> Self;
    fn value(&self) -> f64;
    fn zero() -> Self; fn one() -> Self;
    fn exp(self) -> Self; fn ln(self) -> Self; fn sqrt(self) -> Self; fn powf(self, e: Self) -> Self; ...
}
```

Implemented for `f64`, `Fwd<T>` (forward mode), `Dual<T>` (reverse mode), and the alias `DualFwd = Dual<Fwd<Fwd<f64>>>`. Curves, instruments, pricers and models are generic over `T: Scalar`; the choice determines whether results carry derivatives. See [Automatic Differentiation](../risk/aad.md).

## Market data

| Type                                                               | Purpose                                                                                                  |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| `QuoteStore`                                                       | Quotes keyed by identifier with a `reference_date()`; `add_quote`, `quote(id)`, `quotes()`               |
| `Quote`, `QuoteDetails`, `QuoteInstrument`, `QuoteLevels`, `Level` | One quote: parsed identifier, instrument kind, `mid`/`bid`/`ask`                                         |
| `FixingStore`                                                      | Historical fixings per `MarketIndex`                                                                     |
| `FxStore`, `FxRateRecord`                                          | Spot FX rates with triangulation                                                                         |
| `Scenario`, `ScenarioType`                                         | Absolute/relative quote shocks                                                                           |
| `QuoteSelector`                                                    | Trait implemented by `QuoteStore` (and shocked stores) used by bootstrappers to read values at a `Level` |

## Constructed elements and context

| Type                                                                                             | Purpose                                                                                                                                           |
| ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PricingContext`                                                                                 | Inputs + configurations + `initialize()` + accessors; implements `MarketDataProvider`                                                             |
| `ConstructedElementStore`                                                                        | Built objects: `discount_curves()`, `credit_curves()`, `volatility_surfaces()`, `volatility_cubes()`, `simulations()` (each with `_mut` variants) |
| `DiscountCurveElement`, `VolatilitySurfaceElement`, `VolatilityCubeElement`, `SimulationElement` | Wrappers holding the index plus `Rc<RefCell<..>>` of the object                                                                                   |
| `MarketDataProvider`, `MarketDataRequest`                                                        | Trait a pricer uses to fetch what it needs (discount factors, forwards, fixings, FX, vols)                                                        |

## Instruments and trades

```rust,ignore
pub trait Instrument { fn identifier(&self) -> &str; /* legs, currency, ... */ }

pub trait Trade<I: Instrument>: Send + Sync {
    fn instrument(&self) -> &I;
    fn trade_date(&self) -> Date;
    fn side(&self) -> Side;
}

pub enum Side { PayShort, LongReceive }
impl Side { pub const fn sign(&self) -> f64 }   // -1.0 / +1.0
```

Every instrument has a builder named `Make<Instrument>` (`MakeSwap`, `MakeBasisSwap`, `MakeCapFloor`, `MakeSwaption`, `MakeFixFloatCrossCurrencySwap`, `MakeFloatFloatCrossCurrencySwap`, `MakeFxForward`, `MakeFxOption`, `MakeEquityForward`, `MakeFutures`, `MakeFixedRateBond`, `MakeFixedRateDeposit`, `MakeFloatingRateNote`, `MakeRateFutures`, `MakeContingentClaim`) following the pattern `::default().with_*(..).build()?`. Every instrument has a matching `<Instrument>Trade::new(instrument, trade_date, notional, side)`.

`AssetClass { FixedIncome, InterestRate, Equity, Fx, Credit, Other }` tags instruments for reporting.

## Pricers and results

```rust,ignore
pub trait Pricer: Send + Sync {
    type Item;                                 // the trade type
    type Policy: ?Sized + Send + Sync;         // the DiscountPolicy family accepted

    fn evaluate(&self, trade: &Self::Item, requests: &[Request], ctx: &impl MarketDataProvider)
        -> Result<EvaluationResults>;
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest>;
    fn set_discount_policy(&mut self, policy: Box<Self::Policy>);
    fn discount_policy(&self) -> Option<&Self::Policy>;
}

pub enum Request { Value, YieldToMaturity, ModifiedDuration, Sensitivities, Cashflows, FairRate }
```

`EvaluationResults` getters: `price() -> Option<f64>`, `fair_rate() -> Option<f64>`, `ytm()`, `modified_duration()`, `sensitivities() -> Option<&SensitivityMap>`, `cashflows() -> Option<&CashflowsTable>`. Pricers return an error for a `Request` they do not support; the per-pricer tables in the [Pricing](../pricing/overview.md) part list what each accepts.

### Type-erased dispatch

```rust,ignore
let mut pricers: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
pricers.insert(
    TypeId::of::<SwapTrade<DualFwd>>(),
    Box::new(DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()),
);
pricers.insert(
    TypeId::of::<FxForwardTrade<DualFwd>>(),
    Box::new(FxForwardPricer::new()),
);
let evaluator = Evaluator::new(pricers);

let results = evaluator.evaluate(&swap_trade as &dyn Any, &[Request::Value], &context)?;
```

`Evaluator::evaluate(&dyn Any, &[Request], &PricingContext)` looks up the pricer by the trade's `TypeId` and returns `QSError::NotFoundErr("No pricer registered for trade type: ..")` when none is registered. Every `Pricer` automatically implements `ErasedPricer`.

### Discount policies

```rust,ignore
pub trait Discountable {
    fn asset_class(&self) -> AssetClass;
    fn discount_index(&self) -> Option<MarketIndex> { None }
    fn currency(&self) -> Currency;
}

pub trait DiscountPolicy: Send + Sync {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex>;  // which curve discounts this leg/instrument
    fn discount_indices(&self) -> Vec<MarketIndex>;                       // every curve the policy may need (for bootstrapping)
}
```

- `SingleCurveCSADiscountPolicy::new(discount_index, currency)` returns `discount_index` for targets in `currency` and `MarketIndex::Collateral(target_ccy, currency)` otherwise, i.e. it asks for an FX-implied collateral curve for foreign legs.
- `FixedIncomeDiscountPolicy::new(prefer_instrument_index).with_risk_free_index(ccy, idx)` applies to `AssetClass::FixedIncome` only: it uses the instrument's own `discount_index()` when `prefer_instrument_index` is true and one is set, otherwise the risk-free index configured for the currency (error `InvalidValueErr("No risk-free index configured for currency ..")` if missing).

Without a policy, `DiscountedCashflowPricer` discounts each leg on the curve of the leg's own index or the context's base index. Policies are how collateralised (CSA) discounting is expressed; the XVA `NettingSet` also carries one.

## Curves and volatility

```rust,ignore
pub trait InterestRatesTermStructure<T: Scalar> {
    fn reference_date(&self) -> Date;
    fn discount_factor(&self, date: Date) -> Result<T>;
    fn discount_factor_from_time(&self, t: f64) -> Result<T>;
    fn forward_rate(&self, start: Date, end: Date, comp: Compounding, freq: Frequency) -> Result<T>;
    fn forward_rate_from_time(&self, t1: f64, t2: f64, comp: Compounding, freq: Frequency) -> Result<T>;
    fn nodes(&self) -> Vec<(Date, T)>;
    fn day_counter(&self) -> DayCounter;
}
```

Implementations: `DiscountTermStructure<T>` (interpolated discount factors) and `FlatForwardTermStructure<T>`. Volatility objects implement `volatility_from_period(expiry, key)` (surface) and `volatility_from_period(expiry, tenor, key)` (cube). See [Yield Curves](../curves/overview.md) and [Volatility Surfaces](../curves/volatility.md).

## Errors

```rust,ignore
pub type Result<T> = std::result::Result<T, QSError>;
```

`QSError` variants: `NotFoundErr`, `DateParsingErr`, `PeriodParsingErr`, `PeriodOperationErr`, `MakeScheduleErr`, `EvaluationErr`, `SerializationErr`, `DeserializationErr`, `ValueNotSetErr` (missing builder field or result), `InvalidValueErr`, `SolverErr`, `NotImplementedErr`, `InterpolationErr`, `NodeError`, `TapeError`, `DualFwdError`, `UnexpectedErr`, `QuoteParsingErr`, `InstrumentResolutionErr`. `QSError` implements `std::error::Error` via `thiserror`, so `?` works in `main() -> Result<(), Box<dyn Error>>`.

## Time utilities

- `Date::new(y, m, d)`, `Date::from_str("2025-01-01", "%Y-%m-%d")`, `date + Period`, `date.advance(n, TimeUnit)`, `Date::empty()`
- `Period::from_str("3M")`, `Period::new(n, TimeUnit)`; `TimeUnit { Days, Weeks, Months, Years }`
- `Frequency { NoFrequency, Once, Annual, Semiannual, EveryFourthMonth, Quarterly, Bimonthly, Monthly, EveryFourthWeek, Biweekly, Weekly, Daily, OtherFrequency }` (discriminant = payments per year) with `FromStr`
- `DayCounter { Actual360, Actual365, Thirty360, Thirty360US, ActualActual, Business252 }` with `year_fraction(start, end)` and `day_count(start, end)`
- `Calendar { NullCalendar, WeekendsOnly, TARGET, UnitedStates, Brazil, Chile }`
- `BusinessDayConvention { Following, ModifiedFollowing, Preceding, ModifiedPreceding, Unadjusted, HalfMonthModifiedFollowing, Nearest }`
- `DateGenerationRule { Backward, Forward, Zero, ThirdWednesday, ThirdWednesdayInclusive, Twentieth, TwentiethIMM, OldCDS, CDS, CDS2015 }`
- `MakeSchedule` for custom coupon schedules; IMM date helpers under `time::imm`.
