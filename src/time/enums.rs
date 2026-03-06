use std::{
    hash::Hash,
    ops::{Add, Sub},
};

use serde::{Deserialize, Serialize};

use crate::utils::errors::{QSError, Result};

/// # Frequency
/// Enum representing a financial frequency.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum Frequency {
    /// No frequency.
    NoFrequency = -1,
    /// Once.
    Once = 0,
    /// Annual frequency.
    Annual = 1,
    /// Semiannual frequency.
    Semiannual = 2,
    /// Every fourth month frequency.
    EveryFourthMonth = 3,
    /// Quarterly frequency.
    Quarterly = 4,
    /// Bimonthly frequency.
    Bimonthly = 6,
    /// Monthly frequency.
    Monthly = 12,
    /// Every fourth week frequency.
    EveryFourthWeek = 13,
    /// Biweekly frequency.
    Biweekly = 26,
    /// Weekly frequency.
    Weekly = 52,
    /// Daily frequency.
    Daily = 365,
    /// Other frequency.
    OtherFrequency = 999,
}

impl TryFrom<String> for Frequency {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "NoFrequency" => Ok(Self::NoFrequency),
            "Once" => Ok(Self::Once),
            "Annual" => Ok(Self::Annual),
            "Semiannual" => Ok(Self::Semiannual),
            "EveryFourthMonth" => Ok(Self::EveryFourthMonth),
            "Quarterly" => Ok(Self::Quarterly),
            "Bimonthly" => Ok(Self::Bimonthly),
            "Monthly" => Ok(Self::Monthly),
            "EveryFourthWeek" => Ok(Self::EveryFourthWeek),
            "Biweekly" => Ok(Self::Biweekly),
            "Weekly" => Ok(Self::Weekly),
            "Daily" => Ok(Self::Daily),
            "OtherFrequency" => Ok(Self::OtherFrequency),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid frequency: {s}"
            ))),
        }
    }
}

impl From<Frequency> for String {
    fn from(frequency: Frequency) -> Self {
        match frequency {
            Frequency::NoFrequency => "NoFrequency".to_string(),
            Frequency::Once => "Once".to_string(),
            Frequency::Annual => "Annual".to_string(),
            Frequency::Semiannual => "Semiannual".to_string(),
            Frequency::EveryFourthMonth => "EveryFourthMonth".to_string(),
            Frequency::Quarterly => "Quarterly".to_string(),
            Frequency::Bimonthly => "Bimonthly".to_string(),
            Frequency::Monthly => "Monthly".to_string(),
            Frequency::EveryFourthWeek => "EveryFourthWeek".to_string(),
            Frequency::Biweekly => "Biweekly".to_string(),
            Frequency::Weekly => "Weekly".to_string(),
            Frequency::Daily => "Daily".to_string(),
            Frequency::OtherFrequency => "OtherFrequency".to_string(),
        }
    }
}

/// # `TimeUnit`
/// Enum representing a time unit.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum TimeUnit {
    /// Days.
    Days,
    /// Weeks.
    Weeks,
    /// Months.
    Months,
    /// Years.
    Years,
}

impl TryFrom<String> for TimeUnit {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Days" => Ok(Self::Days),
            "Weeks" => Ok(Self::Weeks),
            "Months" => Ok(Self::Months),
            "Years" => Ok(Self::Years),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid time unit: {s}"
            ))),
        }
    }
}

impl From<TimeUnit> for String {
    fn from(time_unit: TimeUnit) -> Self {
        match time_unit {
            TimeUnit::Days => "Days".to_string(),
            TimeUnit::Weeks => "Weeks".to_string(),
            TimeUnit::Months => "Months".to_string(),
            TimeUnit::Years => "Years".to_string(),
        }
    }
}

/// # Month
/// Enum representing a month.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum Month {
    /// January.
    January = 1,
    /// February.
    February,
    /// March.
    March,
    /// April.
    April,
    /// May.
    May,
    /// June.
    June,
    /// July.
    July,
    /// August.
    August,
    /// September.
    September,
    /// October.
    October,
    /// November.
    November,
    /// December.
    December,
}

impl TryFrom<String> for Month {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "January" => Ok(Self::January),
            "February" => Ok(Self::February),
            "March" => Ok(Self::March),
            "April" => Ok(Self::April),
            "May" => Ok(Self::May),
            "June" => Ok(Self::June),
            "July" => Ok(Self::July),
            "August" => Ok(Self::August),
            "September" => Ok(Self::September),
            "October" => Ok(Self::October),
            "November" => Ok(Self::November),
            "December" => Ok(Self::December),
            _ => Err(QSError::InvalidValueErr(format!("Invalid month: {s}"))),
        }
    }
}

impl From<Month> for String {
    fn from(month: Month) -> Self {
        match month {
            Month::January => "January".to_string(),
            Month::February => "February".to_string(),
            Month::March => "March".to_string(),
            Month::April => "April".to_string(),
            Month::May => "May".to_string(),
            Month::June => "June".to_string(),
            Month::July => "July".to_string(),
            Month::August => "August".to_string(),
            Month::September => "September".to_string(),
            Month::October => "October".to_string(),
            Month::November => "November".to_string(),
            Month::December => "December".to_string(),
        }
    }
}

/// # `IMMMonth`
/// Enum representing an IMM month.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum IMMMonth {
    /// F.
    F = 1,
    /// G.
    G = 2,
    /// H.
    H = 3,
    /// J.
    J = 4,
    /// K.
    K = 5,
    /// M.
    M = 6,
    /// N.
    N = 7,
    /// Q.
    Q = 8,
    /// U.
    U = 9,
    /// V.
    V = 10,
    /// X.
    X = 11,
    /// Z.
    Z = 12,
}

/// # `DateGenerationRule`
/// Enum representing a date generation rule.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum DateGenerationRule {
    /// Backward generation rule.
    Backward,
    /// Forward generation rule.
    Forward,
    /// Zero generation rule.
    Zero,
    /// Third Wednesday generation rule.
    ThirdWednesday,
    /// Third Wednesday inclusive generation rule.
    ThirdWednesdayInclusive,
    /// Twentieth generation rule.
    Twentieth,
    /// Twentieth IMM generation rule.
    TwentiethIMM,
    /// Old CDS generation rule.
    OldCDS,
    /// CDS generation rule.
    CDS,
    /// CDS 2015 generation rule.
    CDS2015,
}

impl TryFrom<String> for DateGenerationRule {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Backward" => Ok(Self::Backward),
            "Forward" => Ok(Self::Forward),
            "Zero" => Ok(Self::Zero),
            "ThirdWednesday" => Ok(Self::ThirdWednesday),
            "ThirdWednesdayInclusive" => Ok(Self::ThirdWednesdayInclusive),
            "Twentieth" => Ok(Self::Twentieth),
            "TwentiethIMM" => Ok(Self::TwentiethIMM),
            "OldCDS" => Ok(Self::OldCDS),
            "CDS" => Ok(Self::CDS),
            "CDS2015" => Ok(Self::CDS2015),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid date generation rule: {s}"
            ))),
        }
    }
}

impl From<DateGenerationRule> for String {
    fn from(date_generation_rule: DateGenerationRule) -> Self {
        match date_generation_rule {
            DateGenerationRule::Backward => "Backward".to_string(),
            DateGenerationRule::Forward => "Forward".to_string(),
            DateGenerationRule::Zero => "Zero".to_string(),
            DateGenerationRule::ThirdWednesday => "ThirdWednesday".to_string(),
            DateGenerationRule::ThirdWednesdayInclusive => "ThirdWednesdayInclusive".to_string(),
            DateGenerationRule::Twentieth => "Twentieth".to_string(),
            DateGenerationRule::TwentiethIMM => "TwentiethIMM".to_string(),
            DateGenerationRule::OldCDS => "OldCDS".to_string(),
            DateGenerationRule::CDS => "CDS".to_string(),
            DateGenerationRule::CDS2015 => "CDS2015".to_string(),
        }
    }
}

/// # `BusinessDayConvention`
/// Enum representing a business day convention. Business day conventions are used to
/// adjust a date in case it is not a business day.
///
/// ## Convention
/// * `Following` - Choose the first business day after the given holiday.
/// * `ModifiedFollowing` - Choose the first business day after the given holiday unless
///   it belongs to a different month, in which case choose the first business day before the given holiday.
/// * `Preceding` - Choose the first business day before the given holiday.
/// * `ModifiedPreceding` - Choose the first business day before the given holiday unless
///   it belongs to a different month, in which case choose the first business day after the given holiday.
/// * `Unadjusted` - Do not adjust.
/// * `HalfMonthModifiedFollowing` - Choose the first business day after the given holiday
///   unless that day falls in the first half of the month, in which case choose the first business day before the given holiday.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize, Hash)]
pub enum BusinessDayConvention {
    /// Following convention.
    Following,
    /// Modified following convention.
    ModifiedFollowing,
    /// Preceding convention.
    Preceding,
    /// Modified preceding convention.
    ModifiedPreceding,
    /// Unadjusted convention.
    Unadjusted,
    /// Half month modified following convention.
    HalfMonthModifiedFollowing,
    /// Nearest convention.
    Nearest,
}

impl TryFrom<String> for BusinessDayConvention {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Following" => Ok(Self::Following),
            "ModifiedFollowing" => Ok(Self::ModifiedFollowing),
            "Preceding" => Ok(Self::Preceding),
            "ModifiedPreceding" => Ok(Self::ModifiedPreceding),
            "Unadjusted" => Ok(Self::Unadjusted),
            "HalfMonthModifiedFollowing" => Ok(Self::HalfMonthModifiedFollowing),
            "Nearest" => Ok(Self::Nearest),
            _ => Err(QSError::InvalidValueErr(format!(
                "Invalid business day convention: {s}"
            ))),
        }
    }
}

impl From<BusinessDayConvention> for String {
    fn from(business_day_convention: BusinessDayConvention) -> Self {
        match business_day_convention {
            BusinessDayConvention::Following => "Following".to_string(),
            BusinessDayConvention::ModifiedFollowing => "ModifiedFollowing".to_string(),
            BusinessDayConvention::Preceding => "Preceding".to_string(),
            BusinessDayConvention::ModifiedPreceding => "ModifiedPreceding".to_string(),
            BusinessDayConvention::Unadjusted => "Unadjusted".to_string(),
            BusinessDayConvention::HalfMonthModifiedFollowing => {
                "HalfMonthModifiedFollowing".to_string()
            }
            BusinessDayConvention::Nearest => "Nearest".to_string(),
        }
    }
}

/// # Weekday
/// Enum representing a weekday.
#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum Weekday {
    /// Sunday.
    Sunday = 1,
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
}

impl TryFrom<String> for Weekday {
    type Error = QSError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Sunday" => Ok(Self::Sunday),
            "Monday" => Ok(Self::Monday),
            "Tuesday" => Ok(Self::Tuesday),
            "Wednesday" => Ok(Self::Wednesday),
            "Thursday" => Ok(Self::Thursday),
            "Friday" => Ok(Self::Friday),
            "Saturday" => Ok(Self::Saturday),
            _ => Err(QSError::InvalidValueErr(format!("Invalid weekday: {s}"))),
        }
    }
}

impl From<Weekday> for String {
    fn from(weekday: Weekday) -> Self {
        match weekday {
            Weekday::Sunday => "Sunday".to_string(),
            Weekday::Monday => "Monday".to_string(),
            Weekday::Tuesday => "Tuesday".to_string(),
            Weekday::Wednesday => "Wednesday".to_string(),
            Weekday::Thursday => "Thursday".to_string(),
            Weekday::Friday => "Friday".to_string(),
            Weekday::Saturday => "Saturday".to_string(),
        }
    }
}

impl Add<i32> for Weekday {
    type Output = i32;

    fn add(self, rhs: i32) -> Self::Output {
        self as i32 + rhs
    }
}

impl Sub<i32> for Weekday {
    type Output = i32;

    fn sub(self, rhs: i32) -> Self::Output {
        self + -rhs
    }
}

impl Add<Self> for Weekday {
    type Output = i32;

    fn add(self, rhs: Self) -> Self::Output {
        rhs as i32 + self as i32
    }
}

impl Sub<Self> for Weekday {
    type Output = i32;

    fn sub(self, rhs: Self) -> Self::Output {
        self as i32 + -(rhs as i32)
    }
}

impl Add<Weekday> for i32 {
    type Output = Self;

    fn add(self, rhs: Weekday) -> Self::Output {
        rhs + self
    }
}

impl Sub<Weekday> for i32 {
    type Output = Self;

    fn sub(self, rhs: Weekday) -> Self::Output {
        self + -(rhs as Self)
    }
}

#[cfg(test)]
mod tests {
    use super::Weekday;

    #[test]
    fn test_add() {
        assert_eq!(Weekday::Monday + 1, 3);
    }

    #[test]
    fn test_sub() {
        assert_eq!(Weekday::Monday - 1, 1);
    }

    #[test]
    fn test_add_weekday() {
        assert_eq!(Weekday::Monday + Weekday::Tuesday, 5);
    }

    #[test]
    fn test_sub_weekday() {
        assert_eq!(Weekday::Monday - Weekday::Tuesday, -1);
    }

    #[test]
    fn test_add_i32() {
        assert_eq!(1 + Weekday::Monday, 3);
    }

    #[test]
    fn test_sub_i32() {
        assert_eq!(1 - Weekday::Monday, -1);
    }
}
