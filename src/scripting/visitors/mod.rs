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

/// Static value-domain analysis for variables and conditions.
pub mod domainprocessor;
/// Exact single-scenario AST evaluator.
pub mod evaluator;
/// Smoothed evaluator for scripts with conditional optionality.
pub mod fuzzyevaluator;
/// Transformation of ordinary conditions into differentiable form.
pub mod ifconditiontransform;
/// Conditional-block analysis and metadata preparation.
pub mod ifprocessor;
/// Variable and market-request indexing.
pub mod varindexer;
