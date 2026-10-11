use serde::{Deserialize, Serialize};

use crate::{
    indices::marketindex::MarketIndex,
    time::{
        calendar::Calendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
        period::Period,
    },
};

/// Overnight coupon averaging, independent of annualized rate compounding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OvernightAveraging {
    /// Product of daily simple interest factors.
    #[default]
    Compound,
    /// Accrual-weighted arithmetic average of daily rates.
    Simple,
}

/// Date used as an OIS calibration node.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum OisPillar {
    /// Last payment or final projected overnight value date, whichever is later.
    #[default]
    LastRelevantDate,
    /// Final accrual end date.
    MaturityDate,
    /// Explicit node date inside the instrument's relevant date range.
    CustomDate(Date),
}

/// Observation conventions for an overnight coupon.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OvernightCouponConventions {
    /// Daily compounding or arithmetic averaging.
    averaging: OvernightAveraging,
    /// Business days between observation and interest dates.
    lookback_days: i32,
    /// Final business-day observations frozen at the preceding observation.
    lockout_days: usize,
    /// Use shifted observation dates for daily accrual weights.
    observation_shift: bool,
    /// Include spread inside each daily interest factor.
    compound_spread_daily: bool,
}

impl OvernightCouponConventions {
    /// Creates standard daily-compounded overnight conventions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the averaging method.
    #[must_use]
    pub const fn averaging(&self) -> OvernightAveraging {
        self.averaging
    }

    /// Sets the averaging method.
    #[must_use]
    pub const fn with_averaging(mut self, averaging: OvernightAveraging) -> Self {
        self.averaging = averaging;
        self
    }

    /// Returns the observation lookback in business days.
    #[must_use]
    pub const fn lookback_days(&self) -> i32 {
        self.lookback_days
    }

    /// Sets the observation lookback in business days.
    #[must_use]
    pub const fn with_lookback_days(mut self, days: i32) -> Self {
        self.lookback_days = days;
        self
    }

    /// Returns the number of final observations subject to lockout.
    #[must_use]
    pub const fn lockout_days(&self) -> usize {
        self.lockout_days
    }

    /// Sets the number of final observations subject to lockout.
    #[must_use]
    pub const fn with_lockout_days(mut self, days: usize) -> Self {
        self.lockout_days = days;
        self
    }

    /// Returns whether shifted observation dates determine accrual weights.
    #[must_use]
    pub const fn observation_shift(&self) -> bool {
        self.observation_shift
    }

    /// Sets whether shifted observation dates determine accrual weights.
    #[must_use]
    pub const fn with_observation_shift(mut self, enabled: bool) -> Self {
        self.observation_shift = enabled;
        self
    }

    /// Returns whether the spread is included in daily compound factors.
    #[must_use]
    pub const fn compound_spread_daily(&self) -> bool {
        self.compound_spread_daily
    }

    /// Sets whether the spread is included in daily compound factors.
    #[must_use]
    pub const fn with_compound_spread_daily(mut self, enabled: bool) -> Self {
        self.compound_spread_daily = enabled;
        self
    }
}

/// Contractual OIS conventions, configurable per instrument or quote identifier.
///
/// `Default` follows neutral QuantLib-style settings (two settlement days,
/// zero payment lag). `for_index` supplies the common SOFR two-day payment lag.
/// These presets are not a substitute for a vendor's curve specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct OisConventions {
    /// Business days from valuation to spot.
    settlement_days: i32,
    /// Business days from accrual end to payment.
    payment_lag: i32,
    /// Settlement calendar; otherwise the index calendar.
    settlement_calendar: Option<Calendar>,
    /// Fixed-leg accrual calendar; otherwise the index calendar.
    fixed_calendar: Option<Calendar>,
    /// Floating-leg accrual calendar; otherwise the index calendar.
    overnight_calendar: Option<Calendar>,
    /// Payment calendar; otherwise the index calendar.
    payment_calendar: Option<Calendar>,
    /// Fixed-leg frequency.
    fixed_frequency: Frequency,
    /// Overnight-leg frequency.
    overnight_frequency: Frequency,
    /// Fixed-leg day count; otherwise the index day count.
    fixed_day_counter: Option<DayCounter>,
    /// Accrual date adjustment.
    convention: BusinessDayConvention,
    /// Termination date adjustment; otherwise the accrual convention.
    termination_convention: Option<BusinessDayConvention>,
    /// Payment date adjustment.
    payment_convention: BusinessDayConvention,
    /// Schedule generation direction.
    date_generation_rule: DateGenerationRule,
    /// EOM behavior; otherwise inferred from the effective date.
    end_of_month: Option<bool>,
    /// Forward start relative to spot.
    forward_start: Option<Period>,
    /// Explicit first coupon date for a front stub.
    first_coupon_date: Option<Date>,
    /// Explicit penultimate coupon date for a back stub.
    next_to_last_coupon_date: Option<Date>,
    /// Daily observation and averaging rules.
    overnight: OvernightCouponConventions,
    /// Calibration node choice.
    pillar: OisPillar,
}

impl Default for OisConventions {
    fn default() -> Self {
        Self {
            settlement_days: 2,
            payment_lag: 0,
            settlement_calendar: None,
            fixed_calendar: None,
            overnight_calendar: None,
            payment_calendar: None,
            fixed_frequency: Frequency::Annual,
            overnight_frequency: Frequency::Annual,
            fixed_day_counter: None,
            convention: BusinessDayConvention::ModifiedFollowing,
            termination_convention: None,
            payment_convention: BusinessDayConvention::Following,
            date_generation_rule: DateGenerationRule::Backward,
            end_of_month: None,
            forward_start: None,
            first_coupon_date: None,
            next_to_last_coupon_date: None,
            overnight: OvernightCouponConventions::default(),
            pillar: OisPillar::LastRelevantDate,
        }
    }
}

impl OisConventions {
    /// Applies the trailing positional fields of an OIS quote identifier.
    ///
    /// # Errors
    /// Rejects excess fields, invalid enum spellings and invalid parameter values.
    pub fn with_quote_fields(mut self, fields: &[&str]) -> crate::utils::errors::Result<Self> {
        use crate::utils::errors::QSError;
        if fields.len() > 23 {
            return Err(QSError::InvalidValueErr(
                "OIS identifier has more than 23 optional fields".into(),
            ));
        }
        for (position, &value) in fields.iter().enumerate() {
            if value.is_empty() {
                continue;
            }
            let invalid = || {
                QSError::InvalidValueErr(format!(
                    "Invalid OIS optional field {}: {value}",
                    position + 1
                ))
            };
            let overnight = self.overnight.clone();
            self = match position {
                0 => self.with_fixed_frequency(value.parse()?),
                1 => self.with_overnight_frequency(value.parse()?),
                2 => self.with_settlement_days(
                    value
                        .parse::<i32>()
                        .ok()
                        .filter(|days| *days >= 0)
                        .ok_or_else(invalid)?,
                ),
                3 => self.with_payment_lag(
                    value
                        .parse::<i32>()
                        .ok()
                        .filter(|days| *days >= 0)
                        .ok_or_else(invalid)?,
                ),
                4 => self.with_fixed_day_counter(DayCounter::try_from(value.to_string())?),
                5 => self.with_settlement_calendar(Calendar::try_from(value.to_string())?),
                6 => self.with_fixed_calendar(Calendar::try_from(value.to_string())?),
                7 => self.with_overnight_calendar(Calendar::try_from(value.to_string())?),
                8 => self.with_payment_calendar(Calendar::try_from(value.to_string())?),
                9 => self.with_convention(BusinessDayConvention::try_from(value.to_string())?),
                10 => self.with_termination_convention(BusinessDayConvention::try_from(
                    value.to_string(),
                )?),
                11 => self
                    .with_payment_convention(BusinessDayConvention::try_from(value.to_string())?),
                12 => {
                    self.with_date_generation_rule(DateGenerationRule::try_from(value.to_string())?)
                }
                13 => self.with_end_of_month(value.parse().map_err(|_| invalid())?),
                14 => self.with_forward_start(Period::from_str(value)?),
                15 => self.with_first_coupon_date(Date::from_str(value, "%Y-%m-%d")?),
                16 => self.with_next_to_last_coupon_date(Date::from_str(value, "%Y-%m-%d")?),
                17 => self.with_overnight(overnight.with_averaging(match value {
                    "Compound" => OvernightAveraging::Compound,
                    "Simple" => OvernightAveraging::Simple,
                    _ => return Err(invalid()),
                })),
                18 => self.with_overnight(
                    overnight.with_lookback_days(
                        value
                            .parse::<i32>()
                            .ok()
                            .filter(|days| *days >= 0)
                            .ok_or_else(invalid)?,
                    ),
                ),
                19 => self.with_overnight(
                    overnight.with_lockout_days(value.parse().map_err(|_| invalid())?),
                ),
                20 => self.with_overnight(
                    overnight.with_observation_shift(value.parse().map_err(|_| invalid())?),
                ),
                21 => self.with_overnight(
                    overnight.with_compound_spread_daily(value.parse().map_err(|_| invalid())?),
                ),
                22 => self.with_pillar(match value {
                    "LastRelevantDate" => OisPillar::LastRelevantDate,
                    "MaturityDate" => OisPillar::MaturityDate,
                    _ => OisPillar::CustomDate(Date::from_str(
                        value
                            .strip_prefix("CustomDate(")
                            .and_then(|date| date.strip_suffix(')'))
                            .ok_or_else(invalid)?,
                        "%Y-%m-%d",
                    )?),
                }),
                _ => return Err(invalid()),
            };
        }
        Ok(self)
    }

    /// Creates neutral QuantLib-style OIS conventions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the settlement lag in business days.
    #[must_use]
    pub const fn settlement_days(&self) -> i32 {
        self.settlement_days
    }

    /// Sets the settlement lag in business days.
    #[must_use]
    pub const fn with_settlement_days(mut self, days: i32) -> Self {
        self.settlement_days = days;
        self
    }

    /// Returns the payment lag in business days.
    #[must_use]
    pub const fn payment_lag(&self) -> i32 {
        self.payment_lag
    }

    /// Sets the payment lag in business days.
    #[must_use]
    pub const fn with_payment_lag(mut self, days: i32) -> Self {
        self.payment_lag = days;
        self
    }

    /// Returns the settlement calendar override.
    #[must_use]
    pub const fn settlement_calendar(&self) -> Option<&Calendar> {
        self.settlement_calendar.as_ref()
    }

    /// Sets the settlement calendar override.
    #[must_use]
    pub fn with_settlement_calendar(mut self, calendar: Calendar) -> Self {
        self.settlement_calendar = Some(calendar);
        self
    }

    /// Returns the fixed-leg accrual calendar override.
    #[must_use]
    pub const fn fixed_calendar(&self) -> Option<&Calendar> {
        self.fixed_calendar.as_ref()
    }

    /// Sets the fixed-leg accrual calendar override.
    #[must_use]
    pub fn with_fixed_calendar(mut self, calendar: Calendar) -> Self {
        self.fixed_calendar = Some(calendar);
        self
    }

    /// Returns the overnight-leg accrual calendar override.
    #[must_use]
    pub const fn overnight_calendar(&self) -> Option<&Calendar> {
        self.overnight_calendar.as_ref()
    }

    /// Sets the overnight-leg accrual calendar override.
    #[must_use]
    pub fn with_overnight_calendar(mut self, calendar: Calendar) -> Self {
        self.overnight_calendar = Some(calendar);
        self
    }

    /// Returns the payment calendar override.
    #[must_use]
    pub const fn payment_calendar(&self) -> Option<&Calendar> {
        self.payment_calendar.as_ref()
    }

    /// Sets the payment calendar override.
    #[must_use]
    pub fn with_payment_calendar(mut self, calendar: Calendar) -> Self {
        self.payment_calendar = Some(calendar);
        self
    }

    /// Returns the fixed-leg payment frequency.
    #[must_use]
    pub const fn fixed_frequency(&self) -> Frequency {
        self.fixed_frequency
    }

    /// Sets the fixed-leg payment frequency.
    #[must_use]
    pub const fn with_fixed_frequency(mut self, frequency: Frequency) -> Self {
        self.fixed_frequency = frequency;
        self
    }

    /// Returns the overnight-leg payment frequency.
    #[must_use]
    pub const fn overnight_frequency(&self) -> Frequency {
        self.overnight_frequency
    }

    /// Sets the overnight-leg payment frequency.
    #[must_use]
    pub const fn with_overnight_frequency(mut self, frequency: Frequency) -> Self {
        self.overnight_frequency = frequency;
        self
    }

    /// Returns the fixed-leg day-count override.
    #[must_use]
    pub const fn fixed_day_counter(&self) -> Option<DayCounter> {
        self.fixed_day_counter
    }

    /// Sets the fixed-leg day-count override.
    #[must_use]
    pub const fn with_fixed_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.fixed_day_counter = Some(day_counter);
        self
    }

    /// Returns the accrual business-day convention.
    #[must_use]
    pub const fn convention(&self) -> BusinessDayConvention {
        self.convention
    }

    /// Sets the accrual business-day convention.
    #[must_use]
    pub const fn with_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.convention = convention;
        self
    }

    /// Returns the termination-date convention override.
    #[must_use]
    pub const fn termination_convention(&self) -> Option<BusinessDayConvention> {
        self.termination_convention
    }

    /// Sets the termination-date convention override.
    #[must_use]
    pub const fn with_termination_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.termination_convention = Some(convention);
        self
    }

    /// Returns the payment business-day convention.
    #[must_use]
    pub const fn payment_convention(&self) -> BusinessDayConvention {
        self.payment_convention
    }

    /// Sets the payment business-day convention.
    #[must_use]
    pub const fn with_payment_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.payment_convention = convention;
        self
    }

    /// Returns the schedule generation rule.
    #[must_use]
    pub const fn date_generation_rule(&self) -> DateGenerationRule {
        self.date_generation_rule
    }

    /// Sets the schedule generation rule.
    #[must_use]
    pub const fn with_date_generation_rule(mut self, rule: DateGenerationRule) -> Self {
        self.date_generation_rule = rule;
        self
    }

    /// Returns the EOM override, or `None` to infer it from the effective date.
    #[must_use]
    pub const fn end_of_month(&self) -> Option<bool> {
        self.end_of_month
    }

    /// Sets whether schedules follow end-of-month dates.
    #[must_use]
    pub const fn with_end_of_month(mut self, enabled: bool) -> Self {
        self.end_of_month = Some(enabled);
        self
    }

    /// Returns the forward start relative to spot.
    #[must_use]
    pub const fn forward_start(&self) -> Option<Period> {
        self.forward_start
    }

    /// Sets the forward start relative to spot.
    #[must_use]
    pub const fn with_forward_start(mut self, period: Period) -> Self {
        self.forward_start = Some(period);
        self
    }

    /// Returns the explicit first coupon date.
    #[must_use]
    pub const fn first_coupon_date(&self) -> Option<Date> {
        self.first_coupon_date
    }

    /// Sets the explicit first coupon date.
    #[must_use]
    pub const fn with_first_coupon_date(mut self, date: Date) -> Self {
        self.first_coupon_date = Some(date);
        self
    }

    /// Returns the explicit penultimate coupon date.
    #[must_use]
    pub const fn next_to_last_coupon_date(&self) -> Option<Date> {
        self.next_to_last_coupon_date
    }

    /// Sets the explicit penultimate coupon date.
    #[must_use]
    pub const fn with_next_to_last_coupon_date(mut self, date: Date) -> Self {
        self.next_to_last_coupon_date = Some(date);
        self
    }

    /// Returns the overnight observation conventions.
    #[must_use]
    pub const fn overnight(&self) -> &OvernightCouponConventions {
        &self.overnight
    }

    /// Sets the overnight observation conventions.
    #[must_use]
    pub const fn with_overnight(mut self, conventions: OvernightCouponConventions) -> Self {
        self.overnight = conventions;
        self
    }

    /// Returns the calibration pillar choice.
    #[must_use]
    pub const fn pillar(&self) -> OisPillar {
        self.pillar
    }

    /// Sets the calibration pillar choice.
    #[must_use]
    pub const fn with_pillar(mut self, pillar: OisPillar) -> Self {
        self.pillar = pillar;
        self
    }

    /// Common market preset, with explicit overrides available on every field.
    #[must_use]
    pub fn for_index(index: &MarketIndex) -> Self {
        Self {
            settlement_days: match index {
                MarketIndex::SONIA => 0,
                MarketIndex::CORRA => 1,
                _ => 2,
            },
            payment_lag: if matches!(index, MarketIndex::SOFR | MarketIndex::SOFRCompounded) {
                2
            } else {
                0
            },
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::enums::TimeUnit;

    #[test]
    fn ois_convention_accessors_preserve_overrides_and_serde() {
        let overnight = OvernightCouponConventions::new()
            .with_averaging(OvernightAveraging::Simple)
            .with_lookback_days(2)
            .with_lockout_days(1)
            .with_observation_shift(true)
            .with_compound_spread_daily(true);
        assert_eq!(overnight.averaging(), OvernightAveraging::Simple);
        assert_eq!(overnight.lookback_days(), 2);
        assert_eq!(overnight.lockout_days(), 1);
        assert!(overnight.observation_shift());
        assert!(overnight.compound_spread_daily());

        let calendar = MarketIndex::SOFR
            .rate_index_details()
            .unwrap()
            .calendar()
            .clone();
        let first = Date::new(2027, 1, 4);
        let penultimate = Date::new(2028, 1, 4);
        let forward = Period::new(1, TimeUnit::Months);
        let conventions = OisConventions::new()
            .with_settlement_days(0)
            .with_payment_lag(2)
            .with_settlement_calendar(calendar.clone())
            .with_fixed_calendar(calendar.clone())
            .with_overnight_calendar(calendar.clone())
            .with_payment_calendar(calendar)
            .with_fixed_frequency(Frequency::Semiannual)
            .with_overnight_frequency(Frequency::Quarterly)
            .with_fixed_day_counter(DayCounter::Actual365)
            .with_convention(BusinessDayConvention::Following)
            .with_termination_convention(BusinessDayConvention::Preceding)
            .with_payment_convention(BusinessDayConvention::ModifiedFollowing)
            .with_date_generation_rule(DateGenerationRule::Forward)
            .with_end_of_month(false)
            .with_forward_start(forward)
            .with_first_coupon_date(first)
            .with_next_to_last_coupon_date(penultimate)
            .with_overnight(overnight.clone())
            .with_pillar(OisPillar::CustomDate(penultimate));
        assert_eq!(conventions.settlement_days(), 0);
        assert_eq!(conventions.payment_lag(), 2);
        assert!(conventions.settlement_calendar().is_some());
        assert!(conventions.fixed_calendar().is_some());
        assert!(conventions.overnight_calendar().is_some());
        assert!(conventions.payment_calendar().is_some());
        assert_eq!(conventions.fixed_frequency(), Frequency::Semiannual);
        assert_eq!(conventions.overnight_frequency(), Frequency::Quarterly);
        assert_eq!(conventions.fixed_day_counter(), Some(DayCounter::Actual365));
        assert_eq!(conventions.convention(), BusinessDayConvention::Following);
        assert_eq!(
            conventions.termination_convention(),
            Some(BusinessDayConvention::Preceding)
        );
        assert_eq!(
            conventions.payment_convention(),
            BusinessDayConvention::ModifiedFollowing
        );
        assert_eq!(
            conventions.date_generation_rule(),
            DateGenerationRule::Forward
        );
        assert_eq!(conventions.end_of_month(), Some(false));
        assert_eq!(conventions.forward_start(), Some(forward));
        assert_eq!(conventions.first_coupon_date(), Some(first));
        assert_eq!(conventions.next_to_last_coupon_date(), Some(penultimate));
        assert_eq!(conventions.overnight(), &overnight);
        assert!(matches!(conventions.pillar(), OisPillar::CustomDate(date) if date == penultimate));

        let serialized = serde_json::to_value(&conventions).unwrap();
        let restored: OisConventions = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), serialized);
    }

    #[test]
    fn ois_convention_constructors_preserve_defaults() {
        let overnight = OvernightCouponConventions::new();
        assert_eq!(overnight, OvernightCouponConventions::default());
        let conventions = OisConventions::new();
        assert_eq!(conventions.settlement_days(), 2);
        assert_eq!(conventions.payment_lag(), 0);
        assert!(conventions.settlement_calendar().is_none());
        assert!(conventions.fixed_day_counter().is_none());
        assert!(conventions.end_of_month().is_none());
        assert_eq!(conventions.overnight(), &overnight);
        assert!(matches!(conventions.pillar(), OisPillar::LastRelevantDate));
        assert_eq!(
            OisConventions::for_index(&MarketIndex::SOFR).payment_lag(),
            2
        );
        assert_eq!(
            OisConventions::for_index(&MarketIndex::SONIA).settlement_days(),
            0
        );
    }
}
