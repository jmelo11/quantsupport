# Netting Sets and CSA

A netting agreement determines which trade values offset each other before exposure is measured. A credit support annex, or CSA, adds the collateral currency, discounting convention, counterparty credit terms, and funding assumptions. This chapter explains how `NettingSet` and `CsaTerms` represent those economics, how the engine selects curve-driven or flat inputs, and how claims from several trades are grouped.

## `NettingSet`

`NettingSet` owns the claims covered by one legal agreement and the discount policy applied to them. It can be constructed from full CSA terms or from an explicit policy for exposure-only work:

```rust,ignore
NettingSet::new(claims: Vec<ContingentClaim>, policy: Box<dyn DiscountPolicy>)
NettingSet::with_csa_terms(claims: Vec<ContingentClaim>, csa: CsaTerms)

ns.claims() -> &[ContingentClaim]
ns.csa_terms() -> Option<&CsaTerms>
ns.discount_policy() -> &dyn DiscountPolicy
```

At every path and date, the engine sums all claim values in the set before calculating positive and negative exposure. Offsetting positions therefore reduce the exposure measure according to the legal netting scope. `XvaEngine::run` uses `with_csa_terms` for valuation adjustments. `NettingSet::new` supports exposure workflows driven by a custom discount policy.

## `CsaTerms`

`CsaTerms` gathers the market identities and assumptions associated with the collateral agreement. Flat values provide a compact setup, and optional curve fields supply term-dependent credit or funding inputs:

```rust,ignore
pub struct CsaTerms {
    collateral_index: MarketIndex,       // discount curve for collateralized cashflows
    collateral_currency: Currency,       // currency of collateral
    credit_spread: f64,                  // flat counterparty hazard rate (if no credit_index)
    recovery: f64,                       // LGD = 1 - recovery
    funding_spread: f64,                 // flat funding spread (fallback)
    funding_spread_curve: Option<FundingSpreadCurve { dates: Vec<Date>, spreads: Vec<f64> }>,
    funding_index: Option<MarketIndex>,  // bootstrapped funding curve
    credit_index: Option<MarketIndex>,   // bootstrapped credit curve, e.g. Credit("ACME")
}
```

The CVA example serializes those terms as JSON. It uses USD SOFR collateral, a flat counterparty credit input, and a dated funding-spread curve:

```json
{
  "collateral_index": "SOFR",
  "collateral_currency": "USD",
  "credit_spread": 0.01,
  "recovery": 0.4,
  "funding_index": "TermSOFR3m",
  "funding_spread_curve": {
    "dates": ["2026-11-11", "2028-11-11", "2030-11-11"],
    "spreads": [0.004, 0.005, 0.006]
  }
}
```

The recovery value converts to loss given default through `1 - recovery`. When a credit or funding index is present, the engine obtains term structure values from the corresponding constructed curve. The explicit funding-spread curve provides dated spreads directly.

The CSA implies `SingleCurveCSADiscountPolicy::new(collateral_index, collateral_currency)`. Claims denominated in the collateral currency use `collateral_index`. Claims in another currency use `MarketIndex::Collateral(ccy, collateral_currency)`. Every selected index needs a bootstrapped curve and an `LgmModelConfig` so deterministic valuation and path simulation share the same market identity.

Optional fields follow a defined precedence when the engine builds aggregators:

| Field set              | Aggregator                                                                                                                   |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `credit_index`         | `CreditCurveCvaFactory` with pillar survivals from the bootstrapped credit curve (sensitivities labeled `<index>.pillar_i`) |
| otherwise              | `CvaFactory` with \\(S(t)=e^{-\text{credit\\\_spread}\cdot t}\\)                                                             |
| `funding_index`        | `FundingCurveFvaFactory` using the spread between the funding curve and the system curve (labels `<funding_index>.<date>`)   |
| `funding_spread_curve` | `FundingCurveFvaFactory` with the explicit term structure (labels `funding_spread.<date>`)                                   |
| otherwise              | `FvaFactory` with the flat `funding_spread`                                                                                  |

Curve fields activate market-driven term structures. The flat credit and funding fields provide complete fallback assumptions for a compact setup.

## Building claims

Trades normally create claims through `IntoContingentClaims`. `MakeContingentClaim` also supports a custom deterministic or simulated payment. The following snippet shows both routes:

```rust,ignore
let claims: Vec<ContingentClaim> = swap_trade.into_claims()?;             // IntoContingentClaims
let claim = MakeContingentClaim::default()
    .with_trade_id("MANUAL_1").with_leg_id("fixed").with_payment_date(d)
    .with_currency(Currency::USD).with_notional(1e6).with_side(Side::LongReceive)
    .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount: 25_000.0 })
    .build()?;
```

Swaps, cross-currency swaps, FX forwards, options, and `ScriptedProduct` values can share a netting set. Every foreign currency represented by those claims needs a corresponding `fx_configs` entry in the XVA model.

## Multiple netting sets

`run(&mut HashMap<String, NettingSet>)` accepts several legal agreements at once. It simulates one market model for all sets and produces an `XvaValue` for each set and measure. Counterparties that share market factors are therefore evaluated on identical paths, which gives portfolio comparisons a common random sample.

## What to remember

A netting set defines the legal scope over which positive and negative trade values offset. CSA terms define collateral discounting and the credit and funding inputs used by aggregators. Optional curve identities introduce market term structures, and flat fields keep the configuration complete for simpler cases. Claim-level trade ids preserve the composition of each set through simulation.
