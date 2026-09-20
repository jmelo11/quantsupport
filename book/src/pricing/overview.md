# Pricing Overview

Pricing connects a trade's contractual cashflows to the curves, fixings, FX rates, volatilities, and models held by a market context. This chapter introduces the common pricer contract, surveys the available implementations, and explains how discount policies, result requests, and portfolio dispatch fit together.

Every pricer implements the `Pricer` trait from `src/core`. The trait separates four responsibilities: valuing a trade, declaring required market data, accepting a discount policy, and exposing the active policy. Its definition is:

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

`evaluate` obtains the elements listed by `market_data_request` from a `MarketDataProvider`, which is commonly a `PricingContext`. Depending on the product, these elements include discount curves, volatility markets, FX pairs, and historical fixings. The requested outputs share one forward valuation pass, which keeps price and risk internally consistent.

## Pricer catalogue

Each pricer specializes the common contract for one payoff family and one valuation method. The catalogue below is a guide to the supported trade types, result requests, and required market elements:

| Pricer                                                 | Trade type                                                                                                              | Requests                                  | Market data                                                       | Model                                      |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------ |
| `DiscountedCashflowPricer<I, T>::new()`                | any `T: LegsProvider<DualFwd> + Trade<I>` (swaps, basis swaps, XCCY swaps, bonds, FRNs, deposits, FX forwards via legs) | Value, FairRate, Cashflows, Sensitivities | discount curve per leg index, FX for cross-currency legs, fixings | \\(\sum_i CF_i\\,P(T_i)\\)                 |
| `CdsPricer::new()`                                     | `CdsTrade`                                                                                                              | Value, FairRate, Sensitivities            | credit curve `MarketIndex::Credit(name)`, discount curve          | premium/protection legs on survival curve  |
| `BlackEuropeanOptionPricer::new()`                     | `EquityEuropeanOptionTrade`                                                                                             | Value, Sensitivities                      | spot, equity surface, discount curve, dividend                    | Black-Scholes                              |
| `BlackMCEuropeanOptionPricer::new()`                   | `EquityEuropeanOptionTrade`                                                                                             | Value, Sensitivities                      | a `SimulationConfiguration`-generated path set                    | \\(P(T)\\,\mathbb E[\text{payoff}(S_T)]\\) |
| `FxForwardPricer::new()`                               | `FxForwardTrade`                                                                                                        | Value, FairRate, Sensitivities            | base/quote discount curves, spot                                  | \\(F = S\\,P_{base}/P_{quote}\\)           |
| `FxOptionPricer::new()`                                | `FxOptionTrade`                                                                                                         | Value, Sensitivities                      | base/quote curves, spot, FX surface                               | Garman-Kohlhagen                           |
| `ClosedFormBlackCapletPricer::new()`                   | `CapletFloorletTrade`                                                                                                   | Value, Sensitivities                      | forward curve, surface at (fixing, strike)                        | Black-76                                   |
| `ClosedFormBlackCapPricer::new()`                      | `CapFloorTrade`                                                                                                         | Value, Sensitivities                      | same                                                              | sum of Black-76 caplets                    |
| `ClosedFormHullWhiteCapletPricer::new(alpha, sigma)`   | `CapletFloorletTrade`                                                                                                   | Value, Sensitivities                      | discount curve                                                    | bond-put representation                    |
| `ClosedFormHullWhiteCapPricer::new(alpha, sigma)`      | `CapFloorTrade`                                                                                                         | Value, Sensitivities                      | discount curve                                                    | sum of HW caplets                          |
| `ClosedFormHullWhiteSwaptionPricer::new(alpha, sigma)` | `EuropeanSwaptionTrade<T>`                                                                                              | Value, Sensitivities                      | discount curve                                                    | Jamshidian                                 |
| `RateFuturesPricer::new()`                             | `RateFuturesTrade`                                                                                                      | Value, Sensitivities                      | curve of `market_index`                                           | \\(Q = 100 - 100F\\)                       |

The table also shows that market-data needs belong to the valuation method. A closed-form option pricer requests a volatility surface. A Monte Carlo pricer requests simulated paths. `Request::YieldToMaturity` and `Request::ModifiedDuration` are reserved in the request enum for specialized fixed-income implementations.

## Discount policies

A pricer must translate the economic terms of a leg into a discount-curve identity. A `DiscountPolicy` owns that decision and can apply collateral or issuer rules consistently across products. The interface is deliberately small:

```rust,ignore
pub trait DiscountPolicy {
    fn accept(&self, target: &dyn Discountable) -> Result<MarketIndex>;
    fn discount_indices(&self) -> Vec<MarketIndex>;
}
```

| Policy                                                                                     | Behavior                                                                                                                                                            |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SingleCurveCSADiscountPolicy::new(discount_index, currency)`                              | legs in `currency` discount on `discount_index`. Legs in another currency use `MarketIndex::Collateral(leg_ccy, currency)`, the FX-implied collateral curve            |
| `FixedIncomeDiscountPolicy::new(prefer_instrument_index).with_risk_free_index(ccy, index)` | bonds/deposits use their own `discount_index` when `prefer_instrument_index` and one is set, otherwise the risk-free index registered for their currency             |

When a `DiscountedCashflowPricer` has no explicit policy, it attempts to infer a curve from the leg's indices. An explicit policy is preferable for production because it records the collateral or fixed-income convention in the valuation setup. If the selected index is `Collateral(..)`, the pricer converts the cashflow with the context FX store and discounts it on the collateral-adjusted curve.

The following setup installs a USD SOFR CSA policy on a swap pricer:

```rust,ignore
let mut pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
pricer.set_discount_policy(Box::new(SingleCurveCSADiscountPolicy::new(MarketIndex::SOFR, Currency::USD)));
```

The installed policy is reused for every leg valued by this pricer. Its explicit presence also makes the collateral convention visible during review and testing.

## Results

`EvaluationResults` collects the outputs produced for a request set. Its accessors expose price, fair rate, sensitivities, and detailed cashflows. A caller should request only the measures needed for that operation, then inspect the corresponding optional result.

Sensitivities come from one reverse sweep from the price to the quote leaves of every curve and volatility object used. The report labels them with market identifiers such as `OIS_USD_SOFR_5Y` and `CapletFloorlet_..._Black`. `SensitivityMap::aggregate()` combines repeated labels that arrive through multiple dependency paths.

## Type-erased dispatch

A concrete Rust pricer retains its trade type at compile time. A heterogeneous portfolio needs runtime dispatch because successive entries may be swaps, options, bonds, or other products. `Evaluator` provides that boundary by associating each trade `TypeId` with an erased pricer. The following example registers swap and FX-option handlers:

```rust,ignore
let mut pricers: HashMap<TypeId, Box<dyn ErasedPricer>> = HashMap::new();
pricers.insert(TypeId::of::<SwapTrade<DualFwd>>(), Box::new(DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new()));
pricers.insert(TypeId::of::<FxOptionTrade>(), Box::new(FxOptionPricer::new()));
let evaluator = Evaluator::new(pricers);
let results = evaluator.evaluate(&trade as &dyn Any, &[Request::Value], &context)?;
```

At evaluation time, the registry selects the entry matching the concrete trade and delegates the same request set and context. An absent registration becomes an explicit dispatch error. The `examples/evaluator` program shows this pattern with a mixed portfolio.

## What to remember

The `Pricer` trait gives every valuation method one consistent lifecycle: declare market needs, resolve them from the context, evaluate requested measures, and return structured results. Discount policies make curve selection visible. The erased evaluator extends the same design to mixed portfolios.

The remaining pricing chapters apply this framework to individual product families. They explain the contract, the market inputs, the valuation logic, and the resulting sensitivities for each family.
