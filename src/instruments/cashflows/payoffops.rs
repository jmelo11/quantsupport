use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use crate::ad::scalar::Scalar;
use crate::utils::errors::Result;

/// [`PayoffOps`] describes the set of possible mathematical operations that can be used to compute the payoff of a [`NonLinearCoupon`](crate::instruments::cashflows::coupons::NonLinearCoupon).
///
/// Equality and hashing preserve expression structure, operand order and the
/// exact bits of constants, including signed zeros and NaN payloads.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PayoffOps {
    /// Max operation.
    Max(Box<Self>, Box<Self>),
    /// Min operation.
    Min(Box<Self>, Box<Self>),
    /// Multiplication operation.
    Times(Box<Self>, Box<Self>),
    /// Addition operation.
    Plus(Box<Self>, Box<Self>),
    /// Subtraction operation.
    Minus(Box<Self>, Box<Self>),
    /// Describes a constant value.
    Const(f64),
    /// Describes an index of reference.
    Index,
}

impl PartialEq for PayoffOps {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Max(a, b), Self::Max(c, d))
            | (Self::Min(a, b), Self::Min(c, d))
            | (Self::Times(a, b), Self::Times(c, d))
            | (Self::Plus(a, b), Self::Plus(c, d))
            | (Self::Minus(a, b), Self::Minus(c, d)) => a == c && b == d,
            (Self::Const(a), Self::Const(b)) => a.to_bits() == b.to_bits(),
            (Self::Index, Self::Index) => true,
            _ => false,
        }
    }
}

impl Eq for PayoffOps {}

impl Hash for PayoffOps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Max(left, right)
            | Self::Min(left, right)
            | Self::Times(left, right)
            | Self::Plus(left, right)
            | Self::Minus(left, right) => {
                left.hash(state);
                right.hash(state);
            }
            Self::Const(value) => value.to_bits().hash(state),
            Self::Index => {}
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn payoff_keys_distinguish_operators_and_operand_order() {
        let left = Box::new(PayoffOps::Index);
        let right = Box::new(PayoffOps::Const(0.04));
        let payoffs = [
            PayoffOps::Max(left.clone(), right.clone()),
            PayoffOps::Min(left.clone(), right.clone()),
            PayoffOps::Times(left.clone(), right.clone()),
            PayoffOps::Plus(left.clone(), right.clone()),
            PayoffOps::Minus(left.clone(), right.clone()),
            PayoffOps::Const(0.04),
            PayoffOps::Index,
        ];
        let keys: HashSet<_> = payoffs.iter().cloned().collect();
        assert_eq!(keys.len(), payoffs.len());
        for payoff in &payoffs {
            assert!(keys.contains(&payoff.clone()));
        }
        assert!(!keys.contains(&PayoffOps::Minus(right, left)));
    }

    #[test]
    fn payoff_keys_preserve_signed_zero_and_nan_payloads() {
        let payoffs = [
            PayoffOps::Const(0.0),
            PayoffOps::Const(-0.0),
            PayoffOps::Const(f64::from_bits(0x7ff8_0000_0000_0001)),
            PayoffOps::Const(f64::from_bits(0x7ff8_0000_0000_0002)),
        ];
        let keys: HashSet<_> = payoffs.iter().cloned().collect();
        assert_eq!(keys.len(), payoffs.len());
        for payoff in payoffs {
            assert_eq!(payoff, payoff.clone());
            assert!(keys.contains(&payoff));
        }
    }
}
