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

use crate::utils::errors::QSError;
use thiserror::Error;

#[derive(Debug, Error)]
/// Error raised while tokenizing, parsing, indexing, or evaluating a script.
pub enum ScriptingError {
    /// Source text does not conform to the scripting grammar.
    #[error("Invalid Syntax: {0}")]
    InvalidSyntax(String),
    /// Lexer encountered an invalid token.
    #[error("Invalid Token: {0}")]
    InvalidToken(String),
    /// A numeric literal could not be parsed.
    #[error("Error while parsing: {0}")]
    ParsingError(#[from] std::num::ParseFloatError),
    /// Parser encountered a token that is invalid in the current position.
    #[error("Unexpected token: {0}")]
    UnexpectedToken(String),
    /// Runtime evaluation failed.
    #[error("Error while evaluating: {0}")]
    EvaluationError(String),
    /// Underlying QuantSupport operation failed.
    #[error("QuantSupport error: {0}")]
    QuantSupport(#[from] QSError),
    /// An indexed script or market value could not be found.
    #[error("Not found: {0}")]
    NotFoundError(String),
    /// The script requested an invalid operation.
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Result type used by the scripting subsystem.
pub type Result<T> = std::result::Result<T, ScriptingError>;

impl From<ScriptingError> for String {
    fn from(e: ScriptingError) -> Self {
        e.to_string()
    }
}
