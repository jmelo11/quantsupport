# Netting Sets and CSA

## `NettingSet`

```rust,ignore
NettingSet::new(claims: Vec<ContingentClaim>, policy: Box<dyn DiscountPolicy>)
NettingSet::with_csa_terms(claims: Vec<ContingentClaim>, csa: CsaTerms)

ns.claims() -> &[ContingentClaim]
ns.csa_terms() -> Option<&CsaTerms>
ns.discount_policy() -> &dyn DiscountPolicy
```

All claims in a netting set are summed per path and date before taking positive/negative parts, so netting benefit is captured. `XvaEngine::run` requires `with_csa_terms`; `NettingSet::new` is for exposure-only runs with a custom policy.

## `CsaTerms`

```rust,ignore
pub struct CsaTerms {
    collateral_index: MarketIndex,       // discount curve for collateralised cashflows
    collateral_currency: Currency,       // currency of collateral
    credit_spread: f64,                  // flat counterparty hazard rate (if no credit_index)
    recovery: f64,                       // LGD = 1 - recovery
    funding_spread: f64,                 // flat funding spread (fallback)
    funding_spread_curve: Option<FundingSpreadCurve { dates: Vec<Date>, spreads: Vec<f64> }>,
    funding_index: Option<MarketIndex>,  // bootstrapped funding curve
    credit_index: Option<MarketIndex>,   // bootstrapped credit curve, e.g. Credit("CLIENT_A")
}
```

`examples/cva/data/csa_terms.json`:

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

The CSA implies a `SingleCurveCSADiscountPolicy::new(collateral_index, collateral_currency)`: claims in the collateral currency discount on `collateral_index`; claims in other currencies discount on `MarketIndex::Collateral(ccy, collateral_currency)`, which therefore must have both a bootstrapped curve and an `LgmModelConfig`.

Selection rules inside the engine:

| Field set              | Aggregator                                                                                                                   |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `credit_index`         | `CreditCurveCvaFactory` with pillar survivals from the bootstrapped credit curve (sensitivities labelled `<index>.pillar_i`) |
| otherwise              | `CvaFactory` with \\(S(t)=e^{-\text{credit\\\_spread}\cdot t}\\)                                                             |
| `funding_index`        | `FundingCurveFvaFactory` using the spread between the funding curve and the system curve (labels `<funding_index>.<date>`)   |
| `funding_spread_curve` | `FundingCurveFvaFactory` with the explicit term structure (labels `funding_spread.<date>`)                                   |
| otherwise              | `FvaFactory` with the flat `funding_spread`                                                                                  |

## Building claims

```rust,ignore
let claims: Vec<ContingentClaim> = swap_trade.into_claims()?;             // IntoContingentClaims
let claim = MakeContingentClaim::default()
    .with_trade_id("MANUAL_1").with_leg_id("fixed").with_payment_date(d)
    .with_currency(Currency::USD).with_notional(1e6).with_side(Side::LongReceive)
    .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount: 25_000.0 })
    .build()?;
```

Multiple trades — swaps, cross-currency swaps, FX forwards, options and `ScriptedProduct`s — can share a netting set as long as their currencies are covered by `fx_configs`.

## Multiple netting sets

`run(&mut HashMap<String, NettingSet>)` simulates a single market model for all sets and produces per-set `XvaValue`s, so counterparties sharing the same market factors are evaluated on identical paths.
