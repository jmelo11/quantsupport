//! Numerical clean/dirty quote conversion, yield solving and duration analytics.
use super::bondpricer::ResolvedCashflow;
use crate::{
    ad::dual::DualFwd,
    instruments::{
        cashflows::{
            cashflowtype::CashflowType,
            coupons::{LinearCoupon, NonLinearCoupon},
            leg::Leg,
        },
        fixedincome::bondpricinganchor::BondPricingAnchor,
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
    time::{date::Date, enums::Frequency},
    utils::errors::{QSError, Result},
};

/// Calculates coupon accrual from the active coupon's accrual start through
/// settlement.
fn accrued_interest(leg: &Leg<DualFwd>, settlement_date: Date) -> Result<f64> {
    let leg: Leg<f64> = leg.clone().into();
    for cashflow in leg.cashflows() {
        let amount = match cashflow {
            CashflowType::FixedRateCoupon(coupon)
                if coupon.accrual_start_date() < settlement_date
                    && settlement_date < coupon.accrual_end_date() =>
            {
                Some(coupon.accrued_amount(coupon.accrual_start_date(), settlement_date)?)
            }
            CashflowType::FloatingRateCoupon(coupon)
                if coupon.accrual_start_date() < settlement_date
                    && settlement_date < coupon.accrual_end_date() =>
            {
                Some(coupon.accrued_amount(coupon.accrual_start_date(), settlement_date)?)
            }
            CashflowType::OptionEmbeddedCoupon(coupon)
                if coupon.accrual_start_date() < settlement_date
                    && settlement_date < coupon.accrual_end_date() =>
            {
                Some(coupon.accrued_amount(coupon.accrual_start_date(), settlement_date)?)
            }
            _ => None,
        };
        if let Some(amount) = amount {
            return Ok(amount);
        }
    }
    Ok(0.0)
}

/// Converts a clean or dirty quote into an optional dirty price per bond
/// units; spread anchors do not provide a target price.
pub(super) fn quoted_dirty_price(
    anchor: BondPricingAnchor,
    leg: &Leg<DualFwd>,
    units: f64,
    settlement_date: Date,
    principal: f64,
) -> Result<Option<f64>> {
    match anchor {
        BondPricingAnchor::CleanPrice(clean) => {
            let accrued_per_unit = accrued_interest(leg, settlement_date)? * units / principal;
            Ok(Some(clean + accrued_per_unit))
        }
        BondPricingAnchor::DirtyPrice(dirty) => Ok(Some(dirty)),
        BondPricingAnchor::ZSpread(_) | BondPricingAnchor::OAS(_) => Ok(None),
    }
}

/// Solves a monotone decreasing bond-price objective by adaptive bracketing
/// followed by bisection.
pub(super) fn bracketed_root(
    mut objective: impl FnMut(f64) -> Result<f64>,
    initial_lower: f64,
    initial_upper: f64,
    hard_lower: f64,
) -> Result<f64> {
    let mut lower = initial_lower.max(hard_lower);
    let mut upper = initial_upper;
    let mut f_lower = objective(lower)?;
    let mut f_upper = objective(upper)?;

    for _ in 0..64 {
        if f_lower >= 0.0 && f_upper <= 0.0 {
            break;
        }
        if f_lower < 0.0 {
            let candidate = 2.0f64.mul_add(lower, -0.01).max(hard_lower);
            if (candidate - lower).abs() < f64::EPSILON {
                break;
            }
            lower = candidate;
            f_lower = objective(lower)?;
        }
        if f_upper > 0.0 {
            upper = 2.0f64.mul_add(upper, 0.01);
            f_upper = objective(upper)?;
        }
    }

    if f_lower.abs() < 1e-12 {
        return Ok(lower);
    }
    if f_upper.abs() < 1e-12 {
        return Ok(upper);
    }
    if !(f_lower >= 0.0 && f_upper <= 0.0) {
        return Err(QSError::SolverErr(format!(
            "Could not bracket bond root: f({lower})={f_lower}, f({upper})={f_upper}"
        )));
    }

    for _ in 0..256 {
        let midpoint = f64::midpoint(lower, upper);
        let f_midpoint = objective(midpoint)?;
        if f_midpoint.abs() < 1e-12 || (upper - lower).abs() < 1e-14 {
            return Ok(midpoint);
        }
        if f_midpoint > 0.0 {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
    }
    Err(QSError::SolverErr(
        "Bond root solve did not converge after 256 iterations".into(),
    ))
}

/// Returns a yield discount factor and its analytic first derivative for one
/// time under the requested compounding convention.
fn yield_discount_and_derivative(
    rate: f64,
    time: f64,
    compounding: Compounding,
    frequency: Frequency,
) -> Result<(f64, f64)> {
    let frequency_value = f64::from(frequency as i32);
    let simple = || {
        let compound = rate.mul_add(time, 1.0);
        if compound <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Yield produces a non-positive compound factor".into(),
            ));
        }
        Ok((1.0 / compound, -time / (compound * compound)))
    };
    let compounded = || {
        let base = rate / frequency_value + 1.0;
        if base <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Yield is outside the compounded-rate domain".into(),
            ));
        }
        let discount = base.powf(-frequency_value * time);
        Ok((discount, -time * discount / base))
    };

    match compounding {
        Compounding::Simple => simple(),
        Compounding::Compounded => compounded(),
        Compounding::Continuous => {
            let discount = (-rate * time).exp();
            Ok((discount, -time * discount))
        }
        Compounding::SimpleThenCompounded => {
            if time <= 1.0 / frequency_value {
                simple()
            } else {
                compounded()
            }
        }
        Compounding::CompoundedThenSimple => {
            if time > 1.0 / frequency_value {
                simple()
            } else {
                compounded()
            }
        }
    }
}

/// Returns dirty yield price and `dP/dy` for all remaining cashflows.
pub(super) fn yield_price_and_derivative(
    cashflows: &[ResolvedCashflow<f64>],
    settlement_date: Date,
    rate_definition: RateDefinition,
    yield_rate: f64,
) -> Result<(f64, f64)> {
    cashflows
        .iter()
        .try_fold((0.0, 0.0), |(price, derivative), cashflow| {
            let time = rate_definition
                .day_counter()
                .year_fraction(settlement_date, cashflow.payment_date);
            let (discount, discount_derivative) = yield_discount_and_derivative(
                yield_rate,
                time,
                rate_definition.compounding(),
                rate_definition.frequency(),
            )?;
            let price = f64::mul_add(cashflow.amount, discount, price);
            let derivative = f64::mul_add(cashflow.amount, discount_derivative, derivative);
            Ok((price, derivative))
        })
}

/// Solves yield to maturity from the model dirty value under the bond's rate
/// definition.
pub(super) fn solve_yield(
    cashflows: &[ResolvedCashflow<f64>],
    settlement_date: Date,
    rate_definition: RateDefinition,
    target_value: f64,
) -> Result<f64> {
    let frequency_value = f64::from(rate_definition.frequency() as i32);
    let hard_lower = match rate_definition.compounding() {
        Compounding::Compounded
        | Compounding::SimpleThenCompounded
        | Compounding::CompoundedThenSimple => -0.999_999 * frequency_value,
        Compounding::Simple | Compounding::Continuous => -10.0,
    };
    bracketed_root(
        |yield_rate| {
            let (price, _) = yield_price_and_derivative(
                cashflows,
                settlement_date,
                rate_definition,
                yield_rate,
            )?;
            Ok(price - target_value)
        },
        -0.05,
        0.10,
        hard_lower,
    )
}
