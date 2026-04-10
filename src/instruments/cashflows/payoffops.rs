use crate::ad::dual::DualFwd;
use crate::utils::errors::Result;

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
    pub fn evaluate(&self, index_fixing: DualFwd) -> Result<DualFwd> {
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
            Self::Const(value) => Ok(DualFwd::new(*value)),
            Self::Index => Ok(index_fixing),
        }
    }

    /// Evaluates the payoff operation given an `f64` fixing.
    ///
    /// ## Errors
    /// Returns an error if the payoff cannot be evaluated.
    pub fn evaluate_f64(&self, index_fixing: f64) -> Result<f64> {
        match self {
            Self::Max(left, right) => Ok(left
                .evaluate_f64(index_fixing)?
                .max(right.evaluate_f64(index_fixing)?)),
            Self::Min(left, right) => Ok(left
                .evaluate_f64(index_fixing)?
                .min(right.evaluate_f64(index_fixing)?)),
            Self::Times(left, right) => {
                Ok(left.evaluate_f64(index_fixing)? * right.evaluate_f64(index_fixing)?)
            }
            Self::Plus(left, right) => {
                Ok(left.evaluate_f64(index_fixing)? + right.evaluate_f64(index_fixing)?)
            }
            Self::Minus(left, right) => {
                Ok(left.evaluate_f64(index_fixing)? - right.evaluate_f64(index_fixing)?)
            }
            Self::Const(value) => Ok(*value),
            Self::Index => Ok(index_fixing),
        }
    }
}
