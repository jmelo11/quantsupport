//! `QuantSupport` is a Rust library for financial calculations and analysis.
//!
//! This library provides tools for computing prices, sensitivities and other
//! metrics of financial products.
//!
//! # Quick-start example
//!
//! The snippet below prices a 5-year receive-fixed / pay-floating USD IRS,
//! retrieves its NPV, cashflow schedule and curve sensitivities.
//! A complete, runnable version lives in `examples/valuation/`.
//!
//! ## 1 — Imports
//!
//! Everything needed is re-exported through the [`prelude`] module.
//!
//! ```no_run
//! use std::{cell::RefCell, rc::Rc};
//! use quantsupport::prelude::*;
//! ```
//!
//! ## 2 — Build the swap trade
//!
//! Use [`MakeSwap`](crate::instruments::rates::makeswap::MakeSwap) to
//! configure the instrument, then wrap it in a
//! [`SwapTrade`](crate::instruments::rates::swap::SwapTrade) that carries
//! trade-level metadata (trade date, notional, side).
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use quantsupport::prelude::*;
//! let start_date    = Date::new(2024, 1, 15);
//! let maturity_date = Date::new(2029, 1, 15);
//! let notional      = 10_000_000.0;
//! let fixed_rate    = 0.030; // 3.0%
//!
//! // Day-count / compounding convention for the fixed leg coupon rate.
//! let rate_definition = RateDefinition::new(
//!     DayCounter::Actual360,
//!     Compounding::Simple,
//!     Frequency::Semiannual,
//! );
//!
//! let swap = MakeSwap::default()
//!     .with_identifier("USD_IRS_5Y".to_string())
//!     .with_start_date(start_date)
//!     .with_maturity_date(maturity_date)
//!     .with_fixed_rate(fixed_rate)
//!     .with_notional(notional)
//!     .with_rate_definition(rate_definition)
//!     .with_currency(Currency::USD)
//!     .with_market_index(MarketIndex::SOFR)
//!     .with_side(Side::LongRecieve)               // receive fixed, pay floating
//!     .with_fixed_leg_frequency(Frequency::Semiannual)
//!     .with_floating_leg_frequency(Frequency::Semiannual)
//!     .build()
//!     .expect("Failed to build swap");
//!
//! let trade = SwapTrade::new(swap, start_date, notional, Side::LongRecieve);
//! ```
//!
//! ## 3 — Set up the pricing context
//!
//! A [`ContextManager`](crate::core::contextmanager::ContextManager) holds
//! market data (discount curves, quote / fixing stores) that pricers consult
//! during evaluation.  Here we create a flat SOFR discount curve at 3.0%.
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use quantsupport::prelude::*;
//! let evaluation_date = Date::new(2024, 1, 15);
//! let discount_rate   = 0.03; // 3.0% flat curve
//!
//! // Curve rate convention: continuous compounding, ACT/360.
//! let curve_definition = RateDefinition::new(
//!     DayCounter::Actual360,
//!     Compounding::Continuous,
//!     Frequency::Annual,
//! );
//!
//! // Build the flat-forward term structure.
//! let discount_curve = FlatForwardTermStructure::new(
//!     evaluation_date,
//!     ADReal::from(discount_rate),
//!     curve_definition,
//! )
//! .with_pillar_label("SOFR_flat".to_string());
//!
//! // Register the curve as the SOFR discount curve.
//! let mut constructed_elements = ConstructedElementStore::default();
//! constructed_elements.discount_curves_mut().insert(
//!     MarketIndex::SOFR,
//!     DiscountCurveElement::new(
//!         MarketIndex::SOFR,
//!         Currency::USD,
//!         Rc::new(RefCell::new(discount_curve)),
//!     ),
//! );
//!
//! // Empty stores — no live quotes or historical fixings in this example.
//! let quote_store  = QuoteStore::new(evaluation_date);
//! let fixing_store = FixingStore::default();
//!
//! let context = ContextManager::new(quote_store, fixing_store)
//!     .with_base_currency(Currency::USD)
//!     .with_constructed_elements(constructed_elements);
//! ```
//!
//! ## 4 — Price the swap and read results
//!
//! Create a [`CashflowDiscountPricer`](crate::pricers::cashflows::discountingcashflowpricer::CashflowDiscountPricer),
//! choose which outputs you need via [`Request`](crate::core::request::Request),
//! and call `evaluate`.
//!
//! ```no_run
//! # use std::{cell::RefCell, rc::Rc};
//! # use quantsupport::prelude::*;
//! # let start_date    = Date::new(2024, 1, 15);
//! # let maturity_date = Date::new(2029, 1, 15);
//! # let notional      = 10_000_000.0;
//! # let fixed_rate    = 0.030;
//! # let rate_definition = RateDefinition::new(
//! #     DayCounter::Actual360, Compounding::Simple, Frequency::Semiannual);
//! # let swap = MakeSwap::default()
//! #     .with_identifier("USD_IRS_5Y".to_string())
//! #     .with_start_date(start_date).with_maturity_date(maturity_date)
//! #     .with_fixed_rate(fixed_rate).with_notional(notional)
//! #     .with_rate_definition(rate_definition)
//! #     .with_currency(Currency::USD).with_market_index(MarketIndex::SOFR)
//! #     .with_side(Side::LongRecieve)
//! #     .with_fixed_leg_frequency(Frequency::Semiannual)
//! #     .with_floating_leg_frequency(Frequency::Semiannual)
//! #     .build().unwrap();
//! # let trade = SwapTrade::new(swap, start_date, notional, Side::LongRecieve);
//! # let evaluation_date = Date::new(2024, 1, 15);
//! # let curve_definition = RateDefinition::new(
//! #     DayCounter::Actual360, Compounding::Continuous, Frequency::Annual);
//! # let discount_curve = FlatForwardTermStructure::new(
//! #     evaluation_date, ADReal::from(0.03), curve_definition)
//! #     .with_pillar_label("SOFR_flat".to_string());
//! # let mut constructed_elements = ConstructedElementStore::default();
//! # constructed_elements.discount_curves_mut().insert(
//! #     MarketIndex::SOFR, DiscountCurveElement::new(
//! #         MarketIndex::SOFR, Currency::USD,
//! #         Rc::new(RefCell::new(discount_curve))));
//! # let quote_store  = QuoteStore::new(evaluation_date);
//! # let fixing_store = FixingStore::default();
//! # let context = ContextManager::new(quote_store, fixing_store)
//! #     .with_base_currency(Currency::USD)
//! #     .with_constructed_elements(constructed_elements);
//! let pricer   = CashflowDiscountPricer::<Swap, SwapTrade>::new();
//! let requests = vec![Request::Value, Request::Cashflows, Request::Sensitivities];
//! let results  = pricer.evaluate(&trade, &requests, &context).expect("pricing failed");
//!
//! // --- NPV ---
//! if let Some(price) = results.price() {
//!     println!("Swap NPV = {price:.2}");
//! }
//!
//! // --- Sensitivities (dV/dQuote per curve pillar) ---
//! if let Some(sensitivities) = results.sensitivities() {
//!     println!("\nSensitivities:");
//!     for (key, exposure) in sensitivities.instrument_keys()
//!         .iter()
//!         .zip(sensitivities.exposure().iter())
//!     {
//!         println!("  {key}: {exposure:.4}");
//!     }
//! }
//!
//! // --- Cashflow schedule ---
//! if let Some(cashflows) = results.cashflows() {
//!     let dates      = cashflows.payment_dates();
//!     let types      = cashflows.cashflow_types();
//!     let amounts    = cashflows.amounts();
//!     let currencies = cashflows.currencies();
//!
//!     println!("\nCashflows ({} rows):", dates.len());
//!     for i in 0..dates.len() {
//!         println!(
//!             "  {} | {:<20} | {:>14.2} {}",
//!             dates[i], types[i], amounts[i], currencies[i]
//!         );
//!     }
//! }
//! ```

pub mod ad;
pub mod core;
pub mod currencies;
pub mod indices;
pub mod instruments;
pub mod math;
pub mod models;
pub mod prelude;
pub mod pricers;
pub mod quotes;
pub mod rates;
pub mod simulations;
pub mod time;
pub mod utils;
pub mod volatility;
