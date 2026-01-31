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
        traits::{InterestAccrual, Payable},
    },
    core::traits::HasCurrency,
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
    visitors::traits::HasCashflows,
};

use super::{
    fixedrateinstrument::FixedRateInstrument,
    traits::{add_cashflows_to_vec, calculate_outstanding, notionals_vector, Structure},
};

/// # `MakeFixedRateInstrument`
/// `MakeFixedRateInstrument` is a builder for `FixedRateInstrument`. Uses the builder pattern.
// TODO: Handle negative amounts (redemptions, notionals and disbursements)
#[derive(Debug, Clone)]
pub struct MakeFixedRateInstrument {
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
    additional_coupon_dates: Option<HashSet<Date>>,
    rate_definition: Option<RateDefinition>,
    rate_value: Option<f64>,
    issue_date: Option<Date>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    yield_rate: Option<InterestRate>,
    id: Option<String>,
}

/// New, setters and getters
impl MakeFixedRateInstrument {
    /// Creates a new `MakeFixedRateInstrument` builder with default values.
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
            discount_curve_id: None,
            disbursements: None,
            redemptions: None,
            additional_coupon_dates: None,
            rate_definition: None,
            rate_value: None,
            id: None,
            issue_date: None,
            yield_rate: None,
            business_day_convention: None,
            date_generation_rule: None,
            calendar: None,
        }
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

    /// Sets the instrument id.
    #[must_use]
    pub fn with_id(mut self, id: Option<String>) -> Self {
        self.id = id;
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

impl Default for MakeFixedRateInstrument {
    fn default() -> Self {
        Self::new()
    }
}

impl MakeFixedRateInstrument {
    /// Builds and returns a `FixedRateInstrument` from the configured builder.
    ///
    /// # Errors
    /// Returns an error if required builder fields are missing or inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<FixedRateInstrument> {
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

                Ok(FixedRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    rate,
                    payment_frequency,
                    cashflows,
                    structure,
                    side,
                    currency,
                    self.discount_curve_id,
                    self.id,
                    self.issue_date,
                    self.yield_rate,
                ))
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
                let start_date = &timeline
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Start date".into()))?
                    .0;
                let end_date = &timeline
                    .last()
                    .ok_or(AtlasError::ValueNotSetErr("End date".into()))?
                    .1;

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                Ok(FixedRateInstrument::new(
                    *start_date,
                    *end_date,
                    notional,
                    rate,
                    payment_frequency,
                    cashflows,
                    structure,
                    side,
                    currency,
                    self.discount_curve_id,
                    self.id,
                    self.issue_date,
                    self.yield_rate,
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

                let mut dates = vec![];
                if let Some(redemption) = self.redemptions {
                        // disbursements should have only one element
                        let disbursements_dates = self
                            .disbursements
                            .ok_or(AtlasError::ValueNotSetErr("Disbursements".into()))?
                            .keys()
                            .copied()
                            .collect::<Vec<Date>>();

                        let redemption_dates = redemption.keys().copied().collect::<Vec<Date>>();
                        // dates equal to disbursements dates an them redemption dates
                        dates.extend(disbursements_dates);
                        dates.extend(redemption_dates);
                        dates.sort();
                } else {
                        let mut schedule_builder = MakeSchedule::new(start_date, end_date)
                            .with_frequency(payment_frequency)
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
                        dates.clone_from(schedule.dates());
                }

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                let redemptions_raw: Vec<f64> =
                    calculate_equal_payment_redemptions(&dates, rate, notional)?;

                let mut notionals =
                    redemptions_raw
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
                let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;
                build_coupons_from_notionals(
                    &mut cashflows,
                    &dates,
                    &notionals,
                    rate,
                    side,
                    currency,
                )?;

                let first_date = vec![*dates
                    .first()
                    .ok_or(AtlasError::ValueNotSetErr("Dates".into()))?];
                add_cashflows_to_vec(
                    &mut cashflows,
                    &first_date,
                    &[notional],
                    side.inverse(),
                    currency,
                    CashflowType::Disbursement,
                );

                let mut redemption_dates = vec![];
                let mut disbursement_dates = vec![];
                let mut redemptions = vec![];
                let mut disbursements = vec![];

                let aux_dates: Vec<Date> = dates.iter().skip(1).copied().collect();
                for (date, amount) in aux_dates.iter().zip(redemptions_raw.iter()) {
                    if *amount >= 0.0 {
                        redemption_dates.push(*date);
                        redemptions.push(*amount);
                    } else {
                        disbursement_dates.push(*date);
                        disbursements.push(-*amount);
                    }
                }

                add_cashflows_to_vec(
                    &mut cashflows,
                    &redemption_dates,
                    &redemptions,
                    side,
                    currency,
                    CashflowType::Redemption,
                );

                if !disbursements.is_empty() {
                    add_cashflows_to_vec(
                        &mut cashflows,
                        &disbursement_dates,
                        &disbursements,
                        side.inverse(),
                        currency,
                        CashflowType::Disbursement,
                    );
                }

                //let infered_cashflows = infer_cashflows_from_amounts(dates, amounts, side, currency);
                //cashflows.extend(infered_cashflows);

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }

                Ok(FixedRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    rate,
                    payment_frequency,
                    cashflows,
                    structure,
                    side,
                    currency,
                    self.discount_curve_id,
                    self.id,
                    self.issue_date,
                    self.yield_rate,
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

                Ok(FixedRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    rate,
                    payment_frequency,
                    cashflows,
                    structure,
                    side,
                    currency,
                    self.discount_curve_id,
                    self.id,
                    self.issue_date,
                    self.yield_rate,
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

                Ok(FixedRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    rate,
                    payment_frequency,
                    cashflows,
                    structure,
                    side,
                    currency,
                    self.discount_curve_id,
                    self.id,
                    self.issue_date,
                    self.yield_rate,
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

//  function to calculate equal payment redemptions, always returns a vector of positive values
fn calculate_equal_payment_redemptions(
    dates: &[Date],
    rate: InterestRate,
    notional: f64,
) -> Result<Vec<f64>> {
    let cost = EqualPaymentCost {
        dates: dates.to_vec(),
        rate,
    };
    let (min, max) = (-0.2, 1.5);
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

    for date_pair in dates.windows(2) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let interest = total_amount * (rate.compound_factor(d1, d2) - 1.0);
        let k = payment - interest;
        total_amount -= k;
        redemptions.push(k);
    }
    Ok(redemptions)
}

/// Implementations for `FixedRateInstrument`
impl From<FixedRateInstrument> for MakeFixedRateInstrument {
    fn from(val: FixedRateInstrument) -> Self {
        let mut disbursements = HashMap::new();
        let mut redemptions = HashMap::new();
        let mut additional_coupon_dates = HashSet::new();

        for cashflow in val.cashflows() {
            match cashflow {
                Cashflow::Disbursement(c) => {
                    if let Ok(amount) = c.amount() {
                        disbursements.insert(c.payment_date(), amount);
                    }
                }
                Cashflow::Redemption(c) => {
                    if let Ok(amount) = c.amount() {
                        redemptions.insert(c.payment_date(), amount);
                    }
                }
                Cashflow::FixedRateCoupon(c) => {
                    if let Ok(start_date) = c.accrual_start_date() {
                        additional_coupon_dates.insert(start_date);
                    }
                    if let Ok(end_date) = c.accrual_end_date() {
                        additional_coupon_dates.insert(end_date);
                    }
                }
                Cashflow::FloatingRateCoupon(_) => (),
            }
        }

        let builder = Self::new()
            .with_start_date(val.start_date())
            .with_end_date(val.end_date())
            .with_rate(val.rate())
            .with_notional(val.notional())
            .with_discount_curve_id(val.discount_curve_id())
            .with_side(val.side())
            .with_currency(val.currency().unwrap_or(Currency::USD))
            .with_disbursements(disbursements)
            .with_redemptions(redemptions)
            .with_additional_coupon_dates(additional_coupon_dates)
            .with_payment_frequency(val.payment_frequency());

        match val.structure() {
            Structure::EqualPayments => builder.equal_payments(),
            Structure::Bullet | Structure::EqualRedemptions | Structure::Zero | Structure::Other => {
                builder.other()
            }
        }
    }
}

impl From<&FixedRateInstrument> for MakeFixedRateInstrument {
    fn from(val: &FixedRateInstrument) -> Self {
        Self::from(val.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        cashflows::{
            cashflow::{Cashflow, Side},
            traits::Payable,
        },
        currencies::enums::Currency,
        instruments::makefixedrateinstrument::MakeFixedRateInstrument,
        rates::{enums::Compounding, interestrate::InterestRate},
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        utils::errors::{AtlasError, Result},
        visitors::traits::HasCashflows,
    };
    use std::collections::{HashMap, HashSet};

    #[test]
    fn build_bullet() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.payment_frequency(), Frequency::Semiannual);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_equal_payments() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(2, TimeUnit::Months);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let notional = 1000.0;
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(notional)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_payments()
            .build()?;

        assert!((instrument.notional() - notional).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.payment_frequency(), Frequency::Monthly);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        for cf in instrument.cashflows() {
            println!("{cf}");
        }

        let mut payments = HashMap::new();
        for cf in instrument.cashflows() {
            match cf {
                Cashflow::FixedRateCoupon(c) => {
                    let amount = c.amount()?;
                    if payments.contains_key(&c.payment_date()) {
                        payments.insert(
                            c.payment_date(),
                            payments[&c.payment_date()] + amount,
                        );
                    } else {
                        payments.insert(c.payment_date(), amount);
                    }
                }
                Cashflow::Redemption(c) => {
                    let amount = c.amount()?;
                    if payments.contains_key(&c.payment_date()) {
                        payments.insert(
                            c.payment_date(),
                            payments[&c.payment_date()] + amount,
                        );
                    } else {
                        payments.insert(c.payment_date(), amount);
                    }
                }
                _ => (),
            }
        }

        //check if all equal
        let first = payments.values().next().ok_or(AtlasError::InvalidValueErr(
            "Payments cannot be empty".into(),
        ))?;
        for value in payments.values() {
            assert!((*value - *first).abs() < 1e-12);
        }

        Ok(())
    }

    #[test]
    fn build_equal_payments_with_delay_first_day() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(2, TimeUnit::Years);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let delay = 2;
        let first_coupon_date = start_date + Period::new(delay, TimeUnit::Months);

        let notional = 1000.0;
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_first_coupon_date(Some(first_coupon_date))
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(notional)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_payments()
            .build()?;

        assert!((instrument.notional() - notional).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.payment_frequency(), Frequency::Monthly);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_equal_redemptions() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_redemptions()
            .build()?;

        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.payment_frequency(), Frequency::Semiannual);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_equal_redemptions_with_tenor() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);

        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_tenor(Period::new(5, TimeUnit::Years))
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_redemptions()
            .build()?;

        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.payment_frequency(), Frequency::Semiannual);
        assert_eq!(instrument.start_date(), start_date);

        Ok(())
    }

    #[test]
    fn build_zero() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(1, TimeUnit::Years);

        let rate = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .zero()
            .build()?;

        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_zero_with_tenor() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let tenor = Period::new(1, TimeUnit::Years);

        let rate = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_tenor(tenor)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .zero()
            .build()?;

        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.rate(), rate);
        assert_eq!(instrument.start_date(), start_date);

        Ok(())
    }

    #[test]
    fn build_other() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(3, TimeUnit::Years);

        let mut disbursements = HashMap::new();
        disbursements.insert(start_date, 100.0);

        let mut redemptions = HashMap::new();
        redemptions.insert(start_date + Period::new(1, TimeUnit::Years), 30.0);
        redemptions.insert(end_date, 70.0);

        let mut additional_coupon_dates = HashSet::new();

        additional_coupon_dates.insert(start_date + Period::new(1, TimeUnit::Years));
        additional_coupon_dates.insert(start_date + Period::new(2, TimeUnit::Years));

        let rate = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Actual360,
        );

        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_disbursements(disbursements)
            .with_redemptions(redemptions)
            .with_additional_coupon_dates(additional_coupon_dates)
            .with_rate(rate)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .other()
            .build()?;

        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn into_test_1() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let notional = 100.0;
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(notional)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_payments()
            .build()?;

        let builder: MakeFixedRateInstrument = instrument.clone().into();
        let instrument2 = builder.build()?;
        assert!((instrument2.notional() - instrument.notional()).abs() < 1e-12);
        assert_eq!(instrument2.rate(), instrument.rate());

        assert_eq!(instrument2.payment_frequency(), Frequency::Monthly);
        assert_eq!(instrument2.start_date(), start_date);
        assert_eq!(instrument2.end_date(), end_date);
        assert_eq!(instrument2.cashflows().len(), instrument.cashflows().len());

        Ok(())
    }

    #[test]
    fn into_test_2() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(1, TimeUnit::Years);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let notional = 100.0;
        let instrument1 = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(notional)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_payments()
            .build()?;

        let builder: MakeFixedRateInstrument =
            MakeFixedRateInstrument::from(&instrument1).with_rate_value(0.06);
        let instrument2 = builder.build()?;

        assert!((instrument2.notional() - instrument1.notional()).abs() < 1e-12);

        Ok(())
    }

    #[test]
    // test the From traint
    fn from_test() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );
        let notional = 100.0;
        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(notional)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_payments()
            .build()?;

        let builder: MakeFixedRateInstrument = MakeFixedRateInstrument::from(&instrument);
        let instrument2 = builder.build()?;

        assert!((instrument2.notional() - instrument.notional()).abs() < 1e-12);
        assert_eq!(instrument2.rate(), instrument.rate());

        assert_eq!(instrument2.payment_frequency(), Frequency::Monthly);
        assert_eq!(instrument2.start_date(), start_date);
        assert_eq!(instrument2.end_date(), end_date);

        Ok(())
    }
}

// test section just for equal payment instruments
#[cfg(test)]
mod tests_equal_payment {
    use crate::rates::interestrate::InterestRate;
    use crate::{
        cashflows::{
            cashflow::{Cashflow, Side},
            traits::Payable,
        },
        currencies::enums::Currency,
        instruments::makefixedrateinstrument::{
            calculate_equal_payment_redemptions, MakeFixedRateInstrument,
        },
        rates::enums::Compounding,
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        utils::errors::Result,
        visitors::traits::HasCashflows,
    };

    #[test]
    fn test_calculate_equal_payment_vector() -> Result<()> {
        let notional = 100.0;
        let dates = vec![
            Date::new(2020, 1, 1),
            Date::new(2020, 12, 1),
            Date::new(2021, 1, 1),
            Date::new(2021, 2, 1),
            Date::new(2021, 3, 1),
            Date::new(2021, 4, 1),
            Date::new(2021, 5, 1),
            Date::new(2021, 6, 1),
            Date::new(2021, 7, 1),
            Date::new(2021, 8, 1),
            Date::new(2021, 9, 1),
            Date::new(2021, 10, 1),
            Date::new(2021, 11, 1),
            Date::new(2021, 12, 1),
            Date::new(2022, 1, 1),
            Date::new(2022, 2, 1),
            Date::new(2022, 3, 1),
            Date::new(2022, 4, 1),
            Date::new(2022, 5, 1),
        ];

        let rate = InterestRate::new(
            0.1,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );

        let redemptions = calculate_equal_payment_redemptions(&dates, rate, notional)?;

        assert_eq!(redemptions.len(), dates.len() - 1);
        assert!(redemptions[0] < 0.0);
        assert!(redemptions.iter().skip(1).all(|&x| x > 0.0));
        Ok(())
    }

    #[test]
    fn test_build_equal_payment_with_grace_period() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);

        let rate = InterestRate::new(
            0.1,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let grace_period = start_date + Period::new(12, TimeUnit::Months);

        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_tenor(Period::new(3, TimeUnit::Years))
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Pay)
            .with_currency(Currency::CLP)
            .with_first_coupon_date(Some(grace_period))
            .equal_payments()
            .build()?;

        for cf in instrument.cashflows() {
            assert!(cf.amount()? > 0.0);
        }

        let notional_calc =
            instrument
                .cashflows()
                .iter()
                .try_fold(0.0, |acc, cf| -> Result<f64> {
                    match cf {
                        Cashflow::Redemption(c) => Ok(acc + c.amount()?),
                        _ => Ok(acc),
                    }
                })?;

        assert!(notional_calc > 100.0);

        Ok(())
    }

    #[test]
    fn test_build_equal_payment_with_grace_period_and_capitalization() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);

        let rate = InterestRate::new(
            0.1,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let grace_period = start_date + Period::new(12, TimeUnit::Months);
        let notional = 100.0;

        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_tenor(Period::new(5, TimeUnit::Years))
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(notional)
            .with_side(Side::Pay)
            .with_currency(Currency::CLP)
            .with_first_coupon_date(Some(grace_period))
            .equal_payments()
            .build()?;

        for cf in instrument.cashflows() {
            println!("{cf}");
        }

        for cf in instrument.cashflows() {
            match &cf {
                Cashflow::Disbursement(c) | Cashflow::Redemption(c) => {
                    assert!(c.amount()? > 0.0);
                }
                _ => (),
            }
        }

        let notional_calc =
            instrument
                .cashflows()
                .iter()
                .try_fold(0.0, |acc, cf| -> Result<f64> {
                    match cf {
                        Cashflow::Redemption(c) => Ok(acc + c.amount()?),
                        _ => Ok(acc),
                    }
                })?;
        assert!(notional_calc > notional);

        let number_of_disbursements = instrument
            .cashflows()
            .iter()
            .filter(|cf| matches!(cf, Cashflow::Disbursement(_)))
            .count();
        assert!(number_of_disbursements > 1);

        Ok(())
    }

    #[test]
    fn test_into_equal_payment_with_grace_period() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);

        let rate = InterestRate::new(
            0.1,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );

        let grace_period = start_date + Period::new(12, TimeUnit::Months);

        let instrument_1 = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_tenor(Period::new(3, TimeUnit::Years))
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Pay)
            .with_currency(Currency::CLP)
            .with_first_coupon_date(Some(grace_period))
            .equal_payments()
            .build()?;

        let builder = MakeFixedRateInstrument::from(&instrument_1);
        let instrument_2 = builder.build()?;

        let notional_1 = instrument_1
            .cashflows()
            .iter()
            .try_fold(0.0, |acc, cf| -> Result<f64> {
                match cf {
                    Cashflow::Redemption(c) => Ok(acc + c.amount()?),
                    _ => Ok(acc),
                }
            })?;

        let notional_2 = instrument_2
            .cashflows()
            .iter()
            .try_fold(0.0, |acc, cf| -> Result<f64> {
                match cf {
                    Cashflow::Redemption(c) => Ok(acc + c.amount()?),
                    _ => Ok(acc),
                }
            })?;

        assert!((notional_1 - notional_2).abs() < 1e-6);
        Ok(())
    }
}
