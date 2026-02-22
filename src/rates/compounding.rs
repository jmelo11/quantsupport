use serde::{Deserialize, Serialize};

use crate::utils::errors::{AtlasError, Result};

/// # Compounding
/// Enumerate the different compounding methods.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Compounding {
    /// Simple interest compounding method.
    Simple,
    /// Compounded interest compounding method.
    Compounded,
    /// Continuous interest compounding method.
    Continuous,
    /// Simple interest followed by compounded interest.
    SimpleThenCompounded,
    /// Compounded interest followed by simple interest.
    CompoundedThenSimple,
}

impl TryFrom<String> for Compounding {
    type Error = AtlasError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Simple" => Ok(Self::Simple),
            "Compounded" => Ok(Self::Compounded),
            "Continuous" => Ok(Self::Continuous),
            "SimpleThenCompounded" => Ok(Self::SimpleThenCompounded),
            "CompoundedThenSimple" => Ok(Self::CompoundedThenSimple),
            _ => Err(AtlasError::InvalidValueErr(format!(
                "Invalid compounding: {s}"
            ))),
        }
    }
}

impl From<Compounding> for String {
    fn from(compounding: Compounding) -> Self {
        match compounding {
            Compounding::Simple => "Simple".to_string(),
            Compounding::Compounded => "Compounded".to_string(),
            Compounding::Continuous => "Continuous".to_string(),
            Compounding::SimpleThenCompounded => "SimpleThenCompounded".to_string(),
            Compounding::CompoundedThenSimple => "CompoundedThenSimple".to_string(),
        }
    }
}
