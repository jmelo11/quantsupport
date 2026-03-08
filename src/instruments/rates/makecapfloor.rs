use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    indices::rateindex::RateIndexDetails,
    instruments::rates::{
        capfloor::{CapFloor, CapFloorType},
        capletfloorlet::{CapletFloorlet, CapletFloorletType},
    },
    rates::interestrate::RateDefinition,
    time::{
        calendar::Calendar,
        calendars::nullcalendar::NullCalendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
        schedule::MakeSchedule,
    },
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// A builder for creating a [`CapFloor`] strip.
#[derive(Default)]
pub struct MakeCapFloor {
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    strike: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    rate_definition: Option<RateDefinition>,
    market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    cap_floor_type: Option<CapFloorType>,
    frequency: Option<Frequency>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
}

impl MakeCapFloor {
    /// Sets the start date.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the maturity date.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self
    }

    /// Sets the strike rate.
    #[must_use]
    pub const fn with_strike(mut self, strike: f64) -> Self {
        self.strike = Some(strike);
        self
    }

    /// Sets the notional amount.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the rate definition for the caplet/floorlet strip.
    #[must_use]
    pub const fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the market index for the caplet/floorlet strip.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side (buyer or seller of the cap/floor).
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the cap/floor type.
    #[must_use]
    pub const fn with_cap_floor_type(mut self, cap_floor_type: CapFloorType) -> Self {
        self.cap_floor_type = Some(cap_floor_type);
        self
    }

    /// Sets the payment frequency.
    #[must_use]
    pub const fn with_frequency(mut self, frequency: Frequency) -> Self {
        self.frequency = Some(frequency);
        self
    }

    /// Sets the calendar for business day adjustments.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Sets the business day convention.
    #[must_use]
    pub const fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Sets the date generation rule.
    #[must_use]
    pub const fn with_date_generation_rule(mut self, rule: DateGenerationRule) -> Self {
        self.date_generation_rule = Some(rule);
        self
    }

    /// Sets the end-of-month flag.
    #[must_use]
    pub const fn with_end_of_month(mut self, eom: bool) -> Self {
        self.end_of_month = Some(eom);
        self
    }

    /// Builds the [`CapFloor`] instance.
    ///
    /// # Errors
    /// Returns an error when required fields are missing or the schedule build fails.
    pub fn build(self) -> Result<CapFloor> {
        let _notional = self
            .notional
            .ok_or_else(|| QSError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| QSError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| QSError::ValueNotSetErr("Maturity date".into()))?;
        let strike = self
            .strike
            .ok_or_else(|| QSError::ValueNotSetErr("Strike".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let market_index = self
            .market_index
            .ok_or_else(|| QSError::ValueNotSetErr("Market index".into()))?;
        let identifier = self
            .identifier
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let cap_floor_type = self
            .cap_floor_type
            .ok_or_else(|| QSError::ValueNotSetErr("CapFloorType".into()))?;

        let _side = self.side.unwrap_or(Side::LongRecieve);
        let frequency = self.frequency.unwrap_or(Frequency::Quarterly);
        let rate_definition = if let Some(rd) = self.rate_definition {
            rd
        } else {
            market_index.rate_index_details()?.rate_definition()
        };

        let strike_spec = Strike::Absolute(strike);
        let option_type = match cap_floor_type {
            CapFloorType::Cap => CapletFloorletType::Caplet,
            CapFloorType::Floor => CapletFloorletType::Floorlet,
        };

        let schedule = MakeSchedule::new(start_date, maturity_date)
            .with_frequency(frequency)
            .with_calendar(
                self.calendar
                    .unwrap_or(Calendar::NullCalendar(NullCalendar::new())),
            )
            .with_convention(
                self.business_day_convention
                    .unwrap_or(BusinessDayConvention::ModifiedFollowing),
            )
            .with_termination_date_convention(
                self.business_day_convention
                    .unwrap_or(BusinessDayConvention::ModifiedFollowing),
            )
            .with_rule(
                self.date_generation_rule
                    .unwrap_or(DateGenerationRule::Backward),
            )
            .end_of_month(self.end_of_month.unwrap_or(false))
            .build()?;

        let dates = schedule.dates();
        if dates.len() < 2 {
            return Err(QSError::InvalidValueErr(
                "CapFloor schedule must have at least two dates".into(),
            ));
        }

        let mut caplet_floorlets = Vec::with_capacity(dates.len().saturating_sub(1));
        for window in dates.windows(2) {
            let period_start = window[0];
            let period_end = window[1];
            let payment_date = period_end;
            let name = format!("{identifier}:{period_start}-{period_end}");

            caplet_floorlets.push(CapletFloorlet::new(
                name,
                market_index.clone(),
                period_start,
                period_end,
                payment_date,
                option_type,
                strike_spec,
                rate_definition,
            ));
        }

        Ok(CapFloor::new(
            identifier,
            caplet_floorlets,
            market_index,
            currency,
            strike,
            cap_floor_type,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::instrument::Instrument;

    fn base_builder() -> MakeCapFloor {
        MakeCapFloor::default()
            .with_identifier("capfloor_test".to_string())
            .with_start_date(Date::new(2024, 1, 1))
            .with_maturity_date(Date::new(2025, 1, 1))
            .with_strike(0.03)
            .with_notional(1_000_000.0)
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_cap_floor_type(CapFloorType::Cap)
    }

    #[test]
    fn test_build_capfloor_success() {
        let result = base_builder().build();
        assert!(result.is_ok(), "expected cap/floor build to succeed");

        let capfloor = result.unwrap();
        assert_eq!(capfloor.identifier(), "capfloor_test");
        assert_eq!(capfloor.currency(), Currency::USD);
        assert_eq!(capfloor.market_index(), MarketIndex::SOFR);
        assert_eq!(capfloor.strike(), 0.03);
        assert!(!capfloor.caplet_floorlets().is_empty());

        let expected_rate_definition = MarketIndex::SOFR
            .rate_index_details()
            .unwrap()
            .rate_definition();
        assert_eq!(
            capfloor.caplet_floorlets()[0].rate_definition(),
            expected_rate_definition
        );
    }

    #[test]
    fn test_build_capfloor_missing_strike_fails() {
        let result = MakeCapFloor::default()
            .with_identifier("capfloor_missing_strike".to_string())
            .with_start_date(Date::new(2024, 1, 1))
            .with_maturity_date(Date::new(2025, 1, 1))
            .with_notional(1_000_000.0)
            .with_market_index(MarketIndex::SOFR)
            .with_currency(Currency::USD)
            .with_cap_floor_type(CapFloorType::Cap)
            .build();

        assert!(result.is_err(), "expected missing strike to fail");
    }
}
