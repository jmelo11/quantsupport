use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    indices::marketindex::MarketIndex,
    instruments::cashflows::{
        cashflow::SimpleCashflow, fixedratecoupon::FixedRateCoupon,
        floatingratecoupon::FloatingRateCoupon, optionembeddedcashflow::OptionEmbeddedCashflow,
        optionembeddedcoupon::OptionEmbeddedCoupon, payoffops::PayoffOps,
    },
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
    },
    time::{date::Date, daycounter::DayCounter, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// An enumeration representing different types of cash flows that can occur in financial instruments.
#[derive(Clone)]
pub enum CashflowType<T: Scalar> {
    /// A fixed rate coupon, where the cash flow is determined by a fixed interest rate applied to the notional amount.
    FixedRateCoupon(FixedRateCoupon<T>),
    /// A floating rate coupon, where the cash flow is determined by a variable interest rate (often linked to an index) applied to the notional amount.
    FloatingRateCoupon(FloatingRateCoupon<T>),
    /// An option-embedded coupon, where the cash flow is determined by a payoff function that may include optionality features (e.g., caps, floors).
    OptionEmbeddedCoupon(OptionEmbeddedCoupon<T>),
    /// A simple cash flow representing a redemption of the notional amount at a given time.
    Redemption(SimpleCashflow<f64>),
    /// A simple cash flow representing a disbursement of the notional amount at the start of the instrument.
    Disbursement(SimpleCashflow<f64>),
    /// Constant amount
    ConstantAmount(SimpleCashflow<f64>),
    /// An option-embedded cash flow, where the cash flow is determined by a payoff function that may include optionality features (e.g., caps, floors).
    OptionEmbeddedCashflow(OptionEmbeddedCashflow<T>),
}

impl<T: Scalar> CashflowType<T> {
    /// Creates a fixed coupon template for a schedule-generating builder.
    ///
    /// The builder supplies the notional and dates. The default convention is
    /// simple annual interest with Actual/365 accrual.
    #[must_use]
    pub fn fixed(rate: f64) -> Self {
        let date = Date::new(1970, 1, 1);
        Self::FixedRateCoupon(FixedRateCoupon::new(
            1.0,
            Box::new(InterestRate::from_rate_definition(
                T::scalar(rate),
                RateDefinition::new(
                    DayCounter::Actual365,
                    Compounding::Simple,
                    Frequency::Annual,
                ),
            )),
            date,
            date,
            date,
        ))
    }

    /// Creates a floating coupon template, with optional all-in rate bounds.
    ///
    /// The builder supplies notional and dates. Bounded rates are represented
    /// by the existing option-embedded coupon and payoff operations.
    #[must_use]
    pub fn floating(index: MarketIndex, spread: f64, floor: Option<f64>, cap: Option<f64>) -> Self {
        let date = Date::new(1970, 1, 1);
        let capped = cap.map_or(PayoffOps::Index, |upper| {
            PayoffOps::Min(
                Box::new(PayoffOps::Index),
                Box::new(PayoffOps::Const(upper - spread)),
            )
        });
        let payoff = floor.map_or_else(
            || capped.clone(),
            |lower| {
                PayoffOps::Max(
                    Box::new(capped.clone()),
                    Box::new(PayoffOps::Const(lower - spread)),
                )
            },
        );
        if floor.is_none() && cap.is_none() {
            Self::FloatingRateCoupon(FloatingRateCoupon::new(
                1.0,
                T::scalar(spread),
                index,
                date,
                date,
                date,
            ))
        } else {
            Self::OptionEmbeddedCoupon(OptionEmbeddedCoupon::new(
                1.0,
                index,
                T::scalar(spread),
                date,
                date,
                date,
                payoff,
            ))
        }
    }

    /// Validates that this cashflow can serve as a coupon template.
    ///
    /// # Errors
    /// Rejects non-coupon variants, non-finite rates, and invalid payoff bounds.
    pub fn validate_coupon(&self) -> Result<()> {
        let rate = match self {
            Self::FixedRateCoupon(coupon) => coupon.rate().rate().value(),
            Self::FloatingRateCoupon(coupon) => coupon.spread().value(),
            Self::OptionEmbeddedCoupon(coupon) => {
                validate_coupon_payoff(coupon.payoff_ops())?;
                coupon.spread().value()
            }
            _ => {
                return Err(QSError::InvalidValueErr(
                    "Coupon schedules require coupon cashflows".into(),
                ))
            }
        };
        if !rate.is_finite() {
            return Err(QSError::InvalidValueErr(
                "Coupon rates and spreads must be finite".into(),
            ));
        }
        Ok(())
    }

    /// Rebinds a coupon template to a generated accrual period and notional.
    ///
    /// Fixed rate conventions are preserved unless explicitly overridden.
    /// Fixings are deliberately cleared for the new accrual period.
    ///
    /// # Errors
    /// Rejects invalid coupon templates, notionals, or accrual dates.
    pub fn for_period(
        &self,
        notional: f64,
        start: Date,
        end: Date,
        payment: Date,
        fixed_definition: Option<RateDefinition>,
    ) -> Result<Self> {
        self.validate_coupon()?;
        if !notional.is_finite() || notional <= 0.0 || end <= start || payment < end {
            return Err(QSError::InvalidValueErr(
                "Invalid coupon notional or accrual period".into(),
            ));
        }
        Ok(match self {
            Self::FixedRateCoupon(coupon) => Self::FixedRateCoupon(FixedRateCoupon::new(
                notional,
                Box::new(fixed_definition.map_or_else(
                    || *coupon.rate(),
                    |definition| {
                        InterestRate::from_rate_definition(coupon.rate().rate(), definition)
                    },
                )),
                start,
                end,
                payment,
            )),
            Self::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(FloatingRateCoupon::new(
                notional,
                coupon.spread(),
                coupon.market_index().clone(),
                start,
                end,
                payment,
            )),
            Self::OptionEmbeddedCoupon(coupon) => {
                Self::OptionEmbeddedCoupon(OptionEmbeddedCoupon::new(
                    notional,
                    coupon.market_index().clone(),
                    coupon.spread(),
                    start,
                    end,
                    payment,
                    coupon.payoff_ops().clone(),
                ))
            }
            _ => {
                return Err(QSError::InvalidValueErr(
                    "Expected a coupon cashflow".into(),
                ))
            }
        })
    }
}

fn validate_coupon_payoff(payoff: &PayoffOps) -> Result<()> {
    match payoff {
        PayoffOps::Const(value) if !value.is_finite() => Err(QSError::InvalidValueErr(
            "Coupon payoff bounds must be finite".into(),
        )),
        PayoffOps::Max(inner, lower) => {
            if let (PayoffOps::Min(_, upper), PayoffOps::Const(floor)) = (&**inner, &**lower) {
                if let PayoffOps::Const(cap) = &**upper {
                    if floor > cap {
                        return Err(QSError::InvalidValueErr(
                            "Coupon floor cannot exceed cap".into(),
                        ));
                    }
                }
            }
            validate_coupon_payoff(inner)?;
            validate_coupon_payoff(lower)
        }
        PayoffOps::Min(left, right)
        | PayoffOps::Times(left, right)
        | PayoffOps::Plus(left, right)
        | PayoffOps::Minus(left, right) => {
            validate_coupon_payoff(left)?;
            validate_coupon_payoff(right)
        }
        _ => Ok(()),
    }
}

pub(crate) fn fixed_cashflow<T: Scalar>(
    notional: f64,
    rate: InterestRate<T>,
    start: Date,
    end: Date,
    payment: Date,
) -> Result<CashflowType<T>> {
    let coupon = CashflowType::FixedRateCoupon(FixedRateCoupon::new(
        notional,
        Box::new(rate),
        start,
        end,
        payment,
    ));
    coupon.validate_coupon()?;
    Ok(coupon)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn floating_cashflow<T: Scalar>(
    notional: f64,
    index: MarketIndex,
    spread: T,
    floor: Option<f64>,
    cap: Option<f64>,
    start: Date,
    end: Date,
    payment: Date,
) -> Result<CashflowType<T>> {
    let template = CashflowType::<T>::floating(index.clone(), spread.value(), floor, cap);
    template.validate_coupon()?;
    Ok(match template {
        CashflowType::FloatingRateCoupon(_) => CashflowType::FloatingRateCoupon(
            FloatingRateCoupon::new(notional, spread, index, start, end, payment),
        ),
        CashflowType::OptionEmbeddedCoupon(coupon) => {
            CashflowType::OptionEmbeddedCoupon(OptionEmbeddedCoupon::new(
                notional,
                index,
                spread,
                start,
                end,
                payment,
                coupon.payoff_ops().clone(),
            ))
        }
        _ => {
            return Err(QSError::UnexpectedErr(
                "Floating template is not a coupon".into(),
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupon_templates_use_existing_cashflow_variants() -> Result<()> {
        let coupon = CashflowType::<f64>::floating(MarketIndex::SOFR, 0.01, Some(0.02), Some(0.05));
        let start = Date::new(2026, 1, 1);
        let end = Date::new(2027, 1, 1);
        let CashflowType::OptionEmbeddedCoupon(mut rebound) =
            coupon.for_period(100.0, start, end, end, None)?
        else {
            panic!("Expected the existing option-embedded coupon");
        };
        rebound.set_fixing(0.08);
        assert!((rebound.amount()? - 100.0 * 0.05 * 365.0 / 360.0).abs() < 1.0e-12);
        assert!(CashflowType::<f64>::fixed(f64::NAN)
            .validate_coupon()
            .is_err());
        assert!(
            CashflowType::<f64>::floating(MarketIndex::SOFR, 0.0, Some(0.06), Some(0.02))
                .validate_coupon()
                .is_err()
        );
        assert!(
            CashflowType::<f64>::Redemption(SimpleCashflow::new(100.0, end))
                .validate_coupon()
                .is_err()
        );
        Ok(())
    }
}

impl From<CashflowType<f64>> for CashflowType<DualFwd> {
    fn from(value: CashflowType<f64>) -> Self {
        match value {
            CashflowType::FixedRateCoupon(coupon) => Self::FixedRateCoupon(coupon.into()),
            CashflowType::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(coupon.into()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Self::OptionEmbeddedCoupon(coupon.into()),
            CashflowType::Redemption(cashflow) => Self::Redemption(cashflow),
            CashflowType::Disbursement(cashflow) => Self::Disbursement(cashflow),
            CashflowType::ConstantAmount(cashflow) => Self::ConstantAmount(cashflow),
            CashflowType::OptionEmbeddedCashflow(cashflow) => {
                Self::OptionEmbeddedCashflow(cashflow.into())
            }
        }
    }
}

impl From<CashflowType<DualFwd>> for CashflowType<f64> {
    fn from(value: CashflowType<DualFwd>) -> Self {
        match value {
            CashflowType::FixedRateCoupon(coupon) => Self::FixedRateCoupon(coupon.into()),
            CashflowType::FloatingRateCoupon(coupon) => Self::FloatingRateCoupon(coupon.into()),
            CashflowType::OptionEmbeddedCoupon(coupon) => Self::OptionEmbeddedCoupon(coupon.into()),
            CashflowType::Redemption(cashflow) => Self::Redemption(cashflow),
            CashflowType::Disbursement(cashflow) => Self::Disbursement(cashflow),
            CashflowType::ConstantAmount(cashflow) => Self::ConstantAmount(cashflow),
            CashflowType::OptionEmbeddedCashflow(cashflow) => {
                Self::OptionEmbeddedCashflow(cashflow.into())
            }
        }
    }
}
