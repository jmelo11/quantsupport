# Instruments and Trades

An instrument defines contractual cashflows or payoff behavior. A trade wraps the instrument with booked economics. This distinction lets one instrument representation participate in valuation, risk, exposure, and XVA workflows.

Builders encode schedule and convention choices:

```rust,ignore
let swap = MakeSwap::<DualFwd>::default()
    .with_identifier("swap-001".to_string())
    .with_start_date(start)
    .with_maturity_date(maturity)
    .with_fixed_rate(0.04)
    .with_notional(1_000_000.0)
    .with_currency(Currency::USD)
    .with_market_index(MarketIndex::SOFR)
    .with_side(Side::LongPay)
    .build()?;

let trade = SwapTrade::new(swap, trade_date, 1_000_000.0, Side::LongPay);
```

Supported families include deposits, bonds, floating-rate notes, futures, swaps, basis swaps, caps and floors, swaptions, cross-currency swaps, FX forwards and options, equity forwards and European options, and credit default swaps.

`Side` determines economic direction. Verify side conventions at construction boundaries, especially for receive/pay swaps and call/put options. Dates, calendars, day counts, frequencies, and business-day conventions determine generated cashflows and should be treated as part of the product definition, not pricing parameters.
