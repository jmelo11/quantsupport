use crate::utils::errors::QSError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScriptingError {
    #[error("Invalid Syntax: {0}")]
    InvalidSyntax(String),
    #[error("Invalid Token: {0}")]
    InvalidToken(String),
    #[error("Error while parsing: {0}")]
    ParsingError(#[from] std::num::ParseFloatError),
    #[error("Unexpected token: {0}")]
    UnexpectedToken(String),
    #[error("Error while evaluating: {0}")]
    EvaluationError(String),
    #[error("QuantSupport error: {0}")]
    QuantSupport(#[from] QSError),
    #[error("Not found: {0}")]
    NotFoundError(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

pub type Result<T> = std::result::Result<T, ScriptingError>;

impl From<ScriptingError> for String {
    fn from(e: ScriptingError) -> Self {
        e.to_string()
    }
}
