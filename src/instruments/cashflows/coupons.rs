use crate::{
    ad::adreal::{ADReal, FloatExt, IsReal},
    instruments::cashflows::cashflow::Cashflow,
    time::date::Date,
    utils::errors::Result,
};

/// A [`LinearCoupon`] is a type of [`Cashflow`] that represents a periodic payment.
///
/// In addition to the amount and payment date, a coupon also has an accrual period defined by a start and end date,
/// and can calculate the accrued amount for a given period.
pub trait LinearCoupon<T>: Cashflow<T>
where
    T: IsReal,
{
    /// Returns the accrued amount between two dates.
    ///
    /// ## Errors
    /// Returns an error if the accrued amount cannot be calculated.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<T>;

    /// Returns the accrual start date.
    fn accrual_start_date(&self) -> Date;

    /// Returns the accrual end date.
    fn accrual_end_date(&self) -> Date;

    /// Returns the notional amount associated with this coupon.
    fn notional(&self) -> f64;
}

/// [`PayoffOps`] describes the set of possible mathematical operations that can be used to compute the payoff of a [`NonLinearCoupon`].
#[derive(Clone)]
pub enum PayoffOps {
    /// Max operation.
    Max(Box<Self>, Box<Self>),
    /// Min operation.
    Min(Box<Self>, Box<Self>),
    /// Multiplication operation.
    Times(Box<Self>, Box<Self>),
    /// Addition operation.
    Plus(Box<Self>, Box<Self>),
    /// Substraction operation.
    Minus(Box<Self>, Box<Self>),
    /// Describes a constant value.
    Const(f64),
    /// Describes an index of reference.
    Index,
}

impl PayoffOps {
    /// Evaluates the payoff operation given an index fixing.
    ///
    /// ## Errors
    /// Returns an error if the payoff cannot be evaluated.
    pub fn evaluate(&self, index_fixing: ADReal) -> Result<ADReal> {
        match self {
            Self::Max(left, right) => Ok(left
                .evaluate(index_fixing)?
                .max(right.evaluate(index_fixing)?)
                .into()),
            Self::Min(left, right) => Ok(left
                .evaluate(index_fixing)?
                .min(right.evaluate(index_fixing)?)
                .into()),
            Self::Times(left, right) => {
                Ok((left.evaluate(index_fixing)? * right.evaluate(index_fixing)?).into())
            }
            Self::Plus(left, right) => {
                Ok((left.evaluate(index_fixing)? + right.evaluate(index_fixing)?).into())
            }
            Self::Minus(left, right) => {
                Ok((left.evaluate(index_fixing)? - right.evaluate(index_fixing)?).into())
            }
            Self::Const(value) => Ok(ADReal::new(*value)),
            Self::Index => Ok(index_fixing),
        }
    }
}

/// A [`NonLinearCoupon`] is a coupon that contains some optionality.
pub trait NonLinearCoupon<T> {
    /// Returns the payoff computation tree.
    fn payoff_description(&self) -> PayoffOps;

    /// Returns the accrued amount between two dates.
    ///
    /// # Errors
    /// Returns an error if the accrued amount cannot be calculated.
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<T>;

    /// Returns the accrual start date.
    fn accrual_start_date(&self) -> Date;

    /// Returns the accrual end date.
    fn accrual_end_date(&self) -> Date;

    /// Returns the notional amount associated with this coupon.
    fn notional(&self) -> f64;

    /// Returns the payment date of the coupon.
    fn payment_date(&self) -> Date;
}
