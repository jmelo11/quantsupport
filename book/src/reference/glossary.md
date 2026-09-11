# Glossary

| Term                          | Meaning in quantsupport                                                                                                                                  |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **AAD / AD**                  | Algorithmic (adjoint) differentiation. `Dual<T>` records a tape; one reverse sweep yields all sensitivities.                                             |
| **Aggregator**                | `PfeAggregator` implementor turning an `NpvCube` into a scalar measure (CVA, DVA, FVA, PFE quantile).                                                    |
| **Annuity**                   | \\(\sum_i N\tau_i P(T_i)\\) over fixed coupons; denominator of the fair swap rate.                                                                       |
| **Claim** (`ContingentClaim`) | Atomic future cashflow with an evaluation strategy; the unit of exposure simulation.                                                                     |
| **Collateral curve**          | `MarketIndex::Collateral(ccy, coll_ccy)`: discount curve for `ccy` cashflows under `coll_ccy` collateral, bootstrapped from cross-currency basis quotes. |
| **CSA** (`CsaTerms`)          | Credit Support Annex parameters: collateral index/currency, credit spread or credit curve, recovery, funding spread or curve.                            |
| **CVA / DVA / FVA**           | Credit, debit and funding valuation adjustments computed from EPE/ENE profiles.                                                                          |
| **Discount policy**           | `DiscountPolicy` trait selecting the discount curve per leg (`SingleCurveCSADiscountPolicy`, `FixedIncomeDiscountPolicy`).                               |
| **DualFwd**                   | `Dual<Fwd2>`: default AD scalar (reverse over second-order forward).                                                                                     |
| **EE / EPE / ENE**            | Expected exposure, expected positive/negative exposure per date from an `NpvCube`.                                                                       |
| **Event / EventStream**       | Dated script code blocks (`CodedEvent`) and their parsed, validated sequence.                                                                            |
| **Evaluator**                 | Type-erased dispatcher from `TypeId` to `ErasedPricer`.                                                                                                  |
| **Fixing**                    | Historical index observation stored in `FixingStore`; used for coupons whose accrual already started.                                                    |
| **FuzzyEvaluator**            | Script evaluator that smooths `if` conditions with call spreads so payoffs are differentiable.                                                           |
| **Hull-White**                | One-factor Gaussian short-rate model \\(dr=(\theta-\alpha r)dt+\sigma dW\\); closed forms for ZCBs, caplets, swaptions.                                  |
| **IFT**                       | Implicit function theorem; converts pillar sensitivities into quote sensitivities after bootstrapping.                                                   |
| **LGM**                       | Linear Gaussian Markov model; state \\(z_t\\), functions \\(H(t)\\), \\(\zeta(t)\\); basis of `LgmMarketModel`.                                          |
| **Level**                     | `Bid`, `Mid`, `Ask` — which side of a quote to use.                                                                                                      |
| **MarketIndex**               | Curve identifier: `SOFR`, `ICP`, `TermSOFR3m`, `Collateral(..)`, `Credit(..)`, `Equity(..)`.                                                             |
| **Netting set**               | Claims valued together under one CSA; positive exposure is taken on the netted sum.                                                                      |
| **NpvCube**                   | `npvs[path][date]` matrix per trade produced by the exposure evaluator.                                                                                  |
| **Numeraire**                 | Bank-account value along a path used to deflate cashflows in LGM.                                                                                        |
| **Pillar**                    | Curve node created by one quote; sensitivities are reported per pillar quote identifier.                                                                 |
| **PricingContext**            | Owner of quotes, configurations, bootstrapped elements and the AD tape; entry point for evaluation.                                                      |
| **Quote identifier**          | Underscore-separated string such as `OIS_USD_SOFR_5Y` parsed into `QuoteDetails`.                                                                        |
| **Request**                   | `Value`, `FairRate`, `Cashflows`, `Sensitivities` (plus unimplemented `YieldToMaturity`, `ModifiedDuration`).                                            |
| **Scenario**                  | Quote shock (`Absolute`/`Relative`) applied before bootstrapping; segment-based target matching.                                                         |
| **ScriptEngine**              | Compiles an `EventStream` into an evaluable product and prices it on a `MarketModel`.                                                                    |
| **Side**                      | `LongReceive` / `PayShort` sign convention for trades and claims.                                                                                        |
| **Strike**                    | `Absolute(K)`, `Atm`, `Relative(spread)` resolved against the forward.                                                                                   |
| **Tape**                      | Thread-local recorder of `Dual` operations; supports marks and rewinds between trades.                                                                   |
| **Vol surface / cube**        | Bilinear (expiry × strike) or trilinear (expiry × tenor × strike) interpolated implied volatilities.                                                     |
