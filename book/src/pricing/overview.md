# Pricing Overview

Every pricer implements the `Pricer` trait from `src/core`:

```rust,ignore
pub trait Pricer {
    type Item;                         // the trade type
    type Policy: ?Sized;               // usually dyn DiscountPolicy
    fn evaluate(&self, trade: &Self::Item, requests: &[Request], ctx: &impl MarketDataProvider) -> Result<EvaluationResults>;
    fn market_data_request(&self, trade: &Self::Item) -> Option<MarketDataRequest>;
    fn set_discount_policy(&mut self, policy: Box<Self::Policy>);
    fn discount_policy(&self) -> Option<&Self::Policy>;
}
```

`evaluate` asks the `PricingContext` (a `MarketDataProvider`) for exactly the elements listed by `market_data_request` — discount curves per index, volatility surfaces/cubes, FX pairs, fixings — then prices on the AD tape. All results for one call share one forward pass.

## Pricer catalogue

| Pricer                                                 | Trade type                                                                                                              | Requests                                  | Market data                                                       | Model                                      |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------ |
| `DiscountedCashflowPricer<I, T>::new()`                | any `T: LegsProvider<DualFwd> + Trade<I>` (swaps, basis swaps, XCCY swaps, bonds, FRNs, deposits, FX forwards via legs) | Value, FairRate, Cashflows, Sensitivities | discount curve per leg index, FX for cross-currency legs, fixings | \\(\sum_i CF_i\\,P(T_i)\\)                 |
| `CdsPricer::new()`                                     | `CdsTrade`                                                                                                              | Value, FairRate, Sensitivities            | credit curve `MarketIndex::Credit(name)`, discount curve          | premium/protection legs on survival curve  |
| `BlackEuropeanOptionPricer::new()`                     | `EquityEuropeanOptionTrade`                                                                                             | Value, Sensitivities                      | spot, equity surface, discount curve, dividend                    | Black-Scholes                              |
| `BlackMCEuropeanOptionPricer::new()`                   | `EquityEuropeanOptionTrade`                                                                                             | Value, Sensitivities                      | a `SimulationConfiguration`-generated path set                    | \\(P(T)\\,\mathbb E[\text{payoff}(S_T)]\\) |
| `FxForwardPricer::new()`                               | `FxForwardTrade`                                                                                                        | Value, FairRate, Sensitivities            | base/quote discount curves, spot                                  | \\(F = S\\,P*{base}/P*{quote}\\)           |
| `FxOptionPricer::new()`                                | `FxOptionTrade`                                                                                                         | Value, Sensitivities                      | base/quote curves, spot, FX surface                               | Garman-Kohlhagen                           |
| `ClosedFormBlackCapletPricer::new()`                   | `CapletFloorletTrade`                                                                                                   | Value, Sensitivities                      | forward curve, surface at (fixing, strike)                        | Black-76                                   |
| `ClosedFormBlackCapPricer::new()`                      | `CapFloorTrade`                                                                                                         | Value, Sensitivities                      | same                                                              | sum of Black-76 caplets                    |
| `ClosedFormHullWhiteCapletPricer::new(alpha, sigma)`   | `CapletFloorletTrade`                                                                                                   | Value, Sensitivities                      | discount curve                                                    | bond-put representation                    |
| `ClosedFormHullWhiteCapPricer::new(alpha, sigma)`      | `CapFloorTrade`                                                                                                         | Value, Sensitivities                      | discount curve                                                    | sum of HW caplets                          |
| `ClosedFormHullWhiteSwaptionPricer::new(alpha, sigma)` | `EuropeanSwaptionTrade<T>`                                                                                              | Value, Sensitivities                      | discount curve                                                    | Jamshidian                                 |
| `RateFuturesPricer::new()`                             | `RateFuturesTrade`                                                                                                      | Value, Sensitivities                      | curve of `market_index`                                           | \\(Q = 100 - 100F\\)                       |

`Request::YieldToMaturity` and `Request::ModifiedDuration` exist in the enum but no public pricer currently fills them.

## Discount policies

A pricer discounts each leg with the curve returned by its `DiscountPolicy`:

```rust,ignore
pub trait DiscountPolicy {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex>;
    fn discount_indices(&self) -> Vec<MarketIndex>;
}
```

| Policy                                                                                     | Behaviour                                                                                                                                                            |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SingleCurveCSADiscountPolicy::new(discount_index, currency)`                              | legs in `currency` discount on `discount_index`; legs in another currency discount on `MarketIndex::Collateral(leg_ccy, currency)` — the FX-implied collateral curve |
| `FixedIncomeDiscountPolicy::new(prefer_instrument_index).with_risk_free_index(ccy, index)` | bonds/deposits use their own `discount_index` when `prefer_instrument_index` and one is set, otherwise the risk-free index registered for their currency             |

Without a policy `DiscountedCashflowPricer` discounts every leg on its own forward index. With a `Collateral(..)` index the pricer converts the cashflow to the collateral currency with the context FX store and discounts on the collateral curve.

```rust,ignore
let mut pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD)));
```

## Results

`EvaluationResults` collects `price()`, `fair_rate()`, `sensitivities()`, `cashflows()`. Sensitivities are computed by one reverse sweep from the price to the quote leaves of every curve/surface used, then labelled with the quote identifiers (`OIS_USD_SOFR_5Y`, `CapletFloorlet_..._Black`). Duplicate labels coming from chained curves are merged with `SensitivityMap::aggregate()`.

## Type-erased dispatch

When a portfolio mixes trade types, register pricers in an `Evaluator`:

```rust,ignore
let mut pricers: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
pricers.insert(TypeId::of::<SwapTrade<DualFwd>>(), Box::new(DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()));
pricers.insert(TypeId::of::<FxOptionTrade>(), Box::new(FxOptionPricer::new()));
let evaluator = Evaluator::new(pricers);
let results = evaluator.evaluate(&trade as &dyn Any, &[Request::Value], &context)?;
```

`examples/evaluator` (`cargo run -p evaluator`) shows this pattern.
