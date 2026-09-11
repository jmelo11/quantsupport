# Interest Rate Swaps

A fixed-floating swap exchanges a fixed coupon stream for coupons linked to a market index. `MakeSwap` creates both legs from dates, frequencies, conventions, currency, side, and notional.

```rust,ignore
let swap = MakeSwap::<DualFwd>::default()
    .with_identifier("SOFR-5Y".to_string())
    .with_start_date(today)
    .with_maturity_date(today + Period::from_str("5Y")?)
    .with_fixed_rate(0.04)
    .with_notional(10_000_000.0)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_side(Side::LongReceive)
    .with_fixed_leg_frequency(Frequency::Semiannual)
    .with_floating_leg_frequency(Frequency::Quarterly)
    .build()?;
```

Wrap the result in `SwapTrade`, then use `DiscountedCashflowPricer`. `Request::FairRate` solves the fixed rate for zero NPV using the same curves and conventions as valuation.

Seasoned swaps require historical fixings for reset dates before valuation. Missing fixings should be treated as input errors, not silently forecast. See [Your First Swap](../getting-started/first-swap.md) and run `cargo run -p valuation`.
