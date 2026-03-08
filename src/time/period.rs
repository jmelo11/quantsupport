use super::enums::{Frequency, TimeUnit};
use serde::de;
use serde::{de::Visitor, Deserializer, Serialize};
use std::fmt;
use std::{
    cmp::Ordering,
    ops::{Add, Mul, MulAssign, Neg, Sub},
};

use crate::utils::errors::{QSError, Result};

/// # Period
/// Struct representing a financial period.
/// # Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p = Period::new(5, TimeUnit::Days);
/// assert_eq!(p.length(), 5);
/// assert_eq!(p.units(), TimeUnit::Days);
/// ```
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Period {
    length: i32,
    units: TimeUnit,
}

impl Period {
    /// Creates a new Period with the given length and time unit.
    ///
    /// # Arguments
    /// * `length` - The length of the period as an integer
    /// * `units` - The time unit of the period
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let p = Period::new(5, TimeUnit::Days);
    /// assert_eq!(p.length(), 5);
    /// assert_eq!(p.units(), TimeUnit::Days);
    /// ```
    #[must_use]
    pub const fn new(length: i32, units: TimeUnit) -> Self {
        Self { length, units }
    }

    /// Creates a Period from a Frequency.
    ///
    /// # Arguments
    /// * `freq` - The frequency to convert to a Period
    ///
    /// # Returns
    /// `Some(Period)` if the frequency can be converted, `None` for `OtherFrequency`.
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::{Frequency, TimeUnit};
    ///
    /// let p = Period::from_frequency(Frequency::Annual).unwrap();
    /// assert_eq!(p.length(), 1);
    /// assert_eq!(p.units(), TimeUnit::Years);
    /// ```
    #[must_use]
    pub const fn from_frequency(freq: Frequency) -> Option<Self> {
        match freq {
            Frequency::NoFrequency => Some(Self {
                units: TimeUnit::Days,
                length: 0,
            }),
            Frequency::Once => Some(Self {
                units: TimeUnit::Years,
                length: 0,
            }),
            Frequency::Annual => Some(Self {
                units: TimeUnit::Years,
                length: 1,
            }),
            Frequency::Semiannual
            | Frequency::EveryFourthMonth
            | Frequency::Quarterly
            | Frequency::Bimonthly
            | Frequency::Monthly => Some(Self {
                units: TimeUnit::Months,
                length: 12 / (freq as i32),
            }),
            Frequency::EveryFourthWeek | Frequency::Biweekly | Frequency::Weekly => Some(Self {
                units: TimeUnit::Weeks,
                length: 52 / (freq as i32),
            }),
            Frequency::Daily => Some(Self {
                units: TimeUnit::Days,
                length: 1,
            }),
            Frequency::OtherFrequency => None,
        }
    }

    /// Returns the Frequency equivalent of this Period.
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::{Frequency, TimeUnit};
    ///
    /// let p = Period::new(1, TimeUnit::Years);
    /// assert_eq!(p.frequency(), Frequency::Annual);
    /// ```
    #[must_use]
    pub const fn frequency(&self) -> Frequency {
        let length = self.length.abs(); // assuming `length` is i32 or some integer type

        if length == 0 {
            match self.units {
                TimeUnit::Years => return Frequency::Once,
                _ => return Frequency::NoFrequency,
            }
        }

        match self.units {
            TimeUnit::Years => {
                if length == 1 {
                    Frequency::Annual
                } else {
                    Frequency::OtherFrequency
                }
            }
            TimeUnit::Months => {
                let quotient = 12 / length;
                if 12 % length == 0 && length <= 12 {
                    match quotient {
                        1 => Frequency::Annual,
                        2 => Frequency::Semiannual,
                        3 => Frequency::Quarterly,
                        4 => Frequency::EveryFourthMonth,
                        6 => Frequency::Bimonthly,
                        12 => Frequency::Monthly,
                        _ => Frequency::OtherFrequency,
                    }
                } else {
                    Frequency::OtherFrequency
                }
            }
            TimeUnit::Weeks => match length {
                1 => Frequency::Weekly,
                2 => Frequency::Biweekly,
                4 => Frequency::EveryFourthWeek,
                _ => Frequency::OtherFrequency,
            },
            TimeUnit::Days => {
                if length == 1 {
                    Frequency::Daily
                } else {
                    Frequency::OtherFrequency
                }
            }
        }
    }

    /// Normalizes the Period by converting to higher time units when possible.
    ///
    /// For example, 7 Days becomes 1 Week, and 12 Months becomes 1 Year.
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let mut p = Period::new(12, TimeUnit::Months);
    /// p.normalize();
    /// assert_eq!(p.length(), 1);
    /// assert_eq!(p.units(), TimeUnit::Years);
    /// ```
    pub const fn normalize(&mut self) {
        if self.length == 0 {
            self.units = TimeUnit::Days;
        }

        match self.units {
            TimeUnit::Months => {
                if self.length % 12 == 0 {
                    self.length /= 12;
                    self.units = TimeUnit::Years;
                }
            }
            TimeUnit::Days => {
                if self.length % 7 == 0 {
                    self.length /= 7;
                    self.units = TimeUnit::Weeks;
                }
            }
            TimeUnit::Weeks | TimeUnit::Years => {}
        }
    }

    /// Returns the length of this Period.
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let p = Period::new(5, TimeUnit::Days);
    /// assert_eq!(p.length(), 5);
    /// ```
    #[must_use]
    pub const fn length(&self) -> i32 {
        self.length
    }

    /// Returns the time unit of this Period.
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let p = Period::new(5, TimeUnit::Days);
    /// assert_eq!(p.units(), TimeUnit::Days);
    /// ```
    #[must_use]
    pub const fn units(&self) -> TimeUnit {
        self.units
    }

    /// Creates an empty Period with zero length in Days.
    ///
    /// # Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let p = Period::empty();
    /// assert_eq!(p.length(), 0);
    /// assert_eq!(p.units(), TimeUnit::Days);
    /// ```
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            length: 0,
            units: TimeUnit::Days,
        }
    }

    /// Parses a tenor string (e.g., "1Y", "6M", "1Y6M") into a Period.
    ///
    /// ## Arguments
    /// * `tenor` - A string in the format like "1Y" or "1Y6M"
    ///
    /// ## Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let p = Period::from_str("1Y6M").unwrap();
    /// assert_eq!(p.length(), 18);
    /// assert_eq!(p.units(), TimeUnit::Months);
    /// ```
    /// Parses a tenor string into a `Period`.
    ///
    /// # Errors
    /// Returns an error if the tenor string cannot be parsed into a valid `Period`.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(tenor: &str) -> Result<Self> {
        Self::parse_impl(tenor)
    }

    fn parse_impl(tenor: &str) -> Result<Self> {
        // parse multiple periods and add them
        let chars = tenor.chars();
        let mut periods = Vec::new();
        let mut current_period = String::new();
        for c in chars {
            current_period.push(c);
            if !c.is_numeric() {
                periods.push(current_period);
                current_period = String::new();
            }
        }
        let mut result = Self::empty();
        for period in periods {
            result = (result + Self::parse_single_period(&period)?)?;
        }
        Ok(result)
    }

    fn parse_single_period(tenor: &str) -> Result<Self> {
        let chars = tenor.chars();
        let mut length = String::new();
        let mut units = String::new();
        for c in chars {
            if c.is_numeric() {
                length.push(c);
            } else {
                units.push(c);
            }
        }
        let length = length.parse::<i32>().map_err(|_| {
            QSError::PeriodOperationErr(format!("Invalid period length ({length})"))
        })?;
        let units = match units.as_str() {
            "Y" => TimeUnit::Years,
            "M" => TimeUnit::Months,
            "W" => TimeUnit::Weeks,
            "D" => TimeUnit::Days,
            _ => {
                return Err(QSError::PeriodOperationErr(format!(
                    "Invalid time unit ({units})"
                )))
            }
        };
        Ok(Self::new(length, units))
    }

    /// Returns the fraction of a year represented by this Period.
    ///
    /// ## Example
    /// ```
    /// use quantsupport::time::period::Period;
    /// use quantsupport::time::enums::TimeUnit;
    ///
    /// let p = Period::new(6, TimeUnit::Months);
    /// assert_eq!(p.period_in_year(), 0.5);
    /// ```
    #[must_use]
    pub fn period_in_year(&self) -> f64 {
        match self.units {
            TimeUnit::Years => f64::from(self.length),
            TimeUnit::Months => f64::from(self.length) / 12.0,
            TimeUnit::Weeks => f64::from(self.length) / 52.0,
            TimeUnit::Days => f64::from(self.length) / 365.0,
        }
    }
}

impl TryFrom<String> for Period {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        Self::from_str(&s)
    }
}

impl std::str::FromStr for Period {
    type Err = QSError;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse_impl(s)
    }
}

impl From<Period> for String {
    fn from(period: Period) -> Self {
        match period.units {
            TimeUnit::Years => format!("{}Y", period.length),
            TimeUnit::Months => format!("{}M", period.length),
            TimeUnit::Weeks => format!("{}W", period.length),
            TimeUnit::Days => format!("{}D", period.length),
        }
    }
}

/// Display a `Period` as a human-readable string (e.g. `1Y`, `6M`).
impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = (*self).into();
        f.write_str(&s)
    }
}

/// Deserializes a string in the format like 1Y or 1Y6M to a Period.
impl<'de> serde::Deserialize<'de> for Period {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PeriodVisitor;

        impl Visitor<'_> for PeriodVisitor {
            type Value = Period;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string in the format like 1Y or 1Y6M")
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Period, E>
            where
                E: de::Error,
            {
                // Parse the string to create a Period
                Period::from_str(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(PeriodVisitor)
    }
}

/// Serializes a Period to a string in the format like 1Y or 1Y6M.
impl Serialize for Period {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self.units {
            TimeUnit::Years => serializer.serialize_str(&format!("{}Y", self.length)),
            TimeUnit::Months => serializer.serialize_str(&format!("{}M", self.length)),
            TimeUnit::Weeks => serializer.serialize_str(&format!("{}W", self.length)),
            TimeUnit::Days => serializer.serialize_str(&format!("{}D", self.length)),
        }
    }
}

/// # `PartialEq` for `Period`
///
/// Compares two `Period` values.
/// ## Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p1 = Period::new(5, TimeUnit::Days);
/// let p2 = Period::new(5, TimeUnit::Days);
/// assert_eq!(p1, p2);
///
/// let p3 = Period::new(5, TimeUnit::Days);
/// let p4 = Period::new(5, TimeUnit::Weeks);
/// assert!(p3<p4);
/// ```
impl PartialOrd for Period {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// # `Ord` for `Period`
/// Compares two `Period` values.
///
/// ## Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p1 = Period::new(5, TimeUnit::Days);
/// let p2 = Period::new(5, TimeUnit::Days);
/// assert_eq!(p1, p2);
/// ```
impl Ord for Period {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.length == other.length {
            self.units.cmp(&other.units)
        } else {
            self.length.cmp(&other.length)
        }
    }

    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        std::cmp::max_by(self, other, Ord::cmp)
    }

    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        std::cmp::min_by(self, other, Ord::cmp)
    }

    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        std::cmp::min(std::cmp::max(self, min), max)
    }
}

/// # `Neg` for `Period`
///
/// Negates a [`Period`].
///
/// ## Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p = Period::new(5, TimeUnit::Days);
/// let negated = -p;
/// assert_eq!(negated.length(), -5);
/// assert_eq!(negated.units(), TimeUnit::Days);
/// ```
impl Neg for Period {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            length: -self.length,
            units: self.units,
        }
    }
}

/// # `Add` for `Period`
///
/// Adds a [`Period`] to another [`Period`].
/// # Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p1 = Period::new(5, TimeUnit::Days);
/// let p2 = Period::new(3, TimeUnit::Days);
/// let p3 = (p1 + p2).unwrap();
/// assert_eq!(p3.length(), 8);
/// assert_eq!(p3.units(), TimeUnit::Days);
impl Add for Period {
    type Output = Result<Self>;

    fn add(self, other: Self) -> Self::Output {
        let mut result = self;
        if result.length == 0 {
            result.length = other.length;
            result.units = other.units;
        } else if result.units == other.units {
            result.length += other.length;
        } else {
            match result.units {
                TimeUnit::Years => match other.units {
                    TimeUnit::Months => {
                        result.units = TimeUnit::Months;
                        result.length = result.length * 12 + other.length;
                    }
                    TimeUnit::Weeks | TimeUnit::Days => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "impossible addition between {result:?} and {other:?}"
                        )));
                    }

                    TimeUnit::Years => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "unknown time unit ({:?})",
                            other.units
                        )))
                    }
                },

                TimeUnit::Months => match other.units {
                    TimeUnit::Years => {
                        result.length += other.length * 12;
                    }
                    TimeUnit::Weeks | TimeUnit::Days => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "impossible addition between {result:?} and {other:?}"
                        )));
                    }

                    TimeUnit::Months => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "unknown time unit ({:?})",
                            other.units
                        )))
                    }
                },

                TimeUnit::Weeks => match other.units {
                    TimeUnit::Days => {
                        result.units = TimeUnit::Days;
                        result.length = result.length * 7 + other.length;
                    }
                    TimeUnit::Years | TimeUnit::Months => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "impossible addition between {result:?} and {other:?}"
                        )));
                    }

                    TimeUnit::Weeks => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "unknown time unit ({:?})",
                            other.units
                        )))
                    }
                },

                TimeUnit::Days => match other.units {
                    TimeUnit::Weeks => {
                        result.length += other.length * 7;
                    }
                    TimeUnit::Years | TimeUnit::Months => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "impossible addition between {result:?} and {other:?}"
                        )));
                    }

                    TimeUnit::Days => {
                        return Err(QSError::PeriodOperationErr(format!(
                            "unknown time unit ({:?})",
                            other.units
                        )))
                    }
                },
            }
        }
        Ok(result)
    }
}

/// # `Sub` for `Period`
/// Subtracts a [`Period`] from another [`Period`].
///
/// ## Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p1 = Period::new(5, TimeUnit::Days);
/// let p2 = Period::new(3, TimeUnit::Days);
/// let p3 = (p1 - p2).unwrap();
/// assert_eq!(p3.length(), 2);
/// assert_eq!(p3.units(), TimeUnit::Days);
/// ```
impl Sub for Period {
    type Output = Result<Self>;

    fn sub(self, other: Self) -> Self::Output {
        self + -other
    }
}

/// # `Mul<i32>` for `Period`
/// Multiplies a [`Period`] by an integer.
///
/// ## Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let p = Period::new(5, TimeUnit::Days);
/// let p2 = p * 2;
/// assert_eq!(p2.length(), 10);
/// assert_eq!(p2.units(), TimeUnit::Days);
/// ```
impl Mul<i32> for Period {
    type Output = Self;

    fn mul(self, n: i32) -> Self::Output {
        Self {
            length: self.length * n,
            units: self.units,
        }
    }
}

/// # `MulAssign<i32>` for `Period`
/// Multiplies a [`Period`] by an integer.
///
/// ## Example
/// ```
/// use quantsupport::time::period::Period;
/// use quantsupport::time::enums::TimeUnit;
///
/// let mut p = Period::new(5, TimeUnit::Days);
/// p *= 2;
/// assert_eq!(p.length(), 10);
/// assert_eq!(p.units(), TimeUnit::Days);
/// ```
impl MulAssign<i32> for Period {
    fn mul_assign(&mut self, n: i32) {
        self.length *= n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_period_negation() {
        let p = Period {
            length: 5,
            units: TimeUnit::Days,
        };
        let negated = -p;
        assert_eq!(negated.length, -5);
        assert_eq!(negated.units, TimeUnit::Days);
    }

    #[test]
    fn test_period_addition() {
        let p1 = Period {
            length: 5,
            units: TimeUnit::Days,
        };
        let p2 = Period {
            length: 3,
            units: TimeUnit::Days,
        };
        let p3 = (p1 + p2).unwrap_or_else(|e| panic!("period addition should succeed: {e}"));
        assert_eq!(p3.length, 8);
        assert_eq!(p3.units, TimeUnit::Days);
    }

    #[test]
    fn test_period_subtraction() {
        let p1 = Period {
            length: 5,
            units: TimeUnit::Days,
        };
        let p2 = Period {
            length: 3,
            units: TimeUnit::Days,
        };
        let p3 = (p1 - p2).unwrap_or_else(|e| panic!("period subtraction should succeed: {e}"));
        assert_eq!(p3.length, 2);
        assert_eq!(p3.units, TimeUnit::Days);
    }

    #[test]
    fn test_period_multiplication() {
        let mut p = Period {
            length: 5,
            units: TimeUnit::Days,
        };
        p *= 2;
        assert_eq!(p.length, 10);
        assert_eq!(p.units, TimeUnit::Days);
    }

    #[test]
    fn test_normalize_weeks() {
        let mut p = Period {
            length: 7,
            units: TimeUnit::Days,
        };
        p.normalize();
        assert_eq!(p.length, 1);
        assert_eq!(p.units, TimeUnit::Weeks);
    }

    #[test]
    fn test_normalize_years() {
        let mut p = Period {
            length: 12,
            units: TimeUnit::Months,
        };
        p.normalize();
        assert_eq!(p.length, 1);
        assert_eq!(p.units, TimeUnit::Years);
    }

    #[test]
    fn test_period_addition_different_units_years_months() {
        let p1 = Period {
            length: 1,
            units: TimeUnit::Years,
        };
        let p2 = Period {
            length: 6,
            units: TimeUnit::Months,
        };
        let p3 = (p1 + p2).unwrap_or_else(|e| panic!("period addition should succeed: {e}"));
        assert_eq!(p3.length, 18);
        assert_eq!(p3.units, TimeUnit::Months);
    }

    #[test]
    fn test_period_addition_different_units_weeks_days() {
        let p1 = Period {
            length: 2,
            units: TimeUnit::Weeks,
        };
        let p2 = Period {
            length: 3,
            units: TimeUnit::Days,
        };
        let p3 = (p1 + p2).unwrap_or_else(|e| panic!("period addition should succeed: {e}"));
        assert_eq!(p3.length, 17);
        assert_eq!(p3.units, TimeUnit::Days);
    }

    #[test]
    fn test_period_addition_days_weeks() {
        let p1 = Period {
            length: 10,
            units: TimeUnit::Days,
        };
        let p2 = Period {
            length: 1,
            units: TimeUnit::Weeks,
        };
        let p3: Period =
            (p1 + p2).unwrap_or_else(|e| panic!("period addition should succeed: {e}"));
        assert_eq!(p3.length, 17);
        assert_eq!(p3.units, TimeUnit::Days);
    }

    #[test]
    fn test_period_addition_months_years() {
        let p1 = Period {
            length: 6,
            units: TimeUnit::Months,
        };
        let p2 = Period {
            length: 1,
            units: TimeUnit::Years,
        };
        let p3 = (p1 + p2).unwrap_or_else(|e| panic!("period addition should succeed: {e}"));
        assert_eq!(p3.length, 18);
        assert_eq!(p3.units, TimeUnit::Months);
    }

    #[test]
    fn test_impossible_addition_weeks_years() -> Result<()> {
        // should err
        let p1 = Period {
            length: 2,
            units: TimeUnit::Weeks,
        };
        let p2 = Period {
            length: 1,
            units: TimeUnit::Years,
        };
        let r = p1 + p2;
        match r {
            Err(_) => Ok(()),
            Ok(_) => Err(QSError::PeriodOperationErr(
                "impossible addition between 2W and 1Y".to_string(),
            )),
        }
    }

    #[test]
    fn test_impossible_addition_days_years() -> Result<()> {
        let p1 = Period {
            length: 5,
            units: TimeUnit::Days,
        };
        let p2 = Period {
            length: 1,
            units: TimeUnit::Years,
        };
        let r = p1 + p2;
        match r {
            Err(_) => Ok(()),
            Ok(_) => Err(QSError::PeriodOperationErr(
                "impossible addition between 5D and 1Y".to_string(),
            )),
        }
    }

    #[test]
    fn test_period_parsing() {
        let p =
            Period::from_str("1Y").unwrap_or_else(|e| panic!("period parsing should succeed: {e}"));
        assert_eq!(p.length(), 1);
        assert_eq!(p.units(), TimeUnit::Years);

        let p =
            Period::from_str("1M").unwrap_or_else(|e| panic!("period parsing should succeed: {e}"));
        assert_eq!(p.length(), 1);
        assert_eq!(p.units(), TimeUnit::Months);

        let p = Period::from_str("1Y1M")
            .unwrap_or_else(|e| panic!("period parsing should succeed: {e}"));
        assert_eq!(p.length(), 13);
        assert_eq!(p.units(), TimeUnit::Months);
    }
}
