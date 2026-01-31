use std::collections::{HashMap, HashSet};

use crate::{
    cashflows::cashflow::Side,
    currencies::enums::Currency,
    rates::interestrate::RateDefinition,
    time::{
        calendar::Calendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
    },
    utils::errors::{AtlasError, Result},
};

use super::{
    instrument::RateType, makefixedrateleg::MakeFixedRateLeg,
    makefloatingrateleg::MakeFloatingRateLeg, swap::Swap, traits::Structure,
};

/// A builder for creating a `Swap` instrument with two customized legs.
pub struct MakeSwap {
    first_leg_rate_type: Option<RateType>,
    first_leg_rate_value: Option<f64>,
    first_leg_rate_definition: Option<RateDefinition>,
    first_leg_currency: Option<Currency>,
    first_leg_side: Option<Side>,
    first_leg_structure: Option<Structure>,
    first_leg_discount_curve_id: Option<usize>,
    first_leg_forecast_curve_id: Option<usize>,
    first_leg_disbursements: Option<HashMap<Date, f64>>,
    first_leg_redemptions: Option<HashMap<Date, f64>>,
    first_leg_additional_coupon_dates: Option<HashSet<Date>>,
    first_leg_payment_frequency: Option<Frequency>,
    first_leg_calendar: Option<Calendar>,
    first_leg_business_day_convention: Option<BusinessDayConvention>,
    first_leg_date_generation_rule: Option<DateGenerationRule>,
    first_leg_end_of_month: Option<bool>,
    first_leg_start_date: Option<Date>,
    first_leg_end_date: Option<Date>,
    first_leg_notional: Option<f64>,

    second_leg_rate_type: Option<RateType>,
    second_leg_rate_value: Option<f64>,
    second_leg_rate_definition: Option<RateDefinition>,
    second_leg_currency: Option<Currency>,
    second_leg_side: Option<Side>,
    second_leg_structure: Option<Structure>,
    second_leg_discount_curve_id: Option<usize>,
    second_leg_forecast_curve_id: Option<usize>,
    second_leg_disbursements: Option<HashMap<Date, f64>>,
    second_leg_redemptions: Option<HashMap<Date, f64>>,
    second_leg_additional_coupon_dates: Option<HashSet<Date>>,
    second_leg_payment_frequency: Option<Frequency>,
    second_leg_calendar: Option<Calendar>,
    second_leg_business_day_convention: Option<BusinessDayConvention>,
    second_leg_date_generation_rule: Option<DateGenerationRule>,
    second_leg_end_of_month: Option<bool>,
    second_leg_start_date: Option<Date>,
    second_leg_end_date: Option<Date>,
    second_leg_notional: Option<f64>,

    id: Option<String>,
}

impl MakeSwap {
    /// Creates a new `MakeSwap` builder with default values.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            first_leg_rate_type: None,
            first_leg_rate_value: None,
            first_leg_rate_definition: None,
            first_leg_currency: None,
            first_leg_side: None,
            first_leg_structure: None,
            first_leg_notional: None,
            first_leg_discount_curve_id: None,
            first_leg_forecast_curve_id: None,
            first_leg_disbursements: None,
            first_leg_redemptions: None,
            first_leg_additional_coupon_dates: None,
            first_leg_payment_frequency: None,
            first_leg_calendar: None,
            first_leg_business_day_convention: None,
            first_leg_date_generation_rule: None,
            first_leg_end_of_month: None,
            first_leg_start_date: None,
            first_leg_end_date: None,

            second_leg_notional: None,
            second_leg_rate_type: None,
            second_leg_rate_value: None,
            second_leg_rate_definition: None,
            second_leg_currency: None,
            second_leg_side: None,
            second_leg_structure: None,
            second_leg_discount_curve_id: None,
            second_leg_forecast_curve_id: None,
            second_leg_disbursements: None,
            second_leg_redemptions: None,
            second_leg_additional_coupon_dates: None,
            second_leg_payment_frequency: None,
            second_leg_calendar: None,
            second_leg_date_generation_rule: None,
            second_leg_business_day_convention: None,
            second_leg_end_of_month: None,
            second_leg_start_date: None,
            second_leg_end_date: None,

            id: None,
        }
    }

    /// Sets the date generation rule for the first leg.
    #[must_use]
    pub const fn with_first_leg_date_generation_rule(
        mut self,
        date_generation_rule: Option<DateGenerationRule>,
    ) -> Self {
        self.first_leg_date_generation_rule = date_generation_rule;
        self
    }

    /// Sets the date generation rule for the second leg.
    #[must_use]
    pub const fn with_second_leg_date_generation_rule(
        mut self,
        date_generation_rule: Option<DateGenerationRule>,
    ) -> Self {
        self.second_leg_date_generation_rule = date_generation_rule;
        self
    }

    /// Sets the notional for the first leg.
    #[must_use]
    pub const fn with_first_leg_notional(mut self, notional: f64) -> Self {
        self.first_leg_notional = Some(notional);
        self
    }

    /// Sets the notional for the second leg.
    #[must_use]
    pub const fn with_second_leg_notional(mut self, notional: f64) -> Self {
        self.second_leg_notional = Some(notional);
        self
    }

    /// Sets the start date for the first leg.
    #[must_use]
    pub const fn with_first_leg_start_date(mut self, start_date: Date) -> Self {
        self.first_leg_start_date = Some(start_date);
        self
    }

    /// Sets the end date for the first leg.
    #[must_use]
    pub const fn with_first_leg_end_date(mut self, end_date: Date) -> Self {
        self.first_leg_end_date = Some(end_date);
        self
    }

    /// Sets the start date for the second leg.
    #[must_use]
    pub const fn with_second_leg_start_date(mut self, start_date: Date) -> Self {
        self.second_leg_start_date = Some(start_date);
        self
    }

    /// Sets the end date for the second leg.
    #[must_use]
    pub const fn with_second_leg_end_date(mut self, end_date: Date) -> Self {
        self.second_leg_end_date = Some(end_date);
        self
    }

    /// Sets the calendar for the first leg.
    #[must_use]
    pub fn with_first_leg_calendar(mut self, calendar: Option<Calendar>) -> Self {
        self.first_leg_calendar = calendar;
        self
    }

    /// Sets the business day convention for the first leg.
    #[must_use]
    pub const fn with_first_leg_business_day_convention(
        mut self,
        convention: Option<BusinessDayConvention>,
    ) -> Self {
        self.first_leg_business_day_convention = convention;
        self
    }

    /// Sets the end of month flag for the first leg.
    #[must_use]
    pub const fn with_first_leg_end_of_month(mut self, end_of_month: Option<bool>) -> Self {
        self.first_leg_end_of_month = end_of_month;
        self
    }

    /// Sets the calendar for the second leg.
    #[must_use]
    pub fn with_second_leg_calendar(mut self, calendar: Option<Calendar>) -> Self {
        self.second_leg_calendar = calendar;
        self
    }

    /// Sets the business day convention for the second leg.
    #[must_use]
    pub const fn with_second_leg_business_day_convention(
        mut self,
        convention: Option<BusinessDayConvention>,
    ) -> Self {
        self.second_leg_business_day_convention = convention;
        self
    }

    /// Sets the end of month flag for the second leg.
    #[must_use]
    pub const fn with_second_leg_end_of_month(mut self, end_of_month: Option<bool>) -> Self {
        self.second_leg_end_of_month = end_of_month;
        self
    }

    /// Sets additional coupon dates for the second leg.
    #[must_use]
    pub fn with_second_leg_additional_coupon_dates(mut self, dates: HashSet<Date>) -> Self {
        self.second_leg_additional_coupon_dates = Some(dates);
        self
    }

    /// Sets disbursements for the first leg.
    #[must_use]
    pub fn with_first_leg_disbursements(mut self, disbursements: HashMap<Date, f64>) -> Self {
        self.first_leg_disbursements = Some(disbursements);
        self
    }

    /// Sets redemptions for the first leg.
    #[must_use]
    pub fn with_first_leg_redemptions(mut self, redemptions: HashMap<Date, f64>) -> Self {
        self.first_leg_redemptions = Some(redemptions);
        self
    }

    /// Sets additional coupon dates for the first leg.
    #[must_use]
    pub fn with_first_leg_additional_coupon_dates(mut self, dates: HashSet<Date>) -> Self {
        self.first_leg_additional_coupon_dates = Some(dates);
        self
    }

    /// Sets disbursements for the second leg.
    #[must_use]
    pub fn with_second_leg_disbursements(mut self, disbursements: HashMap<Date, f64>) -> Self {
        self.second_leg_disbursements = Some(disbursements);
        self
    }

    /// Sets redemptions for the second leg.
    #[must_use]
    pub fn with_second_leg_redemptions(mut self, redemptions: HashMap<Date, f64>) -> Self {
        self.second_leg_redemptions = Some(redemptions);
        self
    }

    /// Sets the rate type for the first leg.
    #[must_use]
    pub const fn with_first_leg_rate_type(mut self, rate_type: RateType) -> Self {
        self.first_leg_rate_type = Some(rate_type);
        self
    }

    /// Sets the rate value for the first leg.
    #[must_use]
    pub const fn with_first_leg_rate_value(mut self, rate_value: f64) -> Self {
        self.first_leg_rate_value = Some(rate_value);
        self
    }

    /// Sets the payment frequency for the first leg.
    #[must_use]
    pub const fn with_first_leg_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.first_leg_payment_frequency = Some(frequency);
        self
    }

    /// Sets the rate definition for the first leg.
    #[must_use]
    pub const fn with_first_leg_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.first_leg_rate_definition = Some(rate_definition);
        self
    }

    /// Sets the currency for the first leg.
    #[must_use]
    pub const fn with_first_leg_currency(mut self, currency: Currency) -> Self {
        self.first_leg_currency = Some(currency);
        self
    }

    /// Sets the side for the first leg.
    #[must_use]
    pub const fn with_first_leg_side(mut self, side: Side) -> Self {
        self.first_leg_side = Some(side);
        self
    }

    /// Sets the discount curve ID for the first leg.
    #[must_use]
    pub const fn with_first_leg_discount_curve_id(mut self, curve_id: Option<usize>) -> Self {
        self.first_leg_discount_curve_id = curve_id;
        self
    }

    /// Sets the forecast curve ID for the first leg.
    #[must_use]
    pub const fn with_first_leg_forecast_curve_id(mut self, curve_id: Option<usize>) -> Self {
        self.first_leg_forecast_curve_id = curve_id;
        self
    }

    /// Sets the rate type for the second leg.
    #[must_use]
    pub const fn with_second_leg_rate_type(mut self, rate_type: RateType) -> Self {
        self.second_leg_rate_type = Some(rate_type);
        self
    }

    /// Sets the rate value for the second leg.
    #[must_use]
    pub const fn with_second_leg_rate_value(mut self, rate_value: f64) -> Self {
        self.second_leg_rate_value = Some(rate_value);
        self
    }

    /// Sets the rate definition for the second leg.
    #[must_use]
    pub const fn with_second_leg_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.second_leg_rate_definition = Some(rate_definition);
        self
    }

    /// Sets the currency for the second leg.
    #[must_use]
    pub const fn with_second_leg_currency(mut self, currency: Currency) -> Self {
        self.second_leg_currency = Some(currency);
        self
    }

    /// Sets the side for the second leg.
    #[must_use]
    pub const fn with_second_leg_side(mut self, side: Side) -> Self {
        self.second_leg_side = Some(side);
        self
    }

    /// Sets the discount curve ID for the second leg.
    #[must_use]
    pub const fn with_second_leg_discount_curve_id(mut self, curve_id: Option<usize>) -> Self {
        self.second_leg_discount_curve_id = curve_id;
        self
    }

    /// Sets the forecast curve ID for the second leg.
    #[must_use]
    pub const fn with_second_leg_forecast_curve_id(mut self, curve_id: Option<usize>) -> Self {
        self.second_leg_forecast_curve_id = curve_id;
        self
    }

    /// Sets the identifier for the instrument.
    #[must_use]
    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the structure for the first leg.
    #[must_use]
    pub const fn with_first_leg_structure(mut self, structure: Structure) -> Self {
        self.first_leg_structure = Some(structure);
        self
    }

    /// Sets the structure for the second leg.
    #[must_use]
    pub const fn with_second_leg_structure(mut self, structure: Structure) -> Self {
        self.second_leg_structure = Some(structure);
        self
    }

    /// Sets the payment frequency for the second leg.
    #[must_use]
    pub const fn with_second_leg_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.second_leg_payment_frequency = Some(frequency);
        self
    }

    /// Builds the `Swap` instrument from the configured parameters.
    ///
    /// # Errors
    /// Returns an error if required builder fields are missing or inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Swap> {
        let first_rate_type = self
            .first_leg_rate_type
            .ok_or(AtlasError::ValueNotSetErr("First Leg RateType".to_string()))?;

        let mut first_leg = match first_rate_type {
            RateType::Fixed => {
                let rate_value = self.first_leg_rate_value.ok_or(AtlasError::ValueNotSetErr(
                    "First Leg RateValue".to_string(),
                ))?;

                let rate_definition =
                    self.first_leg_rate_definition
                        .ok_or(AtlasError::ValueNotSetErr(
                            "First Leg RateDefinition".to_string(),
                        ))?;

                let currency = self
                    .first_leg_currency
                    .ok_or(AtlasError::ValueNotSetErr("First Leg Currency".to_string()))?;

                let side = self
                    .first_leg_side
                    .ok_or(AtlasError::ValueNotSetErr("First Leg Side".to_string()))?;

                let structure = self.first_leg_structure.ok_or(AtlasError::ValueNotSetErr(
                    "First Leg Structure".to_string(),
                ))?;

                let start_date = self.first_leg_start_date.ok_or(AtlasError::ValueNotSetErr(
                    "First Leg Start Date".to_string(),
                ))?;

                let end_date = self
                    .first_leg_end_date
                    .ok_or(AtlasError::ValueNotSetErr("First Leg End Date".to_string()))?;

                let notional = self
                    .first_leg_notional
                    .ok_or(AtlasError::ValueNotSetErr("First Leg Notional".to_string()))?;

                let builder = MakeFixedRateLeg::new()
                    .with_notional(notional)
                    .with_start_date(start_date)
                    .with_end_date(end_date)
                    .with_rate_value(rate_value)
                    .with_rate_definition(rate_definition)
                    .with_currency(currency)
                    .with_side(side)
                    .with_structure(structure)
                    .with_date_generation_rule(self.first_leg_date_generation_rule)
                    .with_business_day_convention(self.first_leg_business_day_convention)
                    .with_calendar(self.first_leg_calendar)
                    .with_end_of_month(self.first_leg_end_of_month)
                    .with_discount_curve_id(self.first_leg_discount_curve_id);

                if structure == Structure::Other {
                        let disbursements =
                            self.first_leg_disbursements
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "First Leg Disbursements".to_string(),
                                ))?;

                        let redemptions =
                            self.first_leg_redemptions
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "First Leg Redemptions".to_string(),
                                ))?;

                        let additional_coupon_dates =
                            self.first_leg_additional_coupon_dates.unwrap_or_default();

                        builder
                            .with_disbursements(disbursements)
                            .with_redemptions(redemptions)
                            .with_additional_coupon_dates(additional_coupon_dates)
                            .build()?
                } else {
                    let payment_frequency =
                        self.first_leg_payment_frequency
                            .ok_or(AtlasError::ValueNotSetErr(
                                "First Leg Payment Frequency".to_string(),
                            ))?;
                    builder.with_payment_frequency(payment_frequency).build()?
                }
            }
            RateType::Floating => {
                let rate_value = self.first_leg_rate_value.ok_or(AtlasError::ValueNotSetErr(
                    "First Leg RateValue".to_string(),
                ))?;

                let rate_definition =
                    self.first_leg_rate_definition
                        .ok_or(AtlasError::ValueNotSetErr(
                            "First Leg RateDefinition".to_string(),
                        ))?;

                let currency = self
                    .first_leg_currency
                    .ok_or(AtlasError::ValueNotSetErr("First Leg Currency".to_string()))?;

                let side = self
                    .first_leg_side
                    .ok_or(AtlasError::ValueNotSetErr("First Leg Side".to_string()))?;

                let structure = self.first_leg_structure.ok_or(AtlasError::ValueNotSetErr(
                    "First Leg Structure".to_string(),
                ))?;

                let start_date = self.first_leg_start_date.ok_or(AtlasError::ValueNotSetErr(
                    "First Leg Start Date".to_string(),
                ))?;

                let end_date = self
                    .first_leg_end_date
                    .ok_or(AtlasError::ValueNotSetErr("First Leg End Date".to_string()))?;

                let notional = self
                    .first_leg_notional
                    .ok_or(AtlasError::ValueNotSetErr("First Leg Notional".to_string()))?;

                let builder = MakeFloatingRateLeg::new()
                    .with_notional(notional)
                    .with_start_date(start_date)
                    .with_end_date(end_date)
                    .with_spread(rate_value)
                    .with_rate_definition(rate_definition)
                    .with_currency(currency)
                    .with_side(side)
                    .with_structure(structure)
                    .with_date_generation_rule(self.first_leg_date_generation_rule)
                    .with_business_day_convention(self.first_leg_business_day_convention)
                    .with_end_of_month(self.first_leg_end_of_month)
                    .with_calendar(self.first_leg_calendar)
                    .with_discount_curve_id(self.first_leg_discount_curve_id)
                    .with_forecast_curve_id(self.first_leg_forecast_curve_id);

                if structure == Structure::Other {
                        let disbursements =
                            self.first_leg_disbursements
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "First Leg Disbursements".to_string(),
                                ))?;

                        let redemptions =
                            self.first_leg_redemptions
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "First Leg Redemptions".to_string(),
                                ))?;

                        let additional_coupon_dates =
                            self.first_leg_additional_coupon_dates.unwrap_or_default();

                        builder
                            .with_disbursements(disbursements)
                            .with_redemptions(redemptions)
                            .with_additional_coupon_dates(additional_coupon_dates)
                            .build()?
                } else {
                    let payment_frequency =
                        self.first_leg_payment_frequency
                            .ok_or(AtlasError::ValueNotSetErr(
                                "First Leg Payment Frequency".to_string(),
                            ))?;
                    builder.with_payment_frequency(payment_frequency).build()?
                }
            }
            _ => Err(AtlasError::InvalidValueErr(format!(
                "RateType: {first_rate_type:?}"
            )))?,
        };

        let second_rate_type = self.second_leg_rate_type.ok_or(AtlasError::ValueNotSetErr(
            "Second Leg RateType".to_string(),
        ))?;

        let mut second_leg = match second_rate_type {
            RateType::Fixed => {
                let rate_value = self
                    .second_leg_rate_value
                    .ok_or(AtlasError::ValueNotSetErr(
                        "Second Leg RateValue".to_string(),
                    ))?;

                let rate_definition =
                    self.second_leg_rate_definition
                        .ok_or(AtlasError::ValueNotSetErr(
                            "Second Leg RateDefinition".to_string(),
                        ))?;

                let currency = self.second_leg_currency.ok_or(AtlasError::ValueNotSetErr(
                    "Second Leg Currency".to_string(),
                ))?;

                let side = self
                    .second_leg_side
                    .ok_or(AtlasError::ValueNotSetErr("Second Leg Side".to_string()))?;

                let structure = self.second_leg_structure.ok_or(AtlasError::ValueNotSetErr(
                    "Second Leg Structure".to_string(),
                ))?;

                let start_date = self
                    .second_leg_start_date
                    .ok_or(AtlasError::ValueNotSetErr(
                        "Second Leg Start Date".to_string(),
                    ))?;

                let end_date = self.second_leg_end_date.ok_or(AtlasError::ValueNotSetErr(
                    "Second Leg End Date".to_string(),
                ))?;

                let notional = self.second_leg_notional.ok_or(AtlasError::ValueNotSetErr(
                    "Second Leg Notional".to_string(),
                ))?;

                let builder = MakeFixedRateLeg::new()
                    .with_notional(notional)
                    .with_start_date(start_date)
                    .with_end_date(end_date)
                    .with_rate_value(rate_value)
                    .with_rate_definition(rate_definition)
                    .with_currency(currency)
                    .with_side(side)
                    .with_date_generation_rule(self.second_leg_date_generation_rule)
                    .with_business_day_convention(self.second_leg_business_day_convention)
                    .with_calendar(self.second_leg_calendar)
                    .with_structure(structure)
                    .with_discount_curve_id(self.second_leg_discount_curve_id);

                if structure == Structure::Other {
                        let disbursements =
                            self.second_leg_disbursements
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "Second Leg Disbursements".to_string(),
                                ))?;

                        let redemptions =
                            self.second_leg_redemptions
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "Second Leg Redemptions".to_string(),
                                ))?;

                        let additional_coupon_dates =
                            self.second_leg_additional_coupon_dates.unwrap_or_default();

                        builder
                            .with_disbursements(disbursements)
                            .with_redemptions(redemptions)
                            .with_additional_coupon_dates(additional_coupon_dates)
                            .build()?
                } else {
                    let payment_frequency =
                        self.second_leg_payment_frequency
                            .ok_or(AtlasError::ValueNotSetErr(
                                "Second Leg Payment Frequency".to_string(),
                            ))?;
                    builder.with_payment_frequency(payment_frequency).build()?
                }
            }
            RateType::Floating => {
                let rate_value = self
                    .second_leg_rate_value
                    .ok_or(AtlasError::ValueNotSetErr(
                        "Second leg RateValue".to_string(),
                    ))?;

                let rate_definition =
                    self.second_leg_rate_definition
                        .ok_or(AtlasError::ValueNotSetErr(
                            "Second leg RateDefinition".to_string(),
                        ))?;

                let currency = self.second_leg_currency.ok_or(AtlasError::ValueNotSetErr(
                    "Second leg Currency".to_string(),
                ))?;

                let side = self
                    .second_leg_side
                    .ok_or(AtlasError::ValueNotSetErr("Second leg Side".to_string()))?;

                let structure = self.second_leg_structure.ok_or(AtlasError::ValueNotSetErr(
                    "Second leg Structure".to_string(),
                ))?;

                let start_date = self
                    .second_leg_start_date
                    .ok_or(AtlasError::ValueNotSetErr(
                        "Second leg Start Date".to_string(),
                    ))?;

                let end_date = self.second_leg_end_date.ok_or(AtlasError::ValueNotSetErr(
                    "Second leg End Date".to_string(),
                ))?;

                let notional = self.second_leg_notional.ok_or(AtlasError::ValueNotSetErr(
                    "Second leg Notional".to_string(),
                ))?;

                let builder = MakeFloatingRateLeg::new()
                    .with_notional(notional)
                    .with_start_date(start_date)
                    .with_end_date(end_date)
                    .with_spread(rate_value)
                    .with_rate_definition(rate_definition)
                    .with_currency(currency)
                    .with_side(side)
                    .with_structure(structure)
                    .with_date_generation_rule(self.second_leg_date_generation_rule)
                    .with_business_day_convention(self.second_leg_business_day_convention)
                    .with_end_of_month(self.second_leg_end_of_month)
                    .with_discount_curve_id(self.second_leg_discount_curve_id)
                    .with_forecast_curve_id(self.second_leg_forecast_curve_id);

                if structure == Structure::Other {
                        let disbursements =
                            self.second_leg_disbursements
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "Second leg Disbursements".to_string(),
                                ))?;

                        let redemptions =
                            self.second_leg_redemptions
                                .ok_or(AtlasError::ValueNotSetErr(
                                    "Second leg Redemptions".to_string(),
                                ))?;

                        let additional_coupon_dates =
                            self.second_leg_additional_coupon_dates.unwrap_or_default();

                        builder
                            .with_disbursements(disbursements)
                            .with_redemptions(redemptions)
                            .with_additional_coupon_dates(additional_coupon_dates)
                            .build()?
                } else {
                    let payment_frequency =
                        self.second_leg_payment_frequency
                            .ok_or(AtlasError::ValueNotSetErr(
                                "Second leg Payment Frequency".to_string(),
                            ))?;
                    builder.with_payment_frequency(payment_frequency).build()?
                }
            }
            _ => Err(AtlasError::InvalidValueErr(format!(
                "RateType: {second_rate_type:?}"
            )))?,
        };

        let mut cashflows = Vec::new();
        cashflows.extend(first_leg.cashflows());
        cashflows.extend(second_leg.cashflows());

        // clear the leg cashflows to avoid unnecessary memory usage
        first_leg.clear();
        second_leg.clear();

        Ok(Swap::new(cashflows, vec![first_leg, second_leg], self.id))
    }
}

impl Default for MakeSwap {
    fn default() -> Self {
        Self::new()
    }
}

/// # `MakeFixFloatSwap`
///
/// Simplified version of the `MakeSwap` struct that only allows for a fixed and floating leg, with
/// the same notional, start and end dates, and currency.
pub struct MakeFixFloatSwap {
    rate_value: Option<f64>,
    rate_definition: Option<RateDefinition>,
    currency: Option<Currency>,
    fix_leg_side: Option<Side>,
    discount_curve_id: Option<usize>,
    forecast_curve_id: Option<usize>,
    notional: Option<f64>,
    start_date: Option<Date>,
    end_date: Option<Date>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    id: Option<String>,
}

impl MakeFixFloatSwap {
    /// Creates a new `MakeFixFloatSwap` builder with default values.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            rate_value: None,
            rate_definition: None,
            currency: None,
            fix_leg_side: None,
            discount_curve_id: None,
            forecast_curve_id: None,
            notional: None,
            start_date: None,
            end_date: None,
            calendar: None,
            business_day_convention: None,
            date_generation_rule: None,
            id: None,
        }
    }

    /// Sets the fixed rate value.
    #[must_use]
    pub const fn with_rate_value(mut self, rate_value: f64) -> Self {
        self.rate_value = Some(rate_value);
        self
    }

    /// Sets the rate definition.
    #[must_use]
    pub const fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the side for the fixed leg.
    #[must_use]
    pub const fn with_fix_leg_side(mut self, side: Side) -> Self {
        self.fix_leg_side = Some(side);
        self
    }

    /// Sets the discount curve ID.
    #[must_use]
    pub const fn with_discount_curve_id(mut self, curve_id: Option<usize>) -> Self {
        self.discount_curve_id = curve_id;
        self
    }

    /// Sets the forecast curve ID.
    #[must_use]
    pub const fn with_forecast_curve_id(mut self, curve_id: Option<usize>) -> Self {
        self.forecast_curve_id = curve_id;
        self
    }

    /// Sets the notional amount.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
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
        convention: Option<BusinessDayConvention>,
    ) -> Self {
        self.business_day_convention = convention;
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

    /// Sets the identifier.
    #[must_use]
    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    /// Builds the fixed-floating Swap instrument.
    ///
    /// # Errors
    /// Returns an error if required builder fields are missing or inconsistent.
    pub fn build(self) -> Result<Swap> {
        let rate_value = self
            .rate_value
            .ok_or(AtlasError::ValueNotSetErr("RateValue".to_string()))?;
        let rate_definition = self
            .rate_definition
            .ok_or(AtlasError::ValueNotSetErr("RateDefinition".to_string()))?;
        let currency = self
            .currency
            .ok_or(AtlasError::ValueNotSetErr("Currency".to_string()))?;
        let fix_leg_side = self
            .fix_leg_side
            .ok_or(AtlasError::ValueNotSetErr("Fix Leg Side".to_string()))?;
        let notional = self
            .notional
            .ok_or(AtlasError::ValueNotSetErr("Notional".to_string()))?;
        let start_date = self
            .start_date
            .ok_or(AtlasError::ValueNotSetErr("Start Date".to_string()))?;
        let end_date = self
            .end_date
            .ok_or(AtlasError::ValueNotSetErr("End Date".to_string()))?;

        MakeSwap::new()
            .with_first_leg_rate_type(RateType::Fixed)
            .with_first_leg_rate_value(rate_value)
            .with_first_leg_rate_definition(rate_definition)
            .with_first_leg_currency(currency)
            .with_first_leg_side(fix_leg_side)
            .with_first_leg_notional(notional)
            .with_first_leg_start_date(start_date)
            .with_first_leg_end_date(end_date)
            .with_first_leg_calendar(self.calendar.clone())
            .with_first_leg_business_day_convention(self.business_day_convention)
            .with_first_leg_date_generation_rule(self.date_generation_rule)
            .with_first_leg_discount_curve_id(self.discount_curve_id)
            .with_first_leg_forecast_curve_id(self.forecast_curve_id)
            .with_id(self.id.unwrap_or_default())
            .with_second_leg_rate_type(RateType::Floating)
            .with_second_leg_rate_value(0.0)
            .with_second_leg_rate_definition(rate_definition)
            .with_second_leg_currency(currency)
            .with_second_leg_side(fix_leg_side.inverse())
            .with_second_leg_notional(notional)
            .with_second_leg_start_date(start_date)
            .with_second_leg_end_date(end_date)
            .with_second_leg_calendar(self.calendar.clone())
            .with_second_leg_business_day_convention(self.business_day_convention)
            .with_second_leg_date_generation_rule(self.date_generation_rule)
            .with_second_leg_discount_curve_id(self.discount_curve_id)
            .with_second_leg_forecast_curve_id(self.forecast_curve_id)
            .build()
    }
}

impl Default for MakeFixFloatSwap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        cashflows::cashflow::Side,
        currencies::enums::Currency,
        instruments::{instrument::RateType, traits::Structure},
        rates::interestrate::RateDefinition,
        time::{date::Date, enums::Frequency},
        utils::errors::Result,
    };

    use super::MakeSwap;

    /// Test successful building of a Swap with all required fields set
    #[test]
    fn test_successful_build_swap() -> Result<()> {
        let start_date = Date::new(2021, 1, 1);
        let end_date = Date::new(2025, 1, 1);
        let notional = 1_000_000.0;

        let _ = MakeSwap::new()
            .with_first_leg_start_date(start_date)
            .with_first_leg_end_date(end_date)
            .with_first_leg_rate_type(RateType::Fixed)
            .with_first_leg_rate_value(0.05)
            .with_first_leg_notional(notional)
            .with_first_leg_rate_definition(RateDefinition::default())
            .with_first_leg_currency(Currency::USD)
            .with_first_leg_side(Side::Pay)
            .with_first_leg_structure(Structure::Bullet)
            .with_first_leg_payment_frequency(Frequency::Quarterly)
            .with_first_leg_discount_curve_id(Some(1))
            .with_second_leg_notional(notional)
            .with_second_leg_start_date(start_date)
            .with_second_leg_end_date(end_date)
            .with_second_leg_rate_type(RateType::Floating)
            .with_second_leg_rate_value(0.01) // This might represent a spread over a reference rate
            .with_second_leg_rate_definition(RateDefinition::default())
            .with_second_leg_currency(Currency::EUR)
            .with_second_leg_side(Side::Receive)
            .with_second_leg_structure(Structure::Bullet)
            .with_second_leg_payment_frequency(Frequency::Semiannual)
            .with_second_leg_discount_curve_id(Some(1))
            .with_second_leg_forecast_curve_id(Some(1))
            .build()?;

        Ok(())
    }
}
