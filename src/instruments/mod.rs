//! Financial instruments.
//!
//! Builders and representations for cashflows, fixed-income products,
//! equity derivatives, FX instruments, and interest-rate swaps.

/// Defines cashflows, coupons and the payment legs used by financial contracts.
pub mod cashflows;
/// Defines credit contracts and their instrument makers.
pub mod credit;
/// Defines equity derivatives and their instrument makers.
pub mod equity;
/// Provides the bonds, deposits, floating-rate notes and loans.
pub mod fixedincome;
/// Provides the FX instruments.
pub mod fx;
/// Defines interest-rate derivatives and their instrument makers.
pub mod rates;
