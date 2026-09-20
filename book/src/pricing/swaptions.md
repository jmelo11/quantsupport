# Swaptions

A swaption grants the right to enter an interest-rate swap at a future expiry. Its value therefore combines the underlying swap schedule with the distribution of future rates. This chapter introduces the contract, develops the closed-form Hull-White valuation, and connects market swaption cubes to model calibration.

## Instrument

The swaption builder specifies both the option expiry and the maturity date of the underlying swap. It also records the strike, notional, currency, index, and payer-or-receiver direction. The following example creates a one-year option on a five-year payer swap:

```rust,ignore
let swaption = MakeSwaption::<DualFwd>::default()
    .with_identifier("USD_SOFR_1Y5Y_PAYER".to_string())
    .with_expiry(rd + Period::from_str("1Y")?)
    .with_swap_tenor_date(rd + Period::from_str("6Y")?)      // underlying swap maturity
    .with_strike(0.04)
    .with_notional(10_000_000.0)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_swaption_type(SwaptionType::Payer)                 // default
    .build()?;
let trade = EuropeanSwaptionTrade::new(swaption, rd, 10_000_000.0, Side::LongReceive);
```

The builder requires the strike, expiry, identifier, market index, currency, underlying maturity, and notional. `SwaptionType` selects payer or receiver rights. Frequency settings generate the payment times and accrual fractions of the underlying fixed leg, which the closed-form decomposition uses directly.

## `ClosedFormHullWhiteSwaptionPricer`

The Hull-White pricer takes the model's mean reversion and short-rate volatility. It supports value and quote sensitivities and requests the curve of the swaption index together with any additional curve selected by the discount policy:

```rust,ignore
let pricer = ClosedFormHullWhiteSwaptionPricer::new(alpha, sigma);
let results = pricer.evaluate(&trade, &[Request::Value, Request::Sensitivities], &ctx)?;
```

Valuation uses Jamshidian's decomposition, which converts the option on a coupon-bearing swap into a portfolio of zero-coupon bond options. The calculation proceeds in three stages:

1. Zero-coupon bond prices in Hull-White are affine, \\(P(t,T\mid r_t)=A(t,T)\\,e^{-B(t,T)r_t}\\), with \\(B(t,T)=\frac{1-e^{-\alpha(T-t)}}{\alpha}\\) and \\(A\\) fitted to the initial curve.
2. Find the critical short rate \\(r^{\ast}\\) that makes the underlying fixed leg, including final principal, worth par at expiry. This means solving \\(\sum_i c_i P(T_0,T_i\mid r^{\ast}) = 1\\) by bisection with up to 200 iterations.
3. Use \\(X_i = P(T_0,T_i\mid r^{\ast})\\) as the strike of each zero-coupon bond option. A payer swaption becomes \\(\sum_i c_i\\,\text{BondPut}(T_0,T_i,X_i)\\). A receiver swaption uses the corresponding calls. Each option uses bond volatility \\(\sigma\\,B(T_0,T_i)\sqrt{(1-e^{-2\alpha T_0})/(2\alpha)}\\).

The automatic-differentiation graph includes the result of the critical-rate solve. `Request::Sensitivities` can therefore return derivatives with respect to curve quotes from the same valuation, with no quote-by-quote recalibration loop.

## Volatility cubes

Market swaption volatilities live in a `VolatilityCubeConfiguration` built from identifiers of the form `Swaption_CCY_Index_Expiry_Tenor_[PayFreq_RecvFreq]_Strike_val_VolType`. [Volatility Surfaces](../curves/volatility.md) explains how those fields become cube coordinates.

`ParameterSource::Calibrated` references the constructed cube through `CalibrationSource::Cube`. Its `calibration_basket` selects the expiry, tenor, and strike rule used to fit an LGM or Hull-White sigma schedule. The ICP configuration in `examples/cva/data/xva_config.json` provides a complete example. Pricing the selected contracts with the Hull-White swaption pricer then provides a direct calibration-quality check.

## What to remember

The swaption contract supplies an expiry and an underlying swap schedule. Hull-White supplies a future-rate distribution and a bond-option representation. The volatility cube supplies market targets for calibration. Keeping those roles explicit lets the same pricer value fixed model parameters, verify calibrated parameters, and report risk under the original curve and volatility identifiers.
