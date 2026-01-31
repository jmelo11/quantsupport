use serde::{Deserialize, Serialize};

use crate::{
    core::{
        meta::{DiscountFactorRequest, ExchangeRateRequest, MarketRequest},
        traits::{HasCurrency, HasDiscountCurveId, HasForecastCurveId, Registrable},
    },
    currencies::enums::Currency,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

use super::cashflow::Side;
use super::traits::{Expires, Payable};

/// # `SimpleCashflow`
/// A simple cashflow that is payable at a given date.
///
/// ## Example
/// ```
/// use rustatlas::prelude::*;
/// let payment_date = Date::new(2020, 1, 1);
/// let cashflow = SimpleCashflow::new(payment_date, Currency::USD, Side::Receive).with_amount(100.0);
/// assert_eq!(cashflow.side(), Side::Receive);
/// assert_eq!(cashflow.payment_date(), payment_date);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleCashflow {
    payment_date: Date,
    currency: Currency,
    side: Side,
    amount: Option<f64>,
    discount_curve_id: Option<usize>,
    id: Option<usize>,
}

impl SimpleCashflow {
    /// Creates a new `SimpleCashflow` with the given payment date, currency, and side.
    #[must_use]
    pub const fn new(payment_date: Date, currency: Currency, side: Side) -> Self {
        Self {
            payment_date,
            currency,
            side,
            amount: None,
            discount_curve_id: None,
            id: None,
        }
    }

    /// Sets the amount for this cashflow and returns self for method chaining.
    #[must_use]
    pub const fn with_amount(mut self, amount: f64) -> Self {
        self.amount = Some(amount);
        self
    }

    #[must_use]
    /// Sets the discount curve ID for this cashflow and returns self for method chaining.
    pub const fn with_discount_curve_id(mut self, discount_curve_id: usize) -> Self {
        self.discount_curve_id = Some(discount_curve_id);
        self
    }

    /// Sets the registry ID for this cashflow and returns self for method chaining.
    #[must_use]
    pub const fn with_id(mut self, registry_id: usize) -> Self {
        self.id = Some(registry_id);
        self
    }

    /// Sets the discount curve ID for this cashflow.
    pub const fn set_discount_curve_id(&mut self, id: usize) {
        self.discount_curve_id = Some(id);
    }

    /// Sets the amount for this cashflow.
    pub const fn set_amount(&mut self, amount: f64) {
        self.amount = Some(amount);
    }
}

impl HasCurrency for SimpleCashflow {
    fn currency(&self) -> Result<Currency> {
        Ok(self.currency)
    }
}

impl HasDiscountCurveId for SimpleCashflow {
    fn discount_curve_id(&self) -> Result<usize> {
        self.discount_curve_id
            .ok_or(AtlasError::ValueNotSetErr("Discount curve id".to_string()))
    }
}

impl HasForecastCurveId for SimpleCashflow {
    fn forecast_curve_id(&self) -> Result<usize> {
        Err(AtlasError::InvalidValueErr(
            "No forecast curve id for simple cashflow".to_string(),
        ))
    }
}

impl Registrable for SimpleCashflow {
    fn id(&self) -> Result<usize> {
        self.id.ok_or(AtlasError::ValueNotSetErr("Id".to_string()))
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn market_request(&self) -> Result<MarketRequest> {
        let id = self.id()?;
        let discount_curve_id = self.discount_curve_id()?;
        let currency = self.currency()?;
        let currency_request = ExchangeRateRequest::new(currency, None, None);
        let discount_request = DiscountFactorRequest::new(discount_curve_id, self.payment_date);
        Ok(MarketRequest::new(
            id,
            Some(discount_request),
            None,
            Some(currency_request),
        ))
    }
}

impl Payable for SimpleCashflow {
    fn amount(&self) -> Result<f64> {
        self.amount.ok_or(AtlasError::ValueNotSetErr(
            "Amount not set for simple cashflow".to_string(),
        ))
    }
    fn side(&self) -> Side {
        self.side
    }

    fn payment_date(&self) -> Date {
        self.payment_date
    }
}

impl Expires for SimpleCashflow {
    fn is_expired(&self, date: Date) -> bool {
        self.payment_date < date
    }
}
