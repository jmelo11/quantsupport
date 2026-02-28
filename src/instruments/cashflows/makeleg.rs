use std::collections::{HashMap, HashSet};

use crate::{
    ad::adreal::{ADReal, IsReal},
    core::trade::Side,
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{
        cashflow::SimpleCashflow, cashflowtype::CashflowType, coupons::PayoffOps,
        fixedratecoupon::FixedRateCoupon, floatingratecoupon::FloatingRateCoupon, leg::Leg,
        optionembeddedcoupon::OptionEmbeddedCoupon,
    },
    rates::interestrate::InterestRate,
    time::{
        calendar::Calendar,
        calendars::nullcalendar::NullCalendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
        period::Period,
        schedule::MakeSchedule,
    },
    utils::errors::{AtlasError, Result},
};

/// Enumeration for the type of rate used in the leg, either fixed or floating.
#[derive(Clone, Copy)]
pub enum RateType {
    /// Fixed rate
    Fixed,
    /// Floating rate
    Floating,
}

/// Payment structure for the leg.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaymentStructure {
    /// Bullet structure: all principal is paid at maturity, with regular coupons.
    Bullet,
    /// Equal payments structure: each payment (coupon + principal) is the same, with principal amortizing over time.
    EqualPayments,
    /// Equal redemptions structure: principal is amortized in equal amounts over the payment schedule, with coupons calculated on the outstanding notional.
    EqualRedemptions,
    /// Zero structure: no coupons, only a single payment at maturity for the notional amount.
    Zero,
    /// Other structure: allows for arbitrary cash flow patterns defined by the user, with disbursements and redemptions specified as date-amount pairs.
    Other,
}

/// [`MakeLeg`] is a builder for a flegs.
#[derive(Clone, Default)]
pub struct MakeLeg {
    // common fields
    leg_id: Option<usize>,
    start_date: Option<Date>,
    end_date: Option<Date>,
    first_coupon_date: Option<Date>,
    payment_frequency: Option<Frequency>,
    tenor: Option<Period>,
    currency: Option<Currency>,
    side: Option<Side>,
    notional: Option<f64>,
    structure: Option<PaymentStructure>,
    redemptions: Option<HashMap<Date, f64>>,
    end_of_month: Option<bool>,
    calendar: Option<Calendar>,
    additional_coupon_dates: Option<HashSet<Date>>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,

    rate_type: Option<RateType>,

    // floating rate specific fields
    spread: Option<f64>,
    market_index: Option<MarketIndex>,

    // fixed rate specific fields
    rate: Option<InterestRate<ADReal>>,
    disbursements: Option<HashMap<Date, f64>>,

    // option-embedded structures
    floorlet_strike: Option<f64>,
    caplet_strike: Option<f64>,
    payoff_ops: Option<PayoffOps>,
}

/// New, setters and getters
impl MakeLeg {
    /// Sets the end of month flag.
    #[must_use]
    pub const fn with_end_of_month(mut self, end_of_month: Option<bool>) -> Self {
        self.end_of_month = end_of_month;
        self
    }

    /// Set the leg id.
    #[must_use]
    pub const fn set_leg_id(mut self, leg_id: usize) -> Self {
        self.leg_id = Some(leg_id);
        self
    }

    /// Sets the first coupon date.
    #[must_use]
    pub const fn with_first_coupon_date(mut self, first_coupon_date: Option<Date>) -> Self {
        self.first_coupon_date = first_coupon_date;
        self
    }

    /// Sets the floor of each coupon. This is only relevant for option-embedded structures (floating rate coupons).
    #[must_use]
    pub const fn with_floorlet_strike(mut self, floor: Option<f64>) -> Self {
        self.floorlet_strike = floor;
        self
    }

    /// Sets the cap of each coupon. This is only relevant for option-embedded structures (floating rate coupons).
    #[must_use]
    pub const fn with_caplet_strike(mut self, cap: Option<f64>) -> Self {
        self.caplet_strike = cap;
        self
    }

    /// Sets the rate type.
    #[must_use]
    pub const fn with_rate_type(mut self, rate_type: RateType) -> Self {
        self.rate_type = Some(rate_type);
        self
    }

    /// Sets the currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the notional.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the calendar.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Option<Calendar>) -> Self {
        self.calendar = calendar;
        self
    }

    /// Sets the business day convention.
    #[must_use]
    pub const fn with_business_day_convention(
        mut self,
        business_day_convention: Option<BusinessDayConvention>,
    ) -> Self {
        self.business_day_convention = business_day_convention;
        self
    }

    /// Sets the date generation rule.
    #[must_use]
    pub const fn with_date_generation_rule(
        mut self,
        date_generation_rule: Option<DateGenerationRule>,
    ) -> Self {
        self.date_generation_rule = date_generation_rule;
        self
    }

    /// Sets the start date.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the end date.
    #[must_use]
    pub const fn with_end_date(mut self, end_date: Date) -> Self {
        self.end_date = Some(end_date);
        self
    }

    /// Sets the disbursements.
    #[must_use]
    pub fn with_disbursements(mut self, disbursements: HashMap<Date, f64>) -> Self {
        self.disbursements = Some(disbursements);
        self
    }

    /// Sets the redemptions.
    #[must_use]
    pub fn with_redemptions(mut self, redemptions: HashMap<Date, f64>) -> Self {
        self.redemptions = Some(redemptions);
        self
    }

    /// Sets the rate.
    #[must_use]
    pub const fn with_rate(mut self, rate: InterestRate<ADReal>) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the tenor.
    #[must_use]
    pub const fn with_tenor(mut self, tenor: Period) -> Self {
        self.tenor = Some(tenor);
        self
    }

    /// Sets the payment frequency.
    #[must_use]
    pub const fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the market index for floating rate or option-embedded legs.
    #[must_use]
    pub fn with_market_index(mut self, market_index: MarketIndex) -> Self {
        self.market_index = Some(market_index);
        self
    }

    /// Sets the spread for floating rate or option-embedded legs.
    #[must_use]
    pub const fn with_spread(mut self, spread: f64) -> Self {
        self.spread = Some(spread);
        self
    }

    /// Sets the structure to bullet.
    #[must_use]
    pub const fn bullet(mut self) -> Self {
        self.structure = Some(PaymentStructure::Bullet);
        self
    }

    /// Sets the structure to equal redemptions.
    #[must_use]
    pub const fn equal_redemptions(mut self) -> Self {
        self.structure = Some(PaymentStructure::EqualRedemptions);
        self
    }

    /// Sets the structure to zero.
    #[must_use]
    pub const fn zero(mut self) -> Self {
        self.structure = Some(PaymentStructure::Zero);
        self.payment_frequency = Some(Frequency::Once);
        self
    }

    /// Sets the structure to equal payments.
    #[must_use]
    pub const fn equal_payments(mut self) -> Self {
        self.structure = Some(PaymentStructure::EqualPayments);
        self
    }

    /// Sets the structure to other.
    #[must_use]
    pub const fn other(mut self) -> Self {
        self.structure = Some(PaymentStructure::Other);
        self.payment_frequency = Some(Frequency::OtherFrequency);
        self
    }

    /// Sets the structure.
    #[must_use]
    pub const fn with_structure(mut self, structure: PaymentStructure) -> Self {
        self.structure = Some(structure);
        self
    }
}

enum LegType {
    FixedRate,
    FloatingRate,
    OptionEmbedded,
}

impl MakeLeg {
    /// Checks the consistency of the configured fields and infers the leg type.
    fn check_leg_type(&self) -> Result<LegType> {
        match self.rate_type {
            Some(RateType::Fixed) => {
                if self.caplet_strike.is_some() || self.floorlet_strike.is_some() {
                    Err(AtlasError::InvalidValueErr(
                        "Caplet and floorlet strikes should not be set for fixed rate leg".into(),
                    ))
                } else {
                    Ok(LegType::FixedRate)
                }
            }
            Some(RateType::Floating) => {
                if self.structure == Some(PaymentStructure::EqualPayments) {
                    return Err(AtlasError::InvalidValueErr(
                        "Equal payments structure is not compatible with floating rate leg".into(),
                    ));
                }
                if self.caplet_strike.is_some() || self.floorlet_strike.is_some() {
                    Ok(LegType::OptionEmbedded)
                } else {
                    Ok(LegType::FloatingRate)
                }
            }
            None => Err(AtlasError::ValueNotSetErr("Rate type".into())),
        }
    }

    /// Builds the leg from the configured [`MakeLeg`] builder.
    ///
    /// # Errors
    /// Returns an error if required builder fields are missing or inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Leg> {
        let mut cashflows = Vec::new();
        let structure = self
            .structure
            .ok_or_else(|| AtlasError::ValueNotSetErr("Structure".into()))?;

        let payment_frequency = self
            .payment_frequency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Payment frequency".into()))?;

        let side = self
            .side
            .ok_or_else(|| AtlasError::ValueNotSetErr("Side".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| AtlasError::ValueNotSetErr("Currency".into()))?;

        let leg_id = self.leg_id.unwrap_or(0);
        match structure {
            PaymentStructure::Bullet => {
                let leg_type = self.check_leg_type()?;
                let start_date = self
                    .start_date
                    .ok_or_else(|| AtlasError::ValueNotSetErr("Start date".into()))?;
                let end_date = if let Some(date) = self.end_date {
                    date
                } else {
                    let tenor = self
                        .tenor
                        .ok_or_else(|| AtlasError::ValueNotSetErr("Tenor".into()))?;
                    start_date + tenor
                };

                let mut schedule_builder = MakeSchedule::new(start_date, end_date)
                    .with_frequency(payment_frequency)
                    .end_of_month(self.end_of_month.unwrap_or(false))
                    .with_calendar(
                        self.calendar
                            .unwrap_or(Calendar::NullCalendar(NullCalendar::new())),
                    )
                    .with_convention(
                        self.business_day_convention
                            .unwrap_or(BusinessDayConvention::Unadjusted),
                    )
                    .with_rule(
                        self.date_generation_rule
                            .unwrap_or(DateGenerationRule::Backward),
                    );

                let schedule = if let Some(date) = self.first_coupon_date {
                    if date > start_date {
                        schedule_builder.with_first_date(date).build()?
                    } else {
                        Err(AtlasError::InvalidValueErr(
                            "First coupon date must be after start date".into(),
                        ))?
                    }
                } else {
                    schedule_builder.build()?
                };

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;
                let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];
                let last_date = vec![*schedule
                    .dates()
                    .last()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];
                let notionals = notionals_vector(
                    schedule.dates().len() - 1,
                    notional,
                    PaymentStructure::Bullet,
                );

                add_cashflows_to_vec(&mut cashflows, &first_date, &[notional], 1);
                add_cashflows_to_vec(&mut cashflows, &last_date, &[notional], 0);

                match leg_type {
                    LegType::FixedRate => {
                        // create coupon cashflows
                        let rate = self
                            .rate
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Rate".into()))?;

                        build_fixed_rate_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            rate,
                        )?;
                        let market_index = self
                            .market_index
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            None,
                            self.rate,
                            side,
                            true,
                        );

                        Ok(leg)
                    }
                    LegType::FloatingRate => {
                        // create coupon cashflows
                        let spread = self
                            .spread
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Spread".into()))?;

                        let market_index = self
                            .market_index
                            .clone()
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;

                        build_floating_rate_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            ADReal::new(spread),
                            market_index.clone(),
                        )?;

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            Some(ADReal::new(spread)),
                            None,
                            side,
                            true,
                        );
                        Ok(leg)
                    }
                    LegType::OptionEmbedded => {
                        let spread = self
                            .spread
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Spread".into()))?;

                        let market_index = self
                            .market_index
                            .clone()
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;

                        let _ = self.payoff_ops.ok_or_else(|| {
                            AtlasError::ValueNotSetErr("Payoff operations".into())
                        })?;

                        build_embedded_option_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            ADReal::new(spread),
                            market_index.clone(),
                            self.floorlet_strike,
                            self.caplet_strike,
                        )?;

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            Some(ADReal::new(spread)),
                            None,
                            side,
                            true,
                        );
                        Ok(leg)
                    }
                }
            }
            PaymentStructure::Other => {
                let disbursements = self
                    .disbursements
                    .ok_or(AtlasError::ValueNotSetErr("Disbursements".into()))?;
                let redemptions = self
                    .redemptions
                    .ok_or(AtlasError::ValueNotSetErr("Redemptions".into()))?;
                let notional = disbursements.values().fold(0.0, |acc, x| acc + x).abs();
                let redemption = redemptions.values().fold(0.0, |acc, x| acc + x).abs();
                if (notional - redemption).abs() > 0.000001 {
                    return Err(AtlasError::InvalidValueErr(
                        "Notional and redemption must be equal".into(),
                    ));
                }

                // Add disbursements as CashflowType::Disbursement
                for (date, amount) in &disbursements {
                    cashflows.push(CashflowType::Disbursement(SimpleCashflow::new(
                        *amount, *date,
                    )));
                }

                // Add redemptions as CashflowType::Redemption
                for (date, amount) in &redemptions {
                    cashflows.push(CashflowType::Redemption(SimpleCashflow::new(
                        *amount, *date,
                    )));
                }

                let leg = Leg::new(
                    leg_id,
                    cashflows,
                    currency,
                    self.market_index.clone(),
                    None,
                    None,
                    side,
                    true,
                );

                Ok(leg)
            }
            PaymentStructure::EqualPayments => {
                let leg_type = self.check_leg_type()?;
                let start_date = self
                    .start_date
                    .ok_or(AtlasError::ValueNotSetErr("Start date".into()))?;
                let end_date = if let Some(date) = self.end_date {
                    date
                } else {
                    let tenor = self
                        .tenor
                        .ok_or(AtlasError::ValueNotSetErr("Tenor".into()))?;
                    start_date + tenor
                };
                let mut schedule_builder = MakeSchedule::new(start_date, end_date)
                    .with_frequency(payment_frequency)
                    .end_of_month(self.end_of_month.unwrap_or(false))
                    .with_calendar(
                        self.calendar
                            .unwrap_or(Calendar::NullCalendar(NullCalendar::new())),
                    )
                    .with_convention(
                        self.business_day_convention
                            .unwrap_or(BusinessDayConvention::Unadjusted),
                    )
                    .with_rule(
                        self.date_generation_rule
                            .unwrap_or(DateGenerationRule::Backward),
                    );

                let schedule = if let Some(date) = self.first_coupon_date {
                    if date > start_date {
                        schedule_builder.with_first_date(date).build()?
                    } else {
                        Err(AtlasError::InvalidValueErr(
                            "First coupon date must be after start date".into(),
                        ))?
                    }
                } else {
                    schedule_builder.build()?
                };

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];

                // Add initial disbursement
                add_cashflows_to_vec(&mut cashflows, &first_date, &[notional], 1);

                // Only FixedRate supports EqualPayments structure
                match leg_type {
                    LegType::FixedRate => {
                        let rate = self
                            .rate
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Rate".into()))?;

                        let redemptions =
                            calculate_equal_payment_redemptions(schedule.dates(), rate, notional)?;

                        let mut notionals =
                            redemptions.iter().try_fold(vec![notional], |mut acc, x| {
                                let last = *acc.last().ok_or(AtlasError::InvalidValueErr(
                                    "Notional schedule cannot be empty".into(),
                                ))?;
                                acc.push(last - x);
                                Ok::<_, AtlasError>(acc)
                            })?;

                        notionals.pop();

                        build_fixed_rate_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            rate,
                        )?;

                        let redemption_dates: Vec<Date> =
                            schedule.dates().iter().skip(1).copied().collect();
                        add_cashflows_to_vec(&mut cashflows, &redemption_dates, &redemptions, 0);

                        let market_index = self
                            .market_index
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            None,
                            self.rate,
                            side,
                            true,
                        );

                        Ok(leg)
                    }
                    LegType::FloatingRate | LegType::OptionEmbedded => {
                        Err(AtlasError::InvalidValueErr(
                            "EqualPayments structure is only supported for fixed rate legs".into(),
                        ))
                    }
                }
            }
            PaymentStructure::Zero => {
                let start_date = self
                    .start_date
                    .ok_or(AtlasError::ValueNotSetErr("Start date".into()))?;
                let end_date = if let Some(date) = self.end_date {
                    date
                } else {
                    let tenor = self
                        .tenor
                        .ok_or(AtlasError::ValueNotSetErr("Tenor".into()))?;
                    start_date + tenor
                };
                let schedule = MakeSchedule::new(start_date, end_date)
                    .with_frequency(payment_frequency)
                    .with_convention(
                        self.business_day_convention
                            .unwrap_or(BusinessDayConvention::Unadjusted),
                    )
                    .with_calendar(
                        self.calendar
                            .unwrap_or(Calendar::NullCalendar(NullCalendar::new())),
                    )
                    .with_rule(
                        self.date_generation_rule
                            .unwrap_or(DateGenerationRule::Backward),
                    )
                    .build()?;

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];
                let last_date = vec![*schedule
                    .dates()
                    .last()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];

                // Add initial disbursement
                add_cashflows_to_vec(&mut cashflows, &first_date, &[notional], 1);
                // Add final redemption
                add_cashflows_to_vec(&mut cashflows, &last_date, &[notional], 0);

                let leg = Leg::new(
                    leg_id,
                    cashflows,
                    currency,
                    self.market_index.clone(),
                    None,
                    None,
                    side,
                    true,
                );

                Ok(leg)
            }
            PaymentStructure::EqualRedemptions => {
                let leg_type = self.check_leg_type()?;
                let start_date = self
                    .start_date
                    .ok_or(AtlasError::ValueNotSetErr("Start date".into()))?;
                let end_date = if let Some(date) = self.end_date {
                    date
                } else {
                    let tenor = self
                        .tenor
                        .ok_or(AtlasError::ValueNotSetErr("Tenor".into()))?;
                    start_date + tenor
                };
                let mut schedule_builder = MakeSchedule::new(start_date, end_date)
                    .with_frequency(payment_frequency)
                    .end_of_month(self.end_of_month.unwrap_or(false))
                    .with_convention(
                        self.business_day_convention
                            .unwrap_or(BusinessDayConvention::Unadjusted),
                    )
                    .with_calendar(
                        self.calendar
                            .unwrap_or(Calendar::NullCalendar(NullCalendar::new())),
                    )
                    .with_rule(
                        self.date_generation_rule
                            .unwrap_or(DateGenerationRule::Backward),
                    );

                let schedule = if let Some(date) = self.first_coupon_date {
                    if date > start_date {
                        schedule_builder.with_first_date(date).build()?
                    } else {
                        Err(AtlasError::InvalidValueErr(
                            "First coupon date must be after start date".into(),
                        ))?
                    }
                } else {
                    schedule_builder.build()?
                };

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];

                let n = schedule.dates().len() - 1;
                let notionals = notionals_vector(n, notional, PaymentStructure::EqualRedemptions);
                let n_f64 = f64::from(u32::try_from(n).map_err(|_| {
                    AtlasError::InvalidValueErr("Redemption count exceeds u32".into())
                })?);
                let redemptions = vec![notional / n_f64; n];

                // Add initial disbursement
                add_cashflows_to_vec(&mut cashflows, &first_date, &[notional], 1);

                match leg_type {
                    LegType::FixedRate => {
                        let rate = self
                            .rate
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Rate".into()))?;

                        build_fixed_rate_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            rate,
                        )?;

                        let redemption_dates: Vec<Date> =
                            schedule.dates().iter().skip(1).copied().collect();
                        add_cashflows_to_vec(&mut cashflows, &redemption_dates, &redemptions, 0);

                        let market_index = self
                            .market_index
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            None,
                            self.rate,
                            side,
                            true,
                        );

                        Ok(leg)
                    }
                    LegType::FloatingRate => {
                        let spread = self
                            .spread
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Spread".into()))?;

                        let market_index = self
                            .market_index
                            .clone()
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?;

                        build_floating_rate_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            ADReal::new(spread),
                            market_index.clone(),
                        )?;

                        let redemption_dates: Vec<Date> =
                            schedule.dates().iter().skip(1).copied().collect();
                        add_cashflows_to_vec(&mut cashflows, &redemption_dates, &redemptions, 0);

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            Some(ADReal::new(spread)),
                            None,
                            side,
                            true,
                        );
                        Ok(leg)
                    }
                    LegType::OptionEmbedded => {
                        let spread = self
                            .spread
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Spread".into()))?;

                        let market_index = self
                            .market_index
                            .as_ref()
                            .ok_or_else(|| AtlasError::ValueNotSetErr("Market index".into()))?
                            .clone();

                        build_embedded_option_coupons_from_notionals(
                            &mut cashflows,
                            schedule.dates(),
                            &notionals,
                            ADReal::new(spread),
                            market_index.clone(),
                            self.floorlet_strike,
                            self.caplet_strike,
                        )?;

                        let redemption_dates: Vec<Date> =
                            schedule.dates().iter().skip(1).copied().collect();
                        add_cashflows_to_vec(&mut cashflows, &redemption_dates, &redemptions, 0);

                        let leg = Leg::new(
                            leg_id,
                            cashflows,
                            currency,
                            Some(market_index),
                            Some(ADReal::new(spread)),
                            None,
                            side,
                            true,
                        );
                        Ok(leg)
                    }
                }
            }
        }
    }
}

/// Helper function to build fixed rate coupons from a vector of notionals and schedule dates.
fn build_fixed_rate_coupons_from_notionals(
    cashflows: &mut Vec<CashflowType>,
    dates: &[Date],
    notionals: &[f64],
    rate: InterestRate<ADReal>,
) -> Result<()> {
    if dates.len() - 1 != notionals.len() {
        Err(AtlasError::InvalidValueErr(
            "Dates and notionals must have the same length".to_string(),
        ))?;
    }
    if dates.len() < 2 {
        Err(AtlasError::InvalidValueErr(
            "Dates must have at least two elements".to_string(),
        ))?;
    }
    for (date_pair, notional) in dates.windows(2).zip(notionals) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let coupon = FixedRateCoupon::new(*notional, Box::new(rate), d1, d2, d2);
        cashflows.push(CashflowType::FixedRateCoupon(coupon));
    }
    Ok(())
}

/// Helper function to build floating rate coupons from a vector of notionals and schedule dates.
fn build_floating_rate_coupons_from_notionals(
    cashflows: &mut Vec<CashflowType>,
    dates: &[Date],
    notionals: &[f64],
    spread: ADReal,
    market_index: MarketIndex,
) -> Result<()> {
    if dates.len() - 1 != notionals.len() {
        Err(AtlasError::InvalidValueErr(
            "Dates and notionals must have the same length".to_string(),
        ))?;
    }
    if dates.len() < 2 {
        Err(AtlasError::InvalidValueErr(
            "Dates must have at least two elements".to_string(),
        ))?;
    }
    for (date_pair, notional) in dates.windows(2).zip(notionals) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let coupon = FloatingRateCoupon::new(*notional, spread, market_index.clone(), d1, d2, d2);
        cashflows.push(CashflowType::FloatingRateCoupon(coupon));
    }
    Ok(())
}

fn build_embedded_option_coupons_from_notionals(
    cashflows: &mut Vec<CashflowType>,
    dates: &[Date],
    notionals: &[f64],
    spread: ADReal,
    market_index: MarketIndex,
    floorlet_strike: Option<f64>,
    caplet_strike: Option<f64>,
) -> Result<()> {
    if dates.len() - 1 != notionals.len() {
        Err(AtlasError::InvalidValueErr(
            "Dates and notionals must have the same length".to_string(),
        ))?;
    }
    if dates.len() < 2 {
        Err(AtlasError::InvalidValueErr(
            "Dates must have at least two elements".to_string(),
        ))?;
    }

    if floorlet_strike.is_none() && caplet_strike.is_none() {
        Err(AtlasError::InvalidValueErr(
            "At least one of floorlet or caplet strike must be set for option-embedded coupons"
                .to_string(),
        ))?;
    }

    let payoff = match (floorlet_strike, caplet_strike) {
        (Some(floor), Some(cap)) => PayoffOps::Max(
            Box::new(PayoffOps::Min(
                Box::new(PayoffOps::Index),
                Box::new(PayoffOps::Const(floor)),
            )),
            Box::new(PayoffOps::Const(cap)),
        ),
        (Some(floor), None) => PayoffOps::Min(
            Box::new(PayoffOps::Index),
            Box::new(PayoffOps::Const(floor)),
        ),
        (None, Some(cap)) => {
            PayoffOps::Max(Box::new(PayoffOps::Index), Box::new(PayoffOps::Const(cap)))
        }
        (None, None) => unreachable!(), // This case is already handled above
    };

    for (date_pair, notional) in dates.windows(2).zip(notionals) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let coupon = OptionEmbeddedCoupon::new(
            *notional,
            market_index.clone(),
            spread,
            d1,
            d2,
            d2,
            payoff.clone(),
        );
        cashflows.push(CashflowType::OptionEmbeddedCoupon(coupon));
    }
    Ok(())
}

/// Helper function to add cashflows to a vector based on dates, amounts and cashflow type (0 for redemption, 1 for disbursement).
fn add_cashflows_to_vec(
    cashflows: &mut Vec<CashflowType>,
    dates: &[Date],
    amounts: &[f64],
    cashflow_type: usize,
) {
    for (date, amount) in dates.iter().zip(amounts) {
        let cashflow = SimpleCashflow::new(*amount, *date);
        match cashflow_type {
            0 => cashflows.push(CashflowType::Redemption(cashflow)),
            1 => cashflows.push(CashflowType::Disbursement(cashflow)),
            _ => (),
        }
    }
}

/// Generates a vector of notionals based on the payment structure.
fn notionals_vector(n: usize, notional: f64, structure: PaymentStructure) -> Vec<f64> {
    match structure {
        PaymentStructure::Bullet => vec![notional; n],
        PaymentStructure::EqualRedemptions => {
            let redemptions = vec![
                notional
                    / f64::from(u32::try_from(n).unwrap_or_else(|_| {
                        panic!("notional schedule length should fit in u32")
                    }));
                n
            ];
            let mut results = Vec::new();
            let mut sum = 0.0;
            for r in redemptions {
                results.push(notional - sum);
                sum += r;
            }
            results
        }
        PaymentStructure::Zero => vec![notional; 1],
        _ => vec![],
    }
}

/// Closed-form solution for constant amortizing payment
/// Payment = Notional / Annuity Factor where Annuity Factor = sum(1 / compound_factor_i) for each period
fn calculate_equal_payment_redemptions(
    dates: &[Date],
    rate: InterestRate<ADReal>,
    notional: f64,
) -> Result<Vec<f64>> {
    let mut annuity_factor = 0.0;
    for date_pair in dates.windows(2) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let cf = rate.compound_factor(d1, d2);
        let cf_f64: f64 = cf.value();
        annuity_factor += 1.0 / cf_f64;
    }

    // Constant payment amount
    let payment = notional / annuity_factor;

    // Calculate principal redemptions for each period
    let mut redemptions = Vec::new();
    let mut balance = notional;

    for date_pair in dates.windows(2) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let cf = rate.compound_factor(d1, d2);
        let cf_f64: f64 = cf.value();
        let interest = balance * (cf_f64 - 1.0);
        let principal = payment - interest;
        balance -= principal;
        redemptions.push(principal);
    }

    Ok(redemptions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rates::compounding::Compounding;
    use crate::rates::interestrate::RateDefinition;
    use crate::time::daycounter::DayCounter;

    fn create_test_leg_builder() -> MakeLeg {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2025, 1, 1);
        let rate = InterestRate::from_rate_definition(
            ADReal::new(0.05),
            RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ),
        );

        MakeLeg::default()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_notional(100_000.0)
            .with_rate(rate)
            .with_rate_type(RateType::Fixed)
            .with_side(Side::PayShort)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Annual)
            .with_market_index(MarketIndex::SOFR)
    }

    #[test]
    fn test_bullet_fixed_rate_leg() {
        let leg_builder = create_test_leg_builder().bullet();

        let result = leg_builder.build();
        if let Err(e) = &result {
            panic!("Failed to build bullet leg: {}", e);
        }

        let leg = result.unwrap();
        assert!(!leg.cashflows().is_empty(), "Leg should have cashflows");
    }

    #[test]
    fn test_zero_coupon_leg() {
        let leg_builder = create_test_leg_builder().zero();

        let result = leg_builder.build();
        assert!(result.is_ok(), "Failed to build zero coupon leg");

        let leg = result.unwrap();
        assert_eq!(
            leg.cashflows().len(),
            2,
            "Zero coupon leg should have 2 cashflows (initial and final)"
        );
    }

    #[test]
    fn test_equal_redemptions_fixed_rate_leg() {
        let leg_builder = create_test_leg_builder()
            .equal_redemptions()
            .with_payment_frequency(Frequency::Semiannual);

        let result = leg_builder.build();
        if let Err(e) = &result {
            panic!("Failed to build equal redemptions leg: {}", e);
        }

        let leg = result.unwrap();
        assert!(!leg.cashflows().is_empty(), "Leg should have cashflows");
    }

    #[test]
    fn test_equal_payments_fixed_rate_leg() {
        let leg_builder = create_test_leg_builder()
            .equal_payments()
            .with_payment_frequency(Frequency::Annual);

        let result = leg_builder.build();
        if let Err(e) = &result {
            panic!("Failed to build equal payments leg: {}", e);
        }

        let leg = result.unwrap();
        assert!(!leg.cashflows().is_empty(), "Leg should have cashflows");
    }

    #[test]
    fn test_missing_notional_error() {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2025, 1, 1);

        let leg_builder = MakeLeg::default()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .bullet()
            .with_rate_type(RateType::Fixed)
            .with_side(Side::PayShort)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Annual);

        let result = leg_builder.build();
        assert!(result.is_err(), "Should fail when notional is not set");
    }

    #[test]
    fn test_missing_rate_error_for_fixed_rate() {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2025, 1, 1);

        let leg_builder = MakeLeg::default()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_notional(100_000.0)
            .bullet()
            .with_rate_type(RateType::Fixed)
            .with_side(Side::PayShort)
            .with_currency(Currency::USD)
            .with_payment_frequency(Frequency::Annual);

        let result = leg_builder.build();
        assert!(
            result.is_err(),
            "Should fail when rate is not set for fixed rate leg"
        );
    }

    #[test]
    fn test_other_structure_with_disbursements_and_redemptions() {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2025, 1, 1);

        let mut disbursements = HashMap::new();
        disbursements.insert(start_date, 100_000.0);

        let mut redemptions = HashMap::new();
        redemptions.insert(end_date, 100_000.0);

        let leg_builder = MakeLeg::default()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_notional(100_000.0)
            .other()
            .with_side(Side::PayShort)
            .with_currency(Currency::USD)
            .with_disbursements(disbursements)
            .with_redemptions(redemptions);

        let result = leg_builder.build();
        if let Err(e) = &result {
            panic!("Failed to build other structure leg: {}", e);
        }

        let leg = result.unwrap();
        assert!(!leg.cashflows().is_empty(), "Leg should have cashflows");
    }

    #[test]
    fn test_other_structure_unequal_notional_error() {
        let start_date = Date::new(2024, 1, 1);
        let end_date = Date::new(2025, 1, 1);

        let mut disbursements = HashMap::new();
        disbursements.insert(start_date, 100_000.0);

        let mut redemptions = HashMap::new();
        redemptions.insert(end_date, 95_000.0); // Unequal amount

        let leg_builder = MakeLeg::default()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .other()
            .with_side(Side::PayShort)
            .with_currency(Currency::USD)
            .with_disbursements(disbursements)
            .with_redemptions(redemptions);

        let result = leg_builder.build();
        assert!(
            result.is_err(),
            "Should fail when disbursements and redemptions are unequal"
        );
    }
}
