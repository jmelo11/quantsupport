use std::collections::{HashMap, HashSet};

use crate::{
    cashflows::{
        cashflow::{Cashflow, CashflowType, Side},
        floatingratecoupon::FloatingRateCoupon,
        simplecashflow::SimpleCashflow,
        traits::{InterestAccrual, Payable},
    },
    core::traits::HasCurrency,
    currencies::enums::Currency,
    rates::interestrate::RateDefinition,
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
    floatingrateinstrument::FloatingRateInstrument,
    traits::{add_cashflows_to_vec, calculate_outstanding, notionals_vector, Structure},
};

/// # `MakeFloatingRateInstrument`
/// Builder for a floating rate loan.
#[derive(Debug, Clone)]
pub struct MakeFloatingRateInstrument {
    start_date: Option<Date>,
    end_date: Option<Date>,
    first_coupon_date: Option<Date>,
    payment_frequency: Option<Frequency>,
    tenor: Option<Period>,
    rate_definition: Option<RateDefinition>,
    notional: Option<f64>,
    currency: Option<Currency>,
    side: Option<Side>,
    spread: Option<f64>,
    structure: Option<Structure>,
    disbursements: Option<HashMap<Date, f64>>,
    redemptions: Option<HashMap<Date, f64>>,
    additional_coupon_dates: Option<HashSet<Date>>,
    forecast_curve_id: Option<usize>,
    discount_curve_id: Option<usize>,
    id: Option<String>,
    issue_date: Option<Date>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
}

/// Constructor, setters and getters.
impl MakeFloatingRateInstrument {
    /// Creates a new `MakeFloatingRateInstrument` with all fields initialized to `None`.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_date: None,
            end_date: None,
            first_coupon_date: None,
            payment_frequency: None,
            tenor: None,
            rate_definition: None,
            notional: None,
            spread: None,
            currency: None,
            side: None,
            structure: None,
            forecast_curve_id: None,
            discount_curve_id: None,
            disbursements: None,
            redemptions: None,
            additional_coupon_dates: None,
            id: None,
            issue_date: None,
            calendar: None,
            business_day_convention: None,
            date_generation_rule: None,
        }
    }

    /// Sets the calendar for the instrument.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Option<Calendar>) -> Self {
        self.calendar = calendar;
        self
    }

    /// Sets the business day convention for the instrument.
    #[must_use]
    pub const fn with_business_day_convention(
        mut self,
        business_day_convention: Option<BusinessDayConvention>,
    ) -> Self {
        self.business_day_convention = business_day_convention;
        self
    }

    /// Sets the date generation rule for the instrument.
    #[must_use]
    pub const fn with_date_generation_rule(
        mut self,
        date_generation_rule: Option<DateGenerationRule>,
    ) -> Self {
        self.date_generation_rule = date_generation_rule;
        self
    }

    /// Sets the issue date for the instrument.
    #[must_use]
    pub const fn with_issue_date(mut self, issue_date: Date) -> Self {
        self.issue_date = Some(issue_date);
        self
    }

    /// Sets the identifier for the instrument.
    #[must_use]
    pub fn with_id(mut self, id: Option<String>) -> Self {
        self.id = id;
        self
    }

    /// Sets the first coupon date for the instrument.
    #[must_use]
    pub const fn with_first_coupon_date(
        mut self,
        first_coupon_date: Option<Date>,
    ) -> Self {
        self.first_coupon_date = first_coupon_date;
        self
    }

    /// Sets the start date for the instrument.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the end date for the instrument.
    #[must_use]
    pub const fn with_end_date(mut self, end_date: Date) -> Self {
        self.end_date = Some(end_date);
        self
    }

    /// Sets the tenor for the instrument.
    #[must_use]
    pub const fn with_tenor(mut self, tenor: Period) -> Self {
        self.tenor = Some(tenor);
        self
    }

    /// Sets the disbursements schedule for the instrument.
    #[must_use]
    pub fn with_disbursements(
        mut self,
        disbursements: HashMap<Date, f64>,
    ) -> Self {
        self.disbursements = Some(disbursements);
        self
    }

    /// Sets the redemptions schedule for the instrument.
    #[must_use]
    pub fn with_redemptions(
        mut self,
        redemptions: HashMap<Date, f64>,
    ) -> Self {
        self.redemptions = Some(redemptions);
        self
    }

    /// Sets the additional coupon dates for the instrument.
    #[must_use]
    pub fn with_additional_coupon_dates(
        mut self,
        additional_coupon_dates: HashSet<Date>,
    ) -> Self {
        self.additional_coupon_dates = Some(additional_coupon_dates);
        self
    }

    /// Sets the forecast curve identifier for the instrument.
    #[must_use]
    pub const fn with_forecast_curve_id(
        mut self,
        forecast_curve_id: Option<usize>,
    ) -> Self {
        self.forecast_curve_id = forecast_curve_id;
        self
    }

    /// Sets the discount curve identifier for the instrument.
    #[must_use]
    pub const fn with_discount_curve_id(
        mut self,
        discount_curve_id: Option<usize>,
    ) -> Self {
        self.discount_curve_id = discount_curve_id;
        self
    }

    /// Sets the rate definition for the instrument.
    #[must_use]
    pub const fn with_rate_definition(
        mut self,
        rate_definition: RateDefinition,
    ) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the notional amount for the instrument.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the currency for the instrument.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the spread for the floating rate instrument.
    #[must_use]
    pub const fn with_spread(mut self, spread: f64) -> Self {
        self.spread = Some(spread);
        self
    }

    /// Sets the instrument structure to bullet.
    #[must_use]
    pub const fn bullet(mut self) -> Self {
        self.structure = Some(Structure::Bullet);
        self
    }

    /// Sets the instrument structure to equal redemptions.
    #[must_use]
    pub const fn equal_redemptions(mut self) -> Self {
        self.structure = Some(Structure::EqualRedemptions);
        self
    }

    /// Sets the instrument structure to zero with single payment frequency.
    #[must_use]
    pub const fn zero(mut self) -> Self {
        self.structure = Some(Structure::Zero);
        self.payment_frequency = Some(Frequency::Once);
        self
    }

    /// Sets the instrument structure to other with custom frequency.
    #[must_use]
    pub const fn other(mut self) -> Self {
        self.structure = Some(Structure::Other);
        self.payment_frequency = Some(Frequency::OtherFrequency);
        self
    }

    /// Sets the side (Receive or Pay) for the instrument.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the payment frequency for the instrument.
    #[must_use]
    pub const fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the structure for the instrument.
    #[must_use]
    pub const fn with_structure(mut self, structure: Structure) -> Self {
        self.structure = Some(structure);
        self
    }
}

impl Default for MakeFloatingRateInstrument {
    fn default() -> Self {
        Self::new()
    }
}

/// Build
impl MakeFloatingRateInstrument {
    /// Builds and returns a `FloatingRateInstrument` from the configured builder.
    ///
    /// # Errors
    /// Returns an error if required builder fields are missing or inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<FloatingRateInstrument> {
        let mut cashflows = Vec::new();
        let structure = self
            .structure
            .ok_or(AtlasError::ValueNotSetErr("Structure".into()))?;
        let rate_definition = self
            .rate_definition
            .ok_or(AtlasError::ValueNotSetErr("Rate definition".into()))?;
        let spread = self
            .spread
            .ok_or(AtlasError::ValueNotSetErr("Spread".into()))?;
        let currency = self
            .currency
            .ok_or(AtlasError::ValueNotSetErr("Currency".into()))?;
        let side = self.side.ok_or(AtlasError::ValueNotSetErr("Side".into()))?;
        let payment_frequency = self
            .payment_frequency
            .ok_or(AtlasError::ValueNotSetErr("Payment frequency".into()))?;
        match structure {
            Structure::Bullet => {
                // common
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

                let schedule = match self.first_coupon_date {
                    Some(date) => {
                        if date > start_date {
                            schedule_builder.with_first_date(date).build()?
                        } else {
                            Err(AtlasError::InvalidValueErr(
                                "First coupon date must be after start date".into(),
                            ))?
                        }
                    }
                    None => schedule_builder.build()?,
                };

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                // end common
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
                    spread,
                    rate_definition,
                    side,
                    currency,
                );
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
                if let Some(id) = self.forecast_curve_id {
                    for cf in &mut cashflows {
                        cf.set_forecast_curve_id(id);
                    }
                }

                Ok(FloatingRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    spread,
                    side,
                    cashflows,
                    payment_frequency,
                    rate_definition,
                    structure,
                    currency,
                    self.discount_curve_id,
                    self.forecast_curve_id,
                    self.id,
                    self.issue_date,
                ))
            }
            Structure::Zero => {
                // common
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
                    )
                    .build()?;
                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                // end common

                let notionals =
                    notionals_vector(schedule.dates().len() - 1, notional, Structure::Zero);
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
                    spread,
                    rate_definition,
                    side,
                    currency,
                );
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
                if let Some(id) = self.forecast_curve_id {
                    for cf in &mut cashflows {
                        cf.set_forecast_curve_id(id);
                    }
                }

                Ok(FloatingRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    spread,
                    side,
                    cashflows,
                    payment_frequency,
                    rate_definition,
                    structure,
                    currency,
                    self.discount_curve_id,
                    self.forecast_curve_id,
                    self.id,
                    self.issue_date,
                ))
            }
            Structure::EqualRedemptions => {
                // common
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

                let schedule = match self.first_coupon_date {
                    Some(date) => {
                        if date > start_date {
                            schedule_builder.with_first_date(date).build()?
                        } else {
                            Err(AtlasError::InvalidValueErr(
                                "First coupon date must be after start date".into(),
                            ))?
                        }
                    }
                    None => schedule_builder.build()?,
                };

                let notional = self
                    .notional
                    .ok_or(AtlasError::ValueNotSetErr("Notional".into()))?;

                // end common

                let n = schedule.dates().len() - 1;
                let notionals = notionals_vector(n, notional, Structure::EqualRedemptions);
                let n_f64 = f64::from(u32::try_from(n).map_err(|_| {
                    AtlasError::InvalidValueErr("Redemption count exceeds u32".into())
                })?);
                let redemptions = vec![notional / n_f64; n];

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
                build_coupons_from_notionals(
                    &mut cashflows,
                    schedule.dates(),
                    &notionals,
                    spread,
                    rate_definition,
                    side,
                    currency,
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
                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }
                if let Some(id) = self.forecast_curve_id {
                    for cf in &mut cashflows {
                        cf.set_forecast_curve_id(id);
                    }
                }

                Ok(FloatingRateInstrument::new(
                    start_date,
                    end_date,
                    notional,
                    spread,
                    side,
                    cashflows,
                    payment_frequency,
                    rate_definition,
                    structure,
                    currency,
                    self.discount_curve_id,
                    self.forecast_curve_id,
                    self.id,
                    self.issue_date,
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
                    Err(AtlasError::InvalidValueErr(
                        "Redemption amount must equal disbursement amount".into(),
                    ))?;
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
                    let coupon = FloatingRateCoupon::new(
                        *notional,
                        spread,
                        *start_date,
                        *end_date,
                        *end_date,
                        Some(*start_date),
                        rate_definition,
                        currency,
                        side,
                    );
                    cashflows.push(Cashflow::FloatingRateCoupon(coupon));
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

                let payment_frequency = self
                    .payment_frequency
                    .unwrap_or_else(|| panic!("Payment frequency not set"));

                if let Some(id) = self.discount_curve_id {
                    for cf in &mut cashflows {
                        cf.set_discount_curve_id(id);
                    }
                }
                if let Some(id) = self.forecast_curve_id {
                    for cf in &mut cashflows {
                        cf.set_forecast_curve_id(id);
                    }
                }
                Ok(FloatingRateInstrument::new(
                    *start_date,
                    *end_date,
                    notional,
                    spread,
                    side,
                    cashflows,
                    payment_frequency,
                    rate_definition,
                    structure,
                    currency,
                    self.discount_curve_id,
                    self.forecast_curve_id,
                    self.id,
                    self.issue_date,
                ))
            }
            Structure::EqualPayments => Err(AtlasError::InvalidValueErr(
                "Invalid structure for floating rate loan".into(),
            ))?,
        }
    }
}

fn build_coupons_from_notionals(
    cashflows: &mut Vec<Cashflow>,
    dates: &[Date],
    notionals: &[f64],
    spread: f64,
    rate_definition: RateDefinition,
    side: Side,
    currency: Currency,
) {
    for (date_pair, notional) in dates.windows(2).zip(notionals) {
        let d1 = date_pair[0];
        let d2 = date_pair[1];
        let coupon = FloatingRateCoupon::new(
            *notional,
            spread,
            d1,
            d2,
            d2,
            Some(d1),
            rate_definition,
            currency,
            side,
        );
        cashflows.push(Cashflow::FloatingRateCoupon(coupon));
    }
}

impl From<FloatingRateInstrument> for MakeFloatingRateInstrument {
    fn from(val: FloatingRateInstrument) -> Self {
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
                Cashflow::FloatingRateCoupon(c) => {
                    if let Ok(start_date) = c.accrual_start_date() {
                        additional_coupon_dates.insert(start_date);
                    }
                    if let Ok(end_date) = c.accrual_end_date() {
                        additional_coupon_dates.insert(end_date);
                    }
                }
                Cashflow::FixedRateCoupon(_) => (),
            }
        }

        Self::new()
            .with_start_date(val.start_date())
            .with_end_date(val.end_date())
            .with_notional(val.notional())
            .with_spread(val.spread())
            .with_side(val.side())
            .with_rate_definition(val.rate_definition())
            .with_forecast_curve_id(val.forecast_curve_id())
            .with_discount_curve_id(val.discount_curve_id())
            .with_payment_frequency(val.payment_frequency())
            .with_currency(val.currency().unwrap_or(Currency::USD))
            .with_disbursements(disbursements)
            .with_redemptions(redemptions)
            .with_additional_coupon_dates(additional_coupon_dates)
            .other()
    }
}

impl From<&FloatingRateInstrument> for MakeFloatingRateInstrument {
    fn from(instrument: &FloatingRateInstrument) -> Self {
        instrument.clone().into()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use crate::{
        cashflows::{cashflow::Side, traits::RequiresFixingRate},
        core::traits::HasCurrency,
        currencies::enums::Currency,
        instruments::{makefloatingrateinstrument::MakeFloatingRateInstrument, traits::Structure},
        rates::{enums::Compounding, interestrate::RateDefinition},
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
    fn build_bullet() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Annual,
        );

        let mut instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(0.05)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        for cf in instrument.mut_cashflows() {
            cf.set_fixing_rate(0.002);
        }
        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_zero() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Annual,
        );

        let mut instrument = super::MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_spread(0.05)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .zero()
            .build()?;

        for cf in instrument.mut_cashflows() {
            cf.set_fixing_rate(0.002);
        }
        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_equal_redemptions() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Annual,
        );

        let mut instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(0.05)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_redemptions()
            .build()?;

        for cf in instrument.mut_cashflows() {
            cf.set_fixing_rate(0.002);
        }
        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);

        Ok(())
    }

    #[test]
    fn build_equal_redemptions_with_tenor() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Annual,
        );

        let mut instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_tenor(Period::new(23, TimeUnit::Months))
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(0.05)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .equal_redemptions()
            .build()?;

        for cf in instrument.mut_cashflows() {
            cf.set_fixing_rate(0.002);
        }
        assert!((instrument.notional() - 100.0).abs() < 1e-12);
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

        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Annual,
        );

        let mut instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_disbursements(disbursements)
            .with_redemptions(redemptions)
            .with_additional_coupon_dates(additional_coupon_dates)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(0.05)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .other()
            .build()?;

        for cf in instrument.mut_cashflows() {
            cf.set_fixing_rate(0.002);
        }
        assert!((instrument.notional() - 100.0).abs() < 1e-12);
        assert_eq!(instrument.start_date(), start_date);

        Ok(())
    }

    #[test]
    fn into_test_1() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Compounded,
            Frequency::Annual,
        );
        let notional = 100.0;

        let instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(0.05)
            .with_notional(notional)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        let builder = MakeFloatingRateInstrument::from(&instrument);
        let instrument2 = builder.build()?;

        assert!((instrument2.notional() - instrument.notional()).abs() < 1e-12);
        assert_eq!(instrument2.start_date(), instrument.start_date());
        assert_eq!(instrument2.end_date(), instrument.end_date());
        assert_eq!(instrument2.rate_definition(), instrument.rate_definition());
        assert_ne!(
            instrument2.payment_frequency(),
            instrument.payment_frequency()
        );
        assert!((instrument2.spread() - instrument.spread()).abs() < 1e-12);
        assert_eq!(instrument2.side(), instrument.side());
        assert_eq!(instrument2.currency()?, instrument.currency()?);
        assert_eq!(
            instrument2.discount_curve_id(),
            instrument.discount_curve_id()
        );
        assert_eq!(
            instrument2.forecast_curve_id(),
            instrument.forecast_curve_id()
        );
        assert_eq!(instrument2.structure(), Structure::Other);
        assert_eq!(instrument.structure(), Structure::Bullet);

        Ok(())
    }
}
