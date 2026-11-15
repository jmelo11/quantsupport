//! Time and date utilities.
//!
//! Dates, calendars, day-count conventions, schedules, periods,
//! and IMM date helpers.

/// Defines the calendar interface used to adjust dates and count business days.
pub mod calendar;
/// Provides the holiday calendars supported by date and schedule calculations.
pub mod calendars;
/// Represents calendar dates and implements their financial date arithmetic.
pub mod date;
/// Represents UTC timestamps used to identify market observations.
pub mod datetime;
/// Selects the day-count convention used to turn date intervals into year fractions.
pub mod daycounter;
/// Implements the supported day-count conventions and their accrual calculations.
pub mod daycounters;
/// Defines the frequencies and date-adjustment conventions used by schedules.
pub mod enums;
/// Calculates the IMM dates used by supported futures contracts.
pub mod imm;
/// Represents tenors and other financial time periods.
pub mod period;
/// Generates contractual date schedules under the selected calendar conventions.
pub mod schedule;
