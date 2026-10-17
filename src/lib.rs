//! Derivatives valuation, market construction, and risk in Rust.
//!
//! QuantSupport separates observable quotes and fixings from constructed curves,
//! contractual instruments from positions, and valuation methods from market-data
//! access. [`PricingContext`](crate::core::pricingcontext::PricingContext) supplies
//! a pricer's dependencies either on request or through explicit initialization.
//! Native pricers, model-backed payoff scripts, and exposure calculations share
//! market-data representations and automatic-differentiation infrastructure.
//!
//! This is the native API reference. The [Book](https://jmelo11.github.io/quantsupport/)
//! develops the financial and computational concepts for all interfaces;
//! the [binding documentation](https://jmelo11.github.io/quantsupport/api/)
//! contains independent Python and Excel guides and references. Rust functionality
//! is not necessarily exposed by either binding.
//!
//! # Receive-fixed swap under a flat curve
//!
//! A five-year USD swap receives a 3% simple ACT/360 coupon and pays a projected
//! floating coupon semiannually. Its projection and discounting curve is flat at
//! 3% continuously compounded on ACT/360. These two rate conventions differ:
//! equal numeric rates do not make this swap par.
//!
//! ```
//! use std::{cell::RefCell, rc::Rc};
//! use quantsupport::prelude::*;
//! # fn main() -> Result<()> {
//! let valuation_date = Date::new(2024, 1, 15);
//! let notional = 10_000_000.0;
//! let swap = MakeSwap::<DualFwd>::default()
//!     .with_identifier("USD_IRS_5Y".to_string())
//!     .with_start_date(valuation_date)
//!     .with_maturity_date(Date::new(2029, 1, 15))
//!     .with_fixed_rate(0.03)
//!     .with_notional(notional)
//!     .with_rate_definition(RateDefinition::new(
//!         DayCounter::Actual360,
//!         Compounding::Simple,
//!         Frequency::Semiannual,
//!     ))
//!     .with_currency(Currency::USD)
//!     .with_market_index(MarketIndex::SOFR)
//!     .with_side(Side::LongReceive)
//!     .with_fixed_leg_frequency(Frequency::Semiannual)
//!     .with_floating_leg_frequency(Frequency::Semiannual)
//!     .build()?;
//! let trade = SwapTrade::new(swap, valuation_date, notional, Side::LongReceive);
//!
//! let curve = FlatForwardTermStructure::new(
//!     valuation_date,
//!     DualFwd::from(0.03),
//!     RateDefinition::new(
//!         DayCounter::Actual360,
//!         Compounding::Continuous,
//!         Frequency::Annual,
//!     ),
//! )
//! .with_pillar_label("SOFR_flat".to_string());
//! let mut elements = ConstructedElementStore::default();
//! elements.discount_curves_mut().insert(
//!     MarketIndex::SOFR,
//!     DiscountCurveElement::new(MarketIndex::SOFR, Rc::new(RefCell::new(curve))),
//! );
//! let context = PricingContext::new()
//!     .with_quote_store(QuoteStore::new(valuation_date))
//!     .with_constructed_elements(elements);
//!
//! let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
//! let results = pricer.evaluate(
//!     &trade,
//!     &[Request::Value, Request::FairRate, Request::Sensitivities],
//!     &context,
//! )?;
//! let npv = results.price()
//!     .ok_or_else(|| QSError::ValueNotSetErr("swap NPV".into()))?;
//! let par_rate = results.fair_rate()
//!     .ok_or_else(|| QSError::ValueNotSetErr("swap par rate".into()))?;
//! assert!(npv < 0.0);
//! assert!(par_rate > 0.03);
//!
//! let risk = results.sensitivities()
//!     .ok_or_else(|| QSError::ValueNotSetErr("swap sensitivities".into()))?;
//! assert_eq!(risk.instrument_keys(), &["SOFR_flat".to_string()]);
//! let approximate_one_bp_change = risk.exposure()[0] * 1e-4;
//! assert!(approximate_one_bp_change < 0.0);
//! # Ok(())
//! # }
//! ```
//!
//! The value is in USD. Here the sensitivity is with respect to the supplied
//! flat decimal rate, not a bootstrapped market quote; multiplying by `1e-4`
//! estimates the P&L for a one-basis-point increase. Bootstrapped curves instead
//! reconnect pillar risk to their calibration quotes. Their labels and native
//! quote units determine how a reported derivative should be scaled.
//!
//! # API entry points
//!
//! [`prelude`] re-exports the principal workflow types. [`time`] and [`rates`]
//! specify date, accrual, compounding, and term-structure conventions;
//! [`instruments`] defines contracts and positions. [`core`] contains result
//! requests, market providers, pricing policies, and historical snapshots.
//! [`pricers`] implements deterministic, analytic, and numerical valuation.
//! [`ad`] supports differentiation, while [`scripting`], [`models`], and [`xva`]
//! supply model-backed payoffs, stochastic dynamics, and exposure aggregation.

pub mod ad;
pub mod calibration;
pub mod core;
pub mod currencies;
pub mod indices;
pub mod instruments;
pub mod math;
/// Pricing models (GBM, Hull-White, etc.).
pub mod models;
/// Commonly used public exports for pricing and market-data workflows.
pub mod prelude;
pub mod pricers;
pub mod quotes;
pub mod rates;
/// Payoff scripting language and model-backed evaluation runtime.
pub mod scripting;
pub mod simulations;
pub mod time;
pub mod utils;
pub mod volatility;
pub mod xva;
