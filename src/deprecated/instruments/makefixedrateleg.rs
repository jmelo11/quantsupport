use argmin::{
    core::{CostFunction, Error, Executor},
    solver::brent::BrentRoot,
};

use std::collections::{HashMap, HashSet};

use crate::{
    cashflows::{
        cashflow::{Cashflow, CashflowType, Side},
        fixedratecoupon::FixedRateCoupon,
        simplecashflow::SimpleCashflow,
    },
    currencies::enums::Currency,
    rates::interestrate::{InterestRate, RateDefinition},
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

use super::{
    instrument::RateType,
    leg::Leg,
    traits::{add_cashflows_to_vec, calculate_outstanding, notionals_vector, Structure},
};

/// # `MakeFixedRateLeg`
/// `MakeFixedRateLeg` is a builder for a fixed rate leg. Uses the builder pattern.
// TODO: Handle negative amounts (redemptions, notionals and disbursements)
#[derive(Debug, Clone)]
pub struct MakeFixedRateLeg {
    start_date: Option<Date>,
    end_date: Option<Date>,
    first_coupon_date: Option<Date>,
    payment_frequency: Option<Frequency>,
    tenor: Option<Period>,
    currency: Option<Currency>,
    side: Option<Side>,
    notional: Option<f64>,
    structure: Option<Structure>,
    rate: Option<InterestRate>,
    discount_curve_id: Option<usize>,
    disbursements: Option<HashMap<Date, f64>>,
    redemptions: Option<HashMap<Date, f64>>,
    end_of_month: Option<bool>,
    additional_coupon_dates: Option<HashSet<Date>>,
    rate_definition: Option<RateDefinition>,
    rate_value: Option<f64>,
    issue_date: Option<Date>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    yield_rate: Option<InterestRate>,
}

/// New, setters and getters
impl MakeFixedRateLeg {
    /// Creates a new `MakeFixedRateLeg` builder with default values.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_date: None,
            end_date: None,
            first_coupon_date: None,
            payment_frequency: None,
            tenor: None,
            rate: None,
            notional: None,
            side: None,
            currency: None,
            structure: None,
            end_of_month: None,
            discount_curve_id: None,
            disbursements: None,
            redemptions: None,
            additional_coupon_dates: None,
            rate_definition: None,
            rate_value: None,
            issue_date: None,
            yield_rate: None,
            business_day_convention: None,
            date_generation_rule: None,
            calendar: None,
        }
    }

    /// Sets the end of month flag.
    #[must_use]
    pub const fn with_end_of_month(mut self, end_of_month: Option<bool>) -> Self {
        self.end_of_month = end_of_month;
        self
    }

    /// Sets the issue date.
    #[must_use]
    pub const fn with_issue_date(mut self, issue_date: Date) -> Self {
        self.issue_date = Some(issue_date);
        self
    }

    /// Sets the first coupon date.
    #[must_use]
    pub const fn with_first_coupon_date(mut self, first_coupon_date: Option<Date>) -> Self {
        self.first_coupon_date = first_coupon_date;
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
    ///
    /// ### Details
    /// Currently does not handle negative amounts.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the yield rate.
    #[must_use]
    pub const fn with_yield_rate(mut self, yield_rate: InterestRate) -> Self {
        self.yield_rate = Some(yield_rate);
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

    /// Sets the rate definition.
    #[must_use]
    pub const fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        match self.rate_value {
            Some(rate_value) => {
                self.rate = Some(InterestRate::new(
                    rate_value,
                    rate_definition.compounding(),
                    rate_definition.frequency(),
                    rate_definition.day_counter(),
                ));
            }
            None => {
                if let Some(rate) = self.rate {
                    self.rate = Some(InterestRate::new(
                        rate.rate(),
                        rate_definition.compounding(),
                        rate_definition.frequency(),
                        rate_definition.day_counter(),
                    ));
                }
            }
        }
        self
    }

    /// Sets the rate value.
    #[must_use]
    pub const fn with_rate_value(mut self, rate_value: f64) -> Self {
        self.rate_value = Some(rate_value);
        match self.rate {
            Some(rate) => {
                self.rate = Some(InterestRate::new(
                    rate_value,
                    rate.compounding(),
                    rate.frequency(),
                    rate.day_counter(),
                ));
            }
            None => {
                if let Some(rate_definition) = self.rate_definition {
                    self.rate = Some(InterestRate::new(
                        rate_value,
                        rate_definition.compounding(),
                        rate_definition.frequency(),
                        rate_definition.day_counter(),
                    ));
                }
            }
        }
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

    /// Sets the additional coupon dates.
    #[must_use]
    pub fn with_additional_coupon_dates(mut self, additional_coupon_dates: HashSet<Date>) -> Self {
        self.additional_coupon_dates = Some(additional_coupon_dates);
        self
    }

    /// Sets the rate.
    #[must_use]
    pub const fn with_rate(mut self, rate: InterestRate) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the discount curve id.
    #[must_use]
    pub const fn with_discount_curve_id(mut self, id: Option<usize>) -> Self {
        self.discount_curve_id = id;
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

    /// Sets the structure to bullet.
    #[must_use]
    pub const fn bullet(mut self) -> Self {
        self.structure = Some(Structure::Bullet);
        self
    }

    /// Sets the structure to equal redemptions.
    #[must_use]
    pub const fn equal_redemptions(mut self) -> Self {
        self.structure = Some(Structure::EqualRedemptions);
        self
    }

    /// Sets the structure to zero.
    #[must_use]
    pub const fn zero(mut self) -> Self {
        self.structure = Some(Structure::Zero);
        self.payment_frequency = Some(Frequency::Once);
        self
    }

    /// Sets the structure to equal payments.
    #[must_use]
    pub const fn equal_payments(mut self) -> Self {
        self.structure = Some(Structure::EqualPayments);
        self
    }

    /// Sets the structure to other.
    #[must_use]
    pub const fn other(mut self) -> Self {
        self.structure = Some(Structure::Other);
        self.payment_frequency = Some(Frequency::OtherFrequency);
        self
    }

    /// Sets the structure.
    #[must_use]
    pub const fn with_structure(mut self, structure: Structure) -> Self {
        self.structure = Some(structure);
        self
    }
}

impl Default for MakeFixedRateLeg {
    fn default() -> Self {
        Self::new()
    }
}

impl MakeFixedRateLeg {
    /// Builds the leg from the configured `MakeFixedRateLeg` builder.
    ///
    /// # Errors
    /// Returns an error if required builder fields are missing or inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Leg> {
        let mut cashflows = Vec::new();
        let structure = self
            .structure
            .ok_or(AtlasError::ValueNotSetErr("Structure".into()))?;
        let rate = self.rate.ok_or(AtlasError::ValueNotSetErr("Rate".into()))?;
        let payment_frequency = self
            .payment_frequency
            .ok_or(AtlasError::ValueNotSetErr("Payment frequency".into()))?;

        let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;
        let currency = self
            .currency
            .ok_or(AtlasError::ValueNotSetErr("Currency".into()))?;

        match structure {
            Structure::Bullet => {
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

                // this logic should go into a separate function/ Schedule should have accessing methods
                // to first and last date and other attributes
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
                let notionals =
                    notionals_vector(schedule.dates().len() - 1, notional, Structure::Bullet);

                add_cashflows_to_vec(
                    &mut cashflows,
                    &first_date,
                    &[notional],
                    side.inverse(),
                    currency,
                    CashflowType::Disbursement,
                );
                build_coupons_from_notionals(
                    &mut cashflows,
                    schedule.dates(),
                    &notionals,
                    rate,
                    side,
                    currency,
                )?;
                add_cashflows_to_vec(
                    &mut cashflows,
                    &last_date,
                    &[notional],
                    side,
                    currency,
                    CashflowType::Redemption,
                );

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                let leg = Leg::new(
                    structure,
                    RateType::Fixed,
                    rate.rate(),
                    rate.rate_definition(),
                    currency,
                    side,
                    self.discount_curve_id,
                    None,
                    cashflows,
                );

                Ok(leg)
            }
            Structure::Other => {
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

                let additional_dates = self.additional_coupon_dates.unwrap_or_default();

                let timeline =
                    calculate_outstanding(&disbursements, &redemptions, &additional_dates);

                for (date, amount) in &disbursements {
                    let cashflow = Cashflow::Disbursement(
                        SimpleCashflow::new(*date, currency, side.inverse()).with_amount(*amount),
                    );
                    cashflows.push(cashflow);
                }
                for (start_date, end_date, notional) in &timeline {
                    let coupon = FixedRateCoupon::new(
                        *notional,
                        rate,
                        *start_date,
                        *end_date,
                        *end_date,
                        currency,
                        side,
                    );
                    cashflows.push(Cashflow::FixedRateCoupon(coupon));
                }
                for (date, amount) in &redemptions {
                    let cashflow = Cashflow::Redemption(
                        SimpleCashflow::new(*date, currency, side).with_amount(*amount),
                    );
                    cashflows.push(cashflow);
                }

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                Ok(Leg::new(
                    structure,
                    RateType::Fixed,
                    rate.rate(),
                    rate.rate_definition(),
                    currency,
                    side,
                    self.discount_curve_id,
                    None,
                    cashflows,
                ))
            }
            Structure::EqualPayments => {
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

                let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;

                let redemptions = calculate_equal_payment_redemptions(
                    schedule.dates(),
                    rate,
                    notional,
                    side,
                )?;

                let mut notionals =
                    redemptions
                        .iter()
                        .try_fold(vec![notional], |mut acc, x| {
                            let last = *acc.last().ok_or(AtlasError::InvalidValueErr(
                                "Notional schedule cannot be empty".into(),
                            ))?;
                            acc.push(last - x);
                            Ok::<_, AtlasError>(acc)
                        })?;

                notionals.pop();

                // create coupon cashflows
                build_coupons_from_notionals(
                    &mut cashflows,
                    schedule.dates(),
                    &notionals,
                    rate,
                    side,
                    currency,
                )?;

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];
                add_cashflows_to_vec(
                    &mut cashflows,
                    &first_date,
                    &[notional],
                    side.inverse(),
                    currency,
                    CashflowType::Disbursement,
                );

                let redemption_dates: Vec<Date> =
                    schedule.dates().iter().skip(1).copied().collect();
                add_cashflows_to_vec(
                    &mut cashflows,
                    &redemption_dates,
                    &redemptions,
                    side,
                    currency,
                    CashflowType::Redemption,
                );

                //let infered_cashflows = infer_cashflows_from_amounts(dates, amounts, side, currency);
                //cashflows.extend(infered_cashflows);

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                Ok(Leg::new(
                    structure,
                    RateType::Fixed,
                    rate.rate(),
                    rate.rate_definition(),
                    currency,
                    side,
                    self.discount_curve_id,
                    None,
                    cashflows,
                ))
            }
            Structure::Zero => {
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
                let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;

                let notionals =
                    notionals_vector(schedule.dates().len() - 1, notional, Structure::Bullet);

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];
                let last_date = vec![*schedule
                    .dates()
                    .last()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];

                add_cashflows_to_vec(
                    &mut cashflows,
                    &first_date,
                    &[notional],
                    side.inverse(),
                    currency,
                    CashflowType::Disbursement,
                );
                build_coupons_from_notionals(
                    &mut cashflows,
                    schedule.dates(),
                    &notionals,
                    rate,
                    side,
                    currency,
                )?;
                add_cashflows_to_vec(
                    &mut cashflows,
                    &last_date,
                    &[notional],
                    side,
                    currency,
                    CashflowType::Redemption,
                );

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                Ok(Leg::new(
                    structure,
                    RateType::Fixed,
                    rate.rate(),
                    rate.rate_definition(),
                    currency,
                    side,
                    self.discount_curve_id,
                    None,
                    cashflows,
                ))
            }
            Structure::EqualRedemptions => {
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
                let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;

                let first_date = vec![*schedule
                    .dates()
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Schedule dates".into()))?];

                let n = schedule.dates().len() - 1;
                let notionals = notionals_vector(n, notional, Structure::EqualRedemptions);
                let n_f64 = f64::from(u32::try_from(n).map_err(|_| {
                    AtlasError::InvalidValueErr("Redemption count exceeds u32".into())
                })?);
                let redemptions = vec![notional / n_f64; n];

                add_cashflows_to_vec(
                    &mut cashflows,
                    &first_date,
                    &[notional],
                    side.inverse(),
                    currency,
                    CashflowType::Disbursement,
                );

                build_coupons_from_notionals(
                    &mut cashflows,
                    schedule.dates(),
                    &notionals,
                    rate,
                    side,
                    currency,
                )?;

                let redemption_dates: Vec<Date> =
                    schedule.dates().iter().skip(1).copied().collect();

                add_cashflows_to_vec(
                    &mut cashflows,
                    &redemption_dates,
                    &redemptions,
                    side,
                    currency,
                    CashflowType::Redemption,
                );

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                Ok(Leg::new(
                    structure,
                    RateType::Fixed,
                    rate.rate(),
                    rate.rate_definition(),
                    currency,
                    side,
                    self.discount_curve_id,
                    None,
                    cashflows,
                ))
            }
        }
    }
}

fn build_coupons_from_notionals(
    cashflows: &mut Vec<Cashflow>,
    dates: &[Date],
    notionals: &[f64],
    rate: InterestRate,
    side: Side,
    currency: Currency,
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
        let coupon = FixedRateCoupon::new(*notional, rate, d1, d2, d2, currency, side);
        cashflows.push(Cashflow::FixedRateCoupon(coupon));
    }
    Ok(())
}

struct EqualPaymentCost {
    dates: Vec<Date>,
    rate: InterestRate,
}

impl CostFunction for EqualPaymentCost {
    type Param = f64;
    type Output = f64;
    fn cost(&self, payment: &Self::Param) -> std::result::Result<Self::Output, Error> {
        let mut total_amount = 1.0;
        for date_pair in self.dates.windows(2) {
            let d1 = date_pair[0];
            let d2 = date_pair[1];
            let interest = total_amount * (self.rate.compound_factor(d1, d2) - 1.0);
            total_amount -= payment - interest;
        }
        Ok(total_amount)
    }
}

fn calculate_equal_payment_redemptions(
    dates: &[Date],
    rate: InterestRate,
    notional: f64,
    side: Side,
) -> Result<Vec<f64>> {
    let cost = EqualPaymentCost {
        dates: dates.to_vec(),
        rate,
    };
    let (min, max) = (-0.1, 1.5);
    let solver = BrentRoot::new(min, max, 1e-6);

    let len = u32::try_from(dates.len()).map_err(|_| {
        AtlasError::InvalidValueErr("Dates length should fit in u32".to_string())
    })?;
    let init_param = 1.0 / f64::from(len);
    let res = Executor::new(cost, solver)
        .configure(|state| state.param(init_param).max_iters(100).target_cost(0.0))
        .run()?;

    let payment = res
        .state()
        .best_param
        .ok_or(AtlasError::EvaluationErr("Solver failed".into()))?
        * notional;

    let mut redemptions = Vec::new();
    let mut total_amount = notional;
    let flag = side.sign();
    for date_pair in dates.windows(2) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let interest = total_amount * (rate.compound_factor(d1, d2) - 1.0);
        let k = payment - interest;
        total_amount -= k;
        redemptions.push(k * flag);
    }
    Ok(redemptions)
}
