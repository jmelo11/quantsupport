use chrono::ParseError;
use thiserror::Error;

/// # `AtlasError`
///
/// Represents errors that can occur in the Atlas library.
#[derive(Debug, Error)]
pub enum AtlasError {
    /// Error indicating that a requested resource was not found.
    #[error("Not found error: {0}")]
    NotFoundErr(String),
    /// Error that occurs when parsing a date fails.
    #[error("Date parsing error: {0}")]
    DateParsingErr(#[from] ParseError),
    /// Error that occurs when parsing a period fails.
    #[error("Period parsing error: {0}")]
    PeriodParsingErr(String),
    /// Error that occurs during period operations.
    #[error("Period operation error: {0}")]
    PeriodOperationErr(String),
    /// Error that occurs when creating a schedule fails.
    #[error("MakeSchedule error: {0}")]
    MakeScheduleErr(String),
    /// Error that occurs during evaluation.
    #[error("Evaluation error: {0}")]
    EvaluationErr(String),
    /// Error that occurs during serialization.
    #[error("Serialization error: {0}")]
    SerializationErr(String),
    /// Error that occurs during deserialization.
    #[error("Deserialization error: {0}")]
    DeserializationErr(String),
    /// Error indicating that a required value was not set.
    #[error("Value not set error: {0}")]
    ValueNotSetErr(String),
    /// Error indicating that a provided value is invalid.
    #[error("Invalid value error: {0}")]
    InvalidValueErr(String),
    /// Error that occurs during solver (Argmin library) operations.
    #[error("Solver error: {0}")]
    ArgMinSolverErr(#[from] argmin::core::Error),
    /// Error that occurs during solver operations.
    #[error("Solver error: {0}")]
    SolverErr(String),
    /// Error indicating that a feature is not yet implemented.
    #[error("{0}")]
    NotImplementedErr(String),
    /// Error that occurs while interpolating values.
    #[error("Interpolation error {0}")]
    InterpolationErr(String),
    /// A generic node error.
    #[error("Node error: {0}")]
    NodeError(String),
    /// A generic tape error.
    #[error("Tape error: {0}")]
    TapeError(String),
    /// A generic AD number error.
    #[error("AD Real error: {0}")]
    ADRealError(String),
    /// Attempted to access a node that is not recorded on the tape.
    #[error("Node not indexed in tape")]
    NodeNotIndexedInTapeErr,
    /// An unexpected error.
    #[error("{0}")]
    UnexpectedErr(String),
    /// Quote parsing error.
    #[error("Quote parsing error: {0}")]
    QuoteParsingErr(String),
}

/// A specialized `Result` type for Atlas operations that may fail with an `AtlasError`.
pub type Result<T> = std::result::Result<T, AtlasError>;
