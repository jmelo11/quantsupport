//! Quantitative finance library for pricing, market construction and risk.
//!
//! `QuantSupport` (QS) is written in Rust, with Python and Excel bindings. It
//! supports instrument valuation across several asset classes, sensitivities
//! using automatic differentiation, interest-rate and credit curve construction,
//! volatility markets, Monte Carlo simulation and valuation adjustments.
//!
//! Observable market data, such as quotes and fixings, are stored separately
//! from constructed objects such as bootstrapped curves and volatility surfaces.
//! Instruments define contractual terms, while trades add position details.
//! A pricer combines the trade with its required market data to calculate the
//! requested measures.
//!
//! Makers can also retain relative tenors until a market context is available.
//! [`MakeInstrument`](core::makeinstrument::MakeInstrument) builds concrete instruments
//! with explicit parameters or a supplied pricing context. The same maker
//! can be resolved for several reference dates; each resolved instrument retains
//! its own contractual dates and principal schedule.
//!
//! This reference describes the native Rust API. The
//! [Book](https://jmelo11.github.io/quantsupport/) explains financial concepts
//! and workflows across the three interfaces. The
//! [binding documentation](https://jmelo11.github.io/quantsupport/api/)
//! provides detailed Python and Excel guides and references.
//!
//! # Quick example: a receive-fixed swap
//!
//! Consider a five-year USD swap that receives a fixed coupon of 3%, using
//! simple compounding and ACT/360 accrual, and pays a floating coupon every six
//! months. We define the contract with `MakeSwap`, wrap it in a trade and supply
//! a flat, continuously compounded 3% curve for projection and discounting.
//! The pricer then returns value, par coupon and market sensitivities.
//!
//! ```
//! use std::{cell::RefCell, rc::Rc};
//! use quantsupport::prelude::*;
//! # fn main() -> Result<()> {
//! let valuation_date = Date::new(2024, 1, 15);
//! let notional = 10_000_000.0;
//! let maker = MakeSwap::<DualFwd>::default()
//!     .with_identifier("USD_IRS_5Y".to_string())
//!     .with_start_tenor(Period::new(0, TimeUnit::Days))
//!     .with_tenor(Period::new(5, TimeUnit::Years))
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
//!     .with_floating_leg_frequency(Frequency::Semiannual);
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
//! let swap = maker.build_using(&context)?;
//! let trade = SwapTrade::new(swap, valuation_date, notional, Side::LongReceive);
//!
//! let pricer = DiscountedCashflowPricer::<Swap<DualFwd>, SwapTrade<DualFwd>>::new();
//! let results = pricer.evaluate(
//!     &trade,
//!     &[Request::Value, Request::FairRate(0), Request::Sensitivities],
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
//! The value is in USD. The receive-fixed position has negative value because
//! the continuous curve implies simple forward coupons above 3%. Its
//! sensitivity measures the response to the supplied flat decimal rate;
//! multiplying by `1e-4` estimates the P&L for a one-basis-point increase.
//! Bootstrapped curves connect risk to their original calibration quotes, whose
//! labels and units determine how to scale each derivative.
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
