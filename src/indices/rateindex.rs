use crate::{
    currencies::currency::Currency,
    indices::marketindex::{MarketIndex, MarketIndexDetails},
    rates::interestrate::RateDefinition,
    time::{
        calendar::Calendar,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
};

/// Default terms for a fixed-floating swap referencing a rate index.
/// Explicit contract terms override these defaults independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SwapConventions {
    fixed_rate_definition: RateDefinition,
    fixed_frequency: Frequency,
    floating_frequency: Frequency,
    floating_day_counter: DayCounter,
    settlement_days: i32,
    business_day_convention: BusinessDayConvention,
}

impl SwapConventions {
    /// Creates an index's swap convention preset.
    #[must_use]
    pub const fn new(
        fixed_rate_definition: RateDefinition,
        fixed_frequency: Frequency,
        floating_frequency: Frequency,
        floating_day_counter: DayCounter,
        settlement_days: i32,
        business_day_convention: BusinessDayConvention,
    ) -> Self {
        Self {
            fixed_rate_definition,
            fixed_frequency,
            floating_frequency,
            floating_day_counter,
            settlement_days,
            business_day_convention,
        }
    }

    /// Fixed coupon day count and compounding definition.
    #[must_use]
    pub const fn fixed_rate_definition(&self) -> RateDefinition {
        self.fixed_rate_definition
    }

    /// Fixed coupon payment frequency.
    #[must_use]
    pub const fn fixed_frequency(&self) -> Frequency {
        self.fixed_frequency
    }

    /// Floating coupon payment frequency, independent of daily observation frequency.
    #[must_use]
    pub const fn floating_frequency(&self) -> Frequency {
        self.floating_frequency
    }

    /// Floating coupon accrual day count.
    #[must_use]
    pub const fn floating_day_counter(&self) -> DayCounter {
        self.floating_day_counter
    }

    /// Business days from the reference date to swap settlement.
    #[must_use]
    pub const fn settlement_days(&self) -> i32 {
        self.settlement_days
    }

    /// Business-day adjustment for both coupon schedules.
    #[must_use]
    pub const fn business_day_convention(&self) -> BusinessDayConvention {
        self.business_day_convention
    }
}

/// # `InterestRateIndex`
pub trait RateIndexDetails: MarketIndexDetails {
    /// Currency associated with the index.
    fn currency(&self) -> Currency;
    /// Calendar of publication of the index.
    fn calendar(&self) -> Calendar;
    /// Rate convention of the index.
    fn rate_definition(&self) -> RateDefinition;
    /// Swap defaults owned by the index, allowing maturity-dependent schedules.
    fn swap_conventions(&self, tenor: Period) -> SwapConventions;
    /// Fixing lag (T+Days).
    fn fixing_lag(&self) -> i64;
    /// Enum related to the index.
    fn market_index(&self) -> MarketIndex;
    /// Observation tenor frequency, independent of the rate's compounding convention.
    fn frequency(&self) -> Frequency {
        Frequency::Daily
    }
    /// Whether the index fixes in arrears (e.g. overnight RFRs like SOFR, SONIA).
    /// Term rates (e.g. Term SOFR, EURIBOR) fix in advance and return `false`.
    fn is_in_arrears(&self) -> bool {
        true
    }
}
