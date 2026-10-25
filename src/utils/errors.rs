use chrono::ParseError;
use thiserror::Error;

/// Errors shared by all library operations, including scripting.
///
/// Use [`QSError::context`] to identify the operation that failed while retaining
/// its cause. Match [`QSError::root_cause`] to inspect the original variant even
/// when callers have attached context.
#[derive(Debug, Error)]
pub enum QSError {
    /// A value has the wrong type at an interface boundary.
    #[error("Invalid type error: expected {expected}, got {actual}")]
    InvalidTypeErr {
        /// Required type or input shape.
        expected: String,
        /// Type or input shape that was supplied.
        actual: String,
    },
    /// An operating-system I/O operation failed.
    #[error("I/O error: {0}")]
    IoErr(#[from] std::io::Error),
    /// Error indicating that a requested resource was not found.
    #[error("Not found error: {0}")]
    NotFoundErr(String),
    /// Error that occurs when parsing a date fails.
    #[error("Date parsing error: {0}")]
    DateParsingErr(#[from] ParseError),
    /// Error that occurs when parsing a period fails.
    #[error("Period parsing error: {0}")]
    PeriodParsingErr(String),
    /// Input text does not conform to the expected grammar or literal format.
    #[error("Parsing error: {0}")]
    ParsingErr(String),
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
    /// Error that occurs during solver operations.
    #[error("Solver error: {0}")]
    SolverErr(String),
    /// Error indicating that a feature is not yet implemented.
    #[error("Not implemented: {0}")]
    NotImplementedErr(String),
    /// Error that occurs while interpolating values.
    #[error("Interpolation error: {0}")]
    InterpolationErr(String),
    /// A generic node error.
    #[error("Node error: {0}")]
    NodeError(String),
    /// A generic tape error.
    #[error("Tape error: {0}")]
    TapeError(String),
    /// A generic AD number error.
    #[error("AD Real error: {0}")]
    DualFwdError(String),
    /// Attempted to access a node that is not recorded on the tape.
    #[error("Node not indexed in tape")]
    NodeNotIndexedInTapeErr,
    /// An unexpected error.
    #[error("Unexpected error: {0}")]
    UnexpectedErr(String),
    /// Quote parsing error.
    #[error("Quote parsing error: {0}")]
    QuoteParsingErr(String),
    /// Instrument resolution error.
    #[error("Instrument resolution error: {0}")]
    InstrumentResolutionErr(String),
    /// An operation failed with the enclosed library error.
    ///
    /// This adds diagnostic context without changing the underlying category.
    #[error("{context}: {source}")]
    Context {
        /// The operation or resource involved in the failure.
        context: String,
        /// The original error, available through [`std::error::Error::source`].
        #[source]
        source: Box<Self>,
    },
}

impl QSError {
    /// Returns a stable code and native variant name, ignoring context wrappers.
    #[must_use]
    pub fn kind(&self) -> (&'static str, &'static str) {
        macro_rules! kinds {
            ($($pattern:pat => ($code:literal, $name:literal)),+ $(,)?) => {
                match self.root_cause() {
                    $($pattern => ($code, $name),)+
                    Self::Context { .. } => unreachable!("root_cause removes context wrappers"),
                }
            };
        }
        kinds! {
            Self::InvalidTypeErr { .. } => ("invalid_type", "InvalidTypeErr"),
            Self::IoErr(_) => ("io", "IoErr"),
            Self::NotFoundErr(_) => ("not_found", "NotFoundErr"),
            Self::DateParsingErr(_) => ("date_parsing", "DateParsingErr"),
            Self::PeriodParsingErr(_) => ("period_parsing", "PeriodParsingErr"),
            Self::ParsingErr(_) => ("parsing", "ParsingErr"),
            Self::PeriodOperationErr(_) => ("period_operation", "PeriodOperationErr"),
            Self::MakeScheduleErr(_) => ("schedule", "MakeScheduleErr"),
            Self::EvaluationErr(_) => ("evaluation", "EvaluationErr"),
            Self::SerializationErr(_) => ("serialization", "SerializationErr"),
            Self::DeserializationErr(_) => ("deserialization", "DeserializationErr"),
            Self::ValueNotSetErr(_) => ("missing_value", "ValueNotSetErr"),
            Self::InvalidValueErr(_) => ("invalid_value", "InvalidValueErr"),
            Self::SolverErr(_) => ("solver", "SolverErr"),
            Self::NotImplementedErr(_) => ("not_implemented", "NotImplementedErr"),
            Self::InterpolationErr(_) => ("interpolation", "InterpolationErr"),
            Self::NodeError(_) => ("node", "NodeError"),
            Self::TapeError(_) => ("tape", "TapeError"),
            Self::DualFwdError(_) => ("dual_forward", "DualFwdError"),
            Self::NodeNotIndexedInTapeErr => ("node_not_indexed", "NodeNotIndexedInTapeErr"),
            Self::UnexpectedErr(_) => ("unexpected", "UnexpectedErr"),
            Self::QuoteParsingErr(_) => ("quote_parsing", "QuoteParsingErr"),
            Self::InstrumentResolutionErr(_) => ("instrument_resolution", "InstrumentResolutionErr"),
        }
    }

    /// Stable machine-readable category shared by native and foreign interfaces.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.kind().0
    }

    /// Operation contexts in order from the outermost to the innermost.
    #[must_use]
    pub fn contexts(&self) -> Vec<&str> {
        let mut result = Vec::new();
        let mut error = self;
        while let Self::Context { context, source } = error {
            result.push(context.as_str());
            error = source;
        }
        result
    }

    /// Adds operation or resource context, preserving the original error.
    #[must_use]
    pub fn context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }

    /// Returns the original library error beneath any context wrappers.
    ///
    /// External causes, such as a date parser's error, remain available through
    /// [`std::error::Error::source`] on the returned error.
    #[must_use]
    pub fn root_cause(&self) -> &Self {
        let mut cause = self;
        while let Self::Context { source, .. } = cause {
            cause = source;
        }
        cause
    }
}

impl From<std::convert::Infallible> for QSError {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
    }
}

/// A specialized `Result` type for operations that may fail with a [`QSError`].
pub type Result<T> = std::result::Result<T, QSError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn structured_categories_survive_context_without_parsing_messages() {
        for (error, code, variant) in [
            (
                QSError::InvalidTypeErr {
                    expected: "dict".into(),
                    actual: "list".into(),
                },
                "invalid_type",
                "InvalidTypeErr",
            ),
            (
                QSError::InvalidValueErr("a misleading Not found error: message".into()),
                "invalid_value",
                "InvalidValueErr",
            ),
            (
                QSError::NotFoundErr("SOFR".into()),
                "not_found",
                "NotFoundErr",
            ),
            (
                QSError::EvaluationErr("inactive context".into()),
                "evaluation",
                "EvaluationErr",
            ),
            (
                QSError::DeserializationErr("quotes[1].mid".into()),
                "deserialization",
                "DeserializationErr",
            ),
            (
                QSError::IoErr(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "quotes.json",
                )),
                "io",
                "IoErr",
            ),
        ] {
            let error = error.context("resolving curve").context("pricing loan");
            assert_eq!(error.code(), code);
            assert_eq!(error.kind(), (code, variant));
            assert_eq!(error.contexts(), ["pricing loan", "resolving curve"]);
            assert_eq!(error.root_cause().kind(), error.kind());
            assert!(error.source().is_some());
        }
    }

    #[test]
    fn context_preserves_category_and_source_chain() -> std::result::Result<(), Box<dyn Error>> {
        let error = QSError::NotFoundErr("discount curve SOFR".into())
            .context("generating path 7")
            .context("evaluating script");
        assert_eq!(
            error.to_string(),
            "evaluating script: generating path 7: Not found error: discount curve SOFR"
        );
        assert!(matches!(error.root_cause(), QSError::NotFoundErr(_)));
        let source = error.source().ok_or("missing operation source")?;
        assert_eq!(
            source.to_string(),
            "generating path 7: Not found error: discount curve SOFR"
        );
        assert_eq!(
            source.source().ok_or("missing root source")?.to_string(),
            "Not found error: discount curve SOFR"
        );
        assert!(source
            .source()
            .ok_or("missing root source")?
            .source()
            .is_none());
        Ok(())
    }

    #[test]
    fn context_retains_external_date_parsing_cause() -> std::result::Result<(), Box<dyn Error>> {
        let parse_error = chrono::NaiveDate::parse_from_str("bad date", "%Y-%m-%d").unwrap_err();
        let error = QSError::from(parse_error).context("parsing payment date");
        assert!(matches!(error.root_cause(), QSError::DateParsingErr(_)));
        assert!(error
            .root_cause()
            .source()
            .ok_or("missing parser source")?
            .is::<ParseError>());
        Ok(())
    }
}
