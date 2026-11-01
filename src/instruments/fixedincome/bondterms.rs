//! Shared contractual and trade terms for fixed and stepped-coupon bonds.
use crate::{
    ad::scalar::Scalar,
    core::{request::LegsProvider, trade::Side},
    instruments::{
        cashflows::leg::Leg,
        fixedincome::{bondpricinganchor::BondPricingAnchor, callschedule::CallScheduleEntry},
    },
    rates::interestrate::RateDefinition,
    time::{date::Date, daycounter::DayCounter},
    utils::errors::Result,
};

/// Common contract exposed by straight and stepped-coupon bond trades.
pub trait BondTerms<T: Scalar>: LegsProvider<T> {
    /// Returns the trade quote or spread used to anchor bond valuation.
    fn pricing_anchor(&self) -> BondPricingAnchor;
    /// Returns the optional settlement date supplied by the trade.
    fn settlement_date(&self) -> Option<Date>;
    /// Returns the optional day count used to apply the bond spread.
    fn spread_day_counter(&self) -> Option<DayCounter>;
    /// Returns the single bond cashflow leg.
    fn bond_leg(&self) -> &Leg<T>;
    /// Returns the quote units, normally 100.
    fn units(&self) -> f64;
    /// Returns the issuer call schedule.
    fn call_schedule(&self) -> &[CallScheduleEntry];
    /// Returns the convention for yield and duration.
    ///
    /// # Errors
    /// Rejects bond types without a yield-to-maturity convention.
    fn yield_rate_definition(&self) -> Result<RateDefinition>;
    /// Returns the instrument identifier.
    fn bond_identifier(&self) -> String;
    /// Returns the signed trade side.
    fn position_side(&self) -> Side;
}
