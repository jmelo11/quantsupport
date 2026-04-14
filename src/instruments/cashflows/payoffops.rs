use crate::ad::scalar::Scalar;
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
    pub fn eval<T: Scalar>(&self, index_fixing: T) -> Result<T> {
        match self {
            Self::Max(left, right) => {
                Ok(left.eval(index_fixing)?.max_val(right.eval(index_fixing)?))
            }
            Self::Min(left, right) => {
                Ok(left.eval(index_fixing)?.min_val(right.eval(index_fixing)?))
            }
            Self::Times(left, right) => {
                Ok(left.eval(index_fixing)?.mul_val(right.eval(index_fixing)?))
            }
            Self::Plus(left, right) => {
                Ok(left.eval(index_fixing)?.add_val(right.eval(index_fixing)?))
            }
            Self::Minus(left, right) => {
                Ok(left.eval(index_fixing)?.sub_val(right.eval(index_fixing)?))
            }
            Self::Const(value) => Ok(T::scalar(*value)),
            Self::Index => Ok(index_fixing),
        }
    }
}
