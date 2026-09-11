/*
This file is part of QuantSupport's Rust rewrite and adaptation of the
derivatives scripting code written by Antoine Savine in 2018.

The original code is the strict intellectual property of Antoine Savine.

A license to use and alter the original code for personal and commercial
applications is freely granted to any person or company that purchased a copy
of the book:

Modern Computational Finance: Scripting for Derivatives and XVA
Jesper Andreasen and Antoine Savine
Wiley, 2018

This attribution and license notice must be preserved at the top of this file.
*/

//! Payoff scripting language.
//!
//! Scripts are parsed into an event stream, indexed into the library's
//! market-data request types, and evaluated against paths produced by the
//! existing [`MarketModel`](crate::xva::visitors::marketmodel::MarketModel)
//! abstraction.

// The parser and AST intentionally favor explicit match arms and stateful
// visitors. Those patterns are clearer for this small interpreter even when
// they conflict with crate-wide style lints aimed at numerical kernels.
#![allow(clippy::cargo, clippy::nursery, clippy::pedantic)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

/// Runtime market-data values supplied to scripts.
pub mod data;
/// Abstract syntax tree nodes and event streams.
pub mod nodes;
/// Lexer and parser for the scripting language.
pub mod parsing;
pub mod product;
pub mod request;
pub mod runtime;
/// Scripting errors and shared helpers.
pub mod utils;
/// AST analysis, transformation, and evaluation visitors.
pub mod visitors;

/// Numeric representation used while evaluating scripts.
pub type NumericType = crate::ad::dual::DualFwd;
