//! Measures attaching a shared fixing history to dated contexts, excluding
//! Excel callbacks, curve construction, and calibration.
//!
//! Run with `cargo run -p quantsupport-xll --release --example benchmark_history`.

use std::{hint::black_box, time::Instant};

use quantsupport::prelude::{Date, FixingStore, MarketIndex, PricingContext, QuoteStore};

fn main() {
    let mut history = FixingStore::default();
    let start = Date::new(2000, 1, 1);
    for day in 0..10_000 {
        history.add_fixing(&MarketIndex::SOFR, start + day, 0.04);
    }
    let began = Instant::now();
    let contexts: Vec<_> = (0..1000)
        .map(|day| {
            PricingContext::new()
                .with_quote_store(QuoteStore::new(start + 10_000 + day))
                .with_fixing_store(history.clone())
        })
        .collect();
    let elapsed = began.elapsed();
    for context in &contexts {
        let _ = black_box(context.fixing_store().fixing(&MarketIndex::SOFR, start));
    }
    println!("1,000 dated contexts with 10,000 fixings: {elapsed:?}");
    black_box(contexts);
}
