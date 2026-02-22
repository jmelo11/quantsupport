use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::{
    core::{
        meta::MarketRequest,
        traits::{HasCurrency, HasDiscountCurveId, HasForecastCurveId, Registrable},
    },
    currencies::enums::Currency,
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

use super::{
    fixedratecoupon::FixedRateCoupon,
    floatingratecoupon::FloatingRateCoupon,
    simplecashflow::SimpleCashflow,
    traits::{InterestAccrual, Payable, RequiresFixingRate},
};

/// # `Side`
/// Enum that represents the side of a cashflow.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Side {
    /// A payment obligation.
    Pay,
    /// A payment receipt.
    Receive,
}

impl Side {
    /// Returns the sign of the side as a multiplier (-1.0 for Pay, 1.0 for Receive).
    #[must_use]
    pub const fn sign(&self) -> f64 {
        match self {
            Self::Pay => -1.0,
            Self::Receive => 1.0,
        }
    }

    /// Returns the inverse side (Pay becomes Receive, and vice versa).
    #[must_use]
    pub const fn inverse(&self) -> Self {
        match self {
            Self::Pay => Self::Receive,
            Self::Receive => Self::Pay,
        }
    }
}

impl TryFrom<String> for Side {
    type Error = AtlasError;

    fn try_from(s: String) -> Result<Self> {
        match s.as_str() {
            "Pay" => Ok(Self::Pay),
            "Receive" => Ok(Self::Receive),
            _ => Err(AtlasError::InvalidValueErr(format!("Invalid side: {s}"))),
        }
    }
}

impl From<Side> for String {
    fn from(side: Side) -> Self {
        match side {
            Side::Pay => "Pay".to_string(),
            Side::Receive => "Receive".to_string(),
        }
    }
}

/// # `Cashflow`
/// Enum that represents a cashflow.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Cashflow {
    /// A redemption cashflow.
    Redemption(SimpleCashflow),
    /// A disbursement cashflow.
    Disbursement(SimpleCashflow),
    /// A fixed rate coupon cashflow.
    FixedRateCoupon(FixedRateCoupon),
    /// A floating rate coupon cashflow.
    FloatingRateCoupon(FloatingRateCoupon),
}

impl Cashflow {
    /// Sets the discount curve ID for this cashflow.
    pub const fn set_discount_curve_id(&mut self, id: usize) {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => {
                cashflow.set_discount_curve_id(id);
            }
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.set_discount_curve_id(id),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.set_discount_curve_id(id),
        }
    }

    /// Sets the forecast curve ID for floating rate coupons.
    pub const fn set_forecast_curve_id(&mut self, id: usize) {
        if let Self::FloatingRateCoupon(coupon) = self {
            coupon.set_forecast_curve_id(id);
        }
    }
}

impl Payable for Cashflow {
    fn amount(&self) -> Result<f64> {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.amount(),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.amount(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.amount(),
        }
    }

    fn side(&self) -> Side {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.side(),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.side(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.side(),
        }
    }

    fn payment_date(&self) -> Date {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.payment_date(),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.payment_date(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.payment_date(),
        }
    }
}

impl HasCurrency for Cashflow {
    fn currency(&self) -> Result<Currency> {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.currency(),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.currency(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.currency(),
        }
    }
}

impl HasDiscountCurveId for Cashflow {
    fn discount_curve_id(&self) -> Result<usize> {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => {
                cashflow.discount_curve_id()
            }
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.discount_curve_id(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.discount_curve_id(),
        }
    }
}

impl HasForecastCurveId for Cashflow {
    fn forecast_curve_id(&self) -> Result<usize> {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => {
                cashflow.forecast_curve_id()
            }
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.forecast_curve_id(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.forecast_curve_id(),
        }
    }
}

impl Registrable for Cashflow {
    fn set_id(&mut self, id: usize) {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.set_id(id),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.set_id(id),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.set_id(id),
        }
    }

    fn id(&self) -> Result<usize> {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.id(),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.id(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.id(),
        }
    }

    fn market_request(&self) -> Result<MarketRequest> {
        match self {
            Self::Redemption(cashflow) | Self::Disbursement(cashflow) => cashflow.market_request(),
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.market_request(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.market_request(),
        }
    }
}

impl InterestAccrual for Cashflow {
    fn accrual_end_date(&self) -> Result<Date> {
        match self {
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.accrual_end_date(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.accrual_end_date(),
            Self::Disbursement(_) | Self::Redemption(_) => Err(AtlasError::InvalidValueErr(
                "Disbursement and Redemption cashflows do not have an accrual end date".to_string(),
            )),
        }
    }

    fn accrual_start_date(&self) -> Result<Date> {
        match self {
            Self::FixedRateCoupon(fixed_coupon) => fixed_coupon.accrual_start_date(),
            Self::FloatingRateCoupon(floating_coupon) => floating_coupon.accrual_start_date(),
            Self::Disbursement(_) | Self::Redemption(_) => Err(AtlasError::InvalidValueErr(
                "Disbursement and Redemption cashflows do not have an accrual start date"
                    .to_string(),
            )),
        }
    }

    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        match self {
            Self::FixedRateCoupon(fixed_coupon) => {
                fixed_coupon.accrued_amount(start_date, end_date)
            }
            Self::FloatingRateCoupon(floating_coupon) => {
                floating_coupon.accrued_amount(start_date, end_date)
            }
            _ => Ok(0.0),
        }
    }
}

impl RequiresFixingRate for Cashflow {
    fn set_fixing_rate(&mut self, fixing_rate: f64) {
        if let Self::FloatingRateCoupon(coupon) = self {
            coupon.set_fixing_rate(fixing_rate);
        }
    }
}

impl Display for Cashflow {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let amount = self.amount().unwrap_or(0.0);
        match self {
            Self::Redemption(cashflow) => write!(
                f,
                "date: {}, amount: {}, side: {:?}, type: redemption",
                cashflow.payment_date(),
                amount,
                cashflow.side()
            ),
            Self::Disbursement(cashflow) => write!(
                f,
                "date: {}, amount: {}, side: {:?}, type: disbursement",
                cashflow.payment_date(),
                amount,
                cashflow.side()
            ),
            Self::FixedRateCoupon(coupon) => write!(
                f,
                "date: {}, amount: {}, side: {:?}, type: fixed rate coupon",
                coupon.payment_date(),
                amount,
                coupon.side()
            ),
            Self::FloatingRateCoupon(coupon) => write!(
                f,
                "date: {}, amount: {}, side: {:?}, type: floating rate coupon",
                coupon.payment_date(),
                amount,
                coupon.side()
            ),
        }
    }
}

/// # `CashflowType`
/// Enum that represents the type of a cashflow.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CashflowType {
    /// A redemption type.
    Redemption,
    /// A disbursement type.
    Disbursement,
    /// A fixed rate coupon type.
    FixedRateCoupon,
    /// A floating rate coupon type.
    FloatingRateCoupon,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn serialization_test() -> Result<()> {
        let cashflow = Cashflow::Redemption(SimpleCashflow::new(
            Date::new(2024, 1, 1),
            Currency::USD,
            Side::Receive,
        ));
        let serialized = serde_json::to_string(&cashflow)
            .map_err(|err| AtlasError::SerializationErr(err.to_string()))?;
        println!("{serialized}");

        let deserialized: Cashflow = serde_json::from_str(&serialized)
            .map_err(|err| AtlasError::DeserializationErr(err.to_string()))?;
        assert_eq!(cashflow, deserialized);
        Ok(())
    }
}
