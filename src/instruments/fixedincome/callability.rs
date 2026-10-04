//! Exercise terms for callable and puttable bonds.

use serde::{Deserialize, Serialize};

use crate::{
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// Identifies which party may exercise an embedded bond option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExerciseType {
    /// The issuer may redeem the bond early, capping the holder's value.
    Call,
    /// The holder may sell the bond back early, flooring the holder's value.
    Put,
}

/// A dated call or put right.
///
/// `price` is quoted per the bond's units and excludes any contractual coupon
/// paid on the same date. This makes coupon/exercise ordering explicit: the
/// coupon is paid first and the option applies to the remaining bond value.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Callability {
    date: Date,
    price: f64,
    exercise_type: ExerciseType,
}

impl Callability {
    /// Creates a call or put event.
    #[must_use]
    pub const fn new(date: Date, price: f64, exercise_type: ExerciseType) -> Self {
        Self {
            date,
            price,
            exercise_type,
        }
    }

    /// Returns the exercise date.
    #[must_use]
    pub const fn date(&self) -> Date {
        self.date
    }

    /// Returns the exercise price per bond units.
    #[must_use]
    pub const fn price(&self) -> f64 {
        self.price
    }

    /// Returns whether the event is an issuer call or holder put.
    #[must_use]
    pub const fn exercise_type(&self) -> ExerciseType {
        self.exercise_type
    }

    /// Validates the exercise price.
    ///
    /// # Errors
    /// Returns an error unless the price is finite and strictly positive.
    pub fn validate(&self) -> Result<()> {
        if self.price.is_finite() && self.price > 0.0 {
            Ok(())
        } else {
            Err(QSError::InvalidValueErr(format!(
                "Bond exercise price must be finite and positive, got {}",
                self.price
            )))
        }
    }
}
