# Glossary

This glossary gives the project-specific meaning of terms used throughout the book. Many entries connect a financial concept to the Rust type that represents it. Use the linked chapter names in the surrounding text for full derivations and workflows, and use this page when a term appears in several layers of the library.

| Term                          | Meaning in quantsupport                                                                                                                                  |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **AAD / AD**                  | Algorithmic or adjoint differentiation. `Dual<T>` records a tape, and one reverse sweep yields all sensitivities.                                        |
| **Aggregator**                | `PfeAggregator` implementor turning an `NpvCube` into a scalar measure (CVA, DVA, FVA, PFE quantile).                                                    |
| **Annuity**                   | \\(\sum_i N\tau_i P(T_i)\\) over fixed coupons and the denominator of the fair swap rate.                                                               |
| **Claim** (`ContingentClaim`) | Atomic future cashflow with an evaluation strategy and the unit of exposure simulation.                                                                   |
| **Collateral curve**          | `MarketIndex::Collateral(ccy, coll_ccy)`: discount curve for `ccy` cashflows under `coll_ccy` collateral, bootstrapped from cross-currency basis quotes. |
| **CSA** (`CsaTerms`)          | Credit Support Annex parameters: collateral index/currency, credit spread or credit curve, recovery, funding spread or curve.                            |
| **CVA / DVA / FVA**           | Credit, debit and funding valuation adjustments computed from EPE/ENE profiles.                                                                          |
| **Discount policy**           | `DiscountPolicy` trait selecting the discount curve per leg (`SingleCurveCSADiscountPolicy`, `FixedIncomeDiscountPolicy`).                               |
| **DualFwd**                   | `Dual<Fwd2>`: default AD scalar (reverse over second-order forward).                                                                                     |
| **EE / EPE / ENE**            | Expected exposure, expected positive/negative exposure per date from an `NpvCube`.                                                                       |
| **Event / EventStream**       | Dated script code blocks (`CodedEvent`) and their parsed, validated sequence.                                                                            |
| **Evaluator**                 | Type-erased dispatcher from `TypeId` to `ErasedPricer`.                                                                                                  |
| **Fixing**                    | Historical index observation stored in `FixingStore` and used for coupons whose accrual already started.                                                |
| **FuzzyEvaluator**            | Script evaluator that smooths `if` conditions with call spreads so payoffs are differentiable.                                                           |
| **Hull-White**                | One-factor Gaussian short-rate model \\(dr=(\theta-\alpha r)dt+\sigma dW\\) with closed forms for ZCBs, caplets, and swaptions.                         |
| **IFT**                       | Implicit function theorem, used to convert calibrated-node sensitivities into quote sensitivities.                                                       |
| **LGM**                       | Linear Gaussian Markov model with state \\(z_t\\), loading \\(H(t)\\), and variance \\(\zeta(t)\\), forming the basis of `LgmMarketModel`.               |
| **Level**                     | `Bid`, `Mid`, `Ask` — which side of a quote to use.                                                                                                      |
| **MarketIndex**               | Curve identifier: `SOFR`, `ICP`, `TermSOFR3m`, `Collateral(..)`, `Credit(..)`, `Equity(..)`.                                                             |
| **Netting set**               | Claims valued together under one CSA, with positive exposure calculated from their netted sum.                                                          |
| **NpvCube**                   | `npvs[path][date]` matrix per trade produced by the exposure evaluator.                                                                                  |
| **Numeraire**                 | Bank-account value along a path used to deflate cashflows in LGM.                                                                                        |
| **Pillar**                    | Named market coordinate or calibrated node used to organize values and sensitivities.                                                                    |
| **PricingContext**            | Owner of quotes, configurations, constructed elements, and the AD tape, serving as the market-data entry point for evaluation.                           |
| **Quote identifier**          | Underscore-separated string such as `OIS_USD_SOFR_5Y` parsed into `QuoteDetails`.                                                                        |
| **Request**                   | Requested output such as `Value`, `FairRate`, `Cashflows`, or `Sensitivities`. The enum also reserves yield and duration variants.                       |
| **Scenario**                  | Absolute or relative quote transformation applied before construction through segment-based target matching.                                            |
| **ScriptEngine**              | Compiles an `EventStream` into an evaluable product and prices it on a `MarketModel`.                                                                    |
| **Side**                      | `LongReceive` / `PayShort` sign convention for trades and claims.                                                                                        |
| **Strike**                    | `Absolute(K)`, `Atm`, `Relative(spread)` resolved against the forward.                                                                                   |
| **Tape**                      | Thread-local recorder of `Dual` operations with marks and rewinds for repeated trade evaluations.                                                        |
| **Vol surface / cube**        | Bilinear (expiry × strike) or trilinear (expiry × tenor × strike) interpolated implied volatilities.                                                     |

## How the terms connect

The glossary terms form one connected workflow. Quotes and fixings enter `PricingContext`, constructed curves and volatility objects expose pillars, pricers answer requests, and AD traces results back to those market labels. Claims, market models, netting sets, and aggregators extend the same vocabulary into exposure and XVA.
