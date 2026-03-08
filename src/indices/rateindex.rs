use crate::{
    currencies::currency::Currency,
    indices::marketindex::{MarketIndex, MarketIndexDetails},
    rates::interestrate::RateDefinition,
    time::calendar::Calendar,
};

/// # `InterestRateIndex`
pub trait RateIndexDetails: MarketIndexDetails {
    /// Currency associated with the index.
    fn currency(&self) -> Currency;
    /// Calendar of publication of the index.
    fn calendar(&self) -> Calendar;
    /// Rate convention of the index.
    fn rate_definition(&self) -> RateDefinition;
    /// Fixing lag (T+Days).
    fn fixing_lag(&self) -> i64;
    /// Enum related to the index.
    fn market_index(&self) -> MarketIndex;
}
