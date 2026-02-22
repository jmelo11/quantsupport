use serde::{Deserialize, Serialize};

use crate::{
    core::{
        meta::{ForwardRateRequest, MarketRequest},
        traits::{HasCurrency, HasDiscountCurveId, HasForecastCurveId, Registrable},
    },
    currencies::enums::Currency,
    rates::interestrate::{InterestRate, RateDefinition},
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

use super::{
    cashflow::Side,
    simplecashflow::SimpleCashflow,
    traits::{Expires, InterestAccrual, Payable, RequiresFixingRate},
};

/// # `FloatingRateCoupon`
/// A floating rate coupon is a cashflow that pays a floating rate of interest on a notional amount.
///
/// ## Parameters
/// * `notional` - The notional amount of the coupon
/// * `spread` - The spread over the floating rate
/// * `accrual_start_date` - The date from which the coupon accrues interest
/// * `accrual_end_date` - The date until which the coupon accrues interest
/// * `payment_date` - The date on which the coupon is paid
/// * `fixing_date` - The date from which the floating rate is observed
/// * `rate_definition` - The definition of the floating rate
/// * `discount_curve_id` - The ID of the discount curve used to calculate the present value of the coupon
/// * `forecast_curve_id` - The ID of the forecast curve used to calculate the present value of the coupon
/// * `currency` - The currency of the coupon
/// * `side` - The side of the coupon (Pay or Receive)
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingRateCoupon {
    notional: f64,
    spread: f64,
    accrual_start_date: Date,
    accrual_end_date: Date,
    fixing_date: Option<Date>,
    rate_definition: RateDefinition,
    cashflow: SimpleCashflow,
    fixing_rate: Option<f64>,
    forecast_curve_id: Option<usize>,
}

impl FloatingRateCoupon {
    /// Creates a new floating rate coupon with the specified parameters.
    #[must_use]
    // allowed: high-arity API; refactor deferred
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        notional: f64,
        spread: f64,
        accrual_start_date: Date,
        accrual_end_date: Date,
        payment_date: Date,
        fixing_date: Option<Date>,
        rate_definition: RateDefinition,
        currency: Currency,
        side: Side,
    ) -> Self {
        Self {
            notional,
            spread,
            fixing_rate: None,
            accrual_start_date,
            accrual_end_date,
            fixing_date,
            rate_definition,
            forecast_curve_id: None,
            cashflow: SimpleCashflow::new(payment_date, currency, side),
        }
    }

    /// Sets the discount curve ID and returns the modified coupon.
    #[must_use]
    pub const fn with_discount_curve_id(mut self, id: usize) -> Self {
        self.cashflow = self.cashflow.with_discount_curve_id(id);
        self
    }

    /// Sets the forecast curve ID and returns the modified coupon.
    #[must_use]
    pub const fn with_forecast_curve_id(mut self, id: usize) -> Self {
        self.forecast_curve_id = Some(id);
        self
    }

    /// Sets the discount curve ID.
    pub const fn set_discount_curve_id(&mut self, id: usize) {
        self.cashflow.set_discount_curve_id(id);
    }

    /// Sets the forecast curve ID.
    pub const fn set_forecast_curve_id(&mut self, id: usize) {
        self.forecast_curve_id = Some(id);
    }

    /// Sets the spread and updates the cashflow if a fixing rate is set.
    pub fn set_spread(&mut self, spread: f64) {
        self.spread = spread;
        // if fixing rate is set, update the cashflow
        if let Some(fixing_rate) = self.fixing_rate {
            self.set_fixing_rate(fixing_rate);
        }
    }

    /// Sets the notional amount.
    pub const fn set_notional(&mut self, notional: f64) {
        self.notional = notional;
    }

    /// Returns the notional amount.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Returns the spread.
    #[must_use]
    pub const fn spread(&self) -> f64 {
        self.spread
    }

    /// Returns the rate definition.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Returns the fixing date, or the accrual start date if no fixing date is set.
    #[must_use]
    pub const fn fixing_date(&self) -> Date {
        match self.fixing_date {
            Some(date) => date,
            None => self.accrual_start_date,
        }
    }

    /// Returns the fixing rate if set.
    #[must_use]
    pub const fn fixing_rate(&self) -> Option<f64> {
        self.fixing_rate
    }
}

impl InterestAccrual for FloatingRateCoupon {
    fn accrual_start_date(&self) -> Result<Date> {
        Ok(self.accrual_start_date)
    }
    fn accrual_end_date(&self) -> Result<Date> {
        Ok(self.accrual_end_date)
    }
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        let fixing = self
            .fixing_rate
            .ok_or(AtlasError::ValueNotSetErr("Fixing rate".to_string()))?;
        let rate = InterestRate::from_rate_definition(fixing + self.spread, self.rate_definition);

        let (d1, d2) = self.relevant_accrual_dates(self.accrual_start_date, end_date)?;
        let acc_1 = self.notional * (rate.compound_factor(d1, d2) - 1.0);

        let (d1, d2) = self.relevant_accrual_dates(self.accrual_start_date, start_date)?;
        let acc_2 = self.notional * (rate.compound_factor(d1, d2) - 1.0);

        Ok(acc_1 - acc_2)
    }
}

impl RequiresFixingRate for FloatingRateCoupon {
    fn set_fixing_rate(&mut self, fixing_rate: f64) {
        self.fixing_rate = Some(fixing_rate);
        if let Ok(accrual) =
            self.accrued_amount(self.accrual_start_date, self.accrual_end_date)
        {
            self.cashflow = self.cashflow.with_amount(accrual);
        }
    }
}

impl Payable for FloatingRateCoupon {
    fn amount(&self) -> Result<f64> {
        self.cashflow.amount()
    }
    fn side(&self) -> Side {
        self.cashflow.side()
    }
    fn payment_date(&self) -> Date {
        self.cashflow.payment_date()
    }
}

impl HasCurrency for FloatingRateCoupon {
    fn currency(&self) -> Result<Currency> {
        self.cashflow.currency()
    }
}

impl HasDiscountCurveId for FloatingRateCoupon {
    fn discount_curve_id(&self) -> Result<usize> {
        self.cashflow.discount_curve_id()
    }
}

impl HasForecastCurveId for FloatingRateCoupon {
    fn forecast_curve_id(&self) -> Result<usize> {
        self.forecast_curve_id
            .ok_or(AtlasError::ValueNotSetErr("Forecast curve id".to_string()))
    }
}

impl Registrable for FloatingRateCoupon {
    fn id(&self) -> Result<usize> {
        self.cashflow.id()
    }

    fn set_id(&mut self, id: usize) {
        self.cashflow.set_id(id);
    }

    fn market_request(&self) -> Result<MarketRequest> {
        let tmp = self.cashflow.market_request()?;
        let forecast_curve_id = self.forecast_curve_id()?;
        let fixing_date = self.fixing_date();
        let forecast = ForwardRateRequest::new(
            forecast_curve_id,
            fixing_date,
            self.accrual_start_date,
            self.accrual_end_date,
            self.rate_definition.compounding(),
            self.rate_definition.frequency(),
        );
        Ok(MarketRequest::new(
            tmp.id(),
            tmp.df(),
            Some(forecast),
            tmp.fx(),
        ))
    }
}

impl Expires for FloatingRateCoupon {
    fn is_expired(&self, date: Date) -> bool {
        self.cashflow.payment_date() < date
    }
}
