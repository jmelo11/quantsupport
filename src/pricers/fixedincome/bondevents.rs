//! Contractual cashflows and issuer-call obstacles on an evolution time grid.
use super::bondanalytics::ResolvedCashflow;
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::pricerconfiguration::TimeGridConfiguration,
    instruments::{
        cashflows::{cashflow::Cashflow, cashflowtype::CashflowType, leg::Leg},
        fixedincome::callschedule::CallScheduleEntry,
    },
    time::date::Date,
    utils::errors::{QSError, Result},
};
#[derive(Clone, Copy)]
pub struct CallStep {
    pub call_price: Option<f64>,
}

impl CallStep {
    /// Creates a grid point without an issuer-call obstacle.
    pub const fn new() -> Self {
        Self { call_price: None }
    }
}

pub struct BondEventGrid<T> {
    pub maturity_time: f64,
    pub steps: usize,
    pub cashflows: Vec<T>,
    pub calls: Vec<CallStep>,
}

/// Returns principal cancelled when an issuer call terminates the bond.
fn call_principal(leg: &Leg<DualFwd>, call_date: Date) -> Result<f64> {
    leg.cashflows()
        .iter()
        .filter_map(|cashflow| match cashflow {
            CashflowType::Redemption(redemption) if redemption.payment_date() > call_date => {
                Some(redemption.amount())
            }
            _ => None,
        })
        .try_fold(0.0, |total, amount| amount.map(|value| total + value))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
/// Maps contractual cashflows and issuer calls to the nearest points of a
/// uniform time grid shared by the two callable engines.
pub fn bond_event_grid<T: Scalar>(
    leg: &Leg<DualFwd>,
    units: f64,
    call_schedule: &[CallScheduleEntry],
    cashflows: &[ResolvedCashflow<T>],
    settlement_date: Date,
    controls: TimeGridConfiguration,
) -> Result<BondEventGrid<T>> {
    let maturity_date = cashflows
        .last()
        .ok_or_else(|| QSError::InvalidValueErr("Bond has no remaining cashflows".into()))?
        .payment_date;
    let maturity_time = controls
        .day_counter()
        .year_fraction(settlement_date, maturity_date);
    if maturity_time <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Callable bond maturity must follow settlement".into(),
        ));
    }
    let target_steps = (maturity_time * controls.steps_per_year() as f64).ceil() as usize;
    let steps = target_steps.clamp(1, controls.max_steps());
    let step_for_date = |date: Date, can_be_maturity: bool| {
        let time = controls.day_counter().year_fraction(settlement_date, date);
        let raw = (time / maturity_time * steps as f64).round() as usize;
        if can_be_maturity {
            raw.clamp(1, steps)
        } else {
            raw.clamp(1, steps.saturating_sub(1).max(1))
        }
    };

    let mut step_cashflows = vec![T::zero(); steps + 1];
    for cashflow in cashflows {
        let step = step_for_date(cashflow.payment_date, true);
        step_cashflows[step] = step_cashflows[step].add_val(cashflow.amount);
    }

    let mut calls = vec![CallStep::new(); steps + 1];
    for event in call_schedule
        .iter()
        .filter(|event| event.end_date() >= settlement_date)
    {
        let start_time = controls
            .day_counter()
            .year_fraction(settlement_date, event.date());
        let end_time = controls
            .day_counter()
            .year_fraction(settlement_date, event.end_date());
        let discrete = event.date() == event.end_date();
        for (step, call) in calls.iter_mut().enumerate().take(steps) {
            let time = maturity_time * step as f64 / steps as f64;
            let eligible = if discrete {
                if event.date() == settlement_date {
                    step == 0
                } else {
                    step == step_for_date(event.date(), false)
                }
            } else {
                time + 1.0e-12 >= start_time && time <= end_time + 1.0e-12
            };
            if !eligible {
                continue;
            }
            let principal = if discrete {
                call_principal(leg, event.date())?
            } else {
                leg.cashflows()
                    .iter()
                    .try_fold(0.0, |principal, cashflow| -> Result<f64> {
                        match cashflow {
                            CashflowType::Redemption(redemption)
                                if step_for_date(redemption.payment_date(), true) > step =>
                            {
                                Ok(principal + redemption.amount()?)
                            }
                            _ => Ok(principal),
                        }
                    })?
            };
            if principal <= 0.0 {
                return Err(QSError::InvalidValueErr(format!(
                    "No principal remains at bond exercise step {step}",
                )));
            }
            let amount = event.strike() * principal / units;
            call.call_price = Some(
                call.call_price
                    .map_or(amount, |existing| existing.min(amount)),
            );
        }
    }

    Ok(BondEventGrid {
        maturity_time,
        steps,
        cashflows: step_cashflows,
        calls,
    })
}

#[cfg(test)]
mod tests {
    use super::super::bondanalytics::BondTerms;
    use super::super::{bondpricer::test_support::FACE, treebasedbondpricer::test_support::bond};
    use super::{bond_event_grid, ResolvedCashflow};
    use crate::prelude::*;
    use crate::{
        currencies::currency::Currency,
        instruments::{
            cashflows::makeleg::PaymentStructure,
            fixedincome::{
                callschedule::CallScheduleEntry, makesteppedcouponbond::MakeSteppedCouponBond,
            },
        },
    };

    #[test]
    fn call_windows_cover_interior_grid_points_and_active_settlement() -> Result<()> {
        let start = Date::new(2025, 1, 1);
        let maturity = Date::new(2027, 1, 1);
        let trade = bond(start, maturity, 0.0, PaymentStructure::Zero, vec![]);
        let cashflows = vec![ResolvedCashflow {
            payment_date: maturity,
            amount: FACE,
        }];
        let terms = bond_event_grid(
            trade.bond_leg(),
            FACE,
            &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
            &cashflows,
            start,
            TimeGridConfiguration::new(2, 4, DayCounter::Actual365),
        )?;
        assert_eq!(terms.steps, 4);
        assert_eq!(
            terms
                .calls
                .iter()
                .map(|call| call.call_price)
                .collect::<Vec<_>>(),
            vec![None, Some(95.0), Some(95.0), Some(95.0), None]
        );
        let active = bond_event_grid(
            trade.bond_leg(),
            FACE,
            &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
            &cashflows,
            start + 365,
            TimeGridConfiguration::new(4, 4, DayCounter::Actual365),
        )?;
        assert_eq!(active.calls[0].call_price, Some(95.0));
        let amortizing = MakeSteppedCouponBond::<DualFwd>::default()
            .with_identifier("USD-AMORTIZING-WINDOW".into())
            .with_start_date(start)
            .with_maturity_date(maturity)
            .with_notional(FACE)
            .with_currency(Currency::USD)
            .with_coupons(vec![CashflowType::fixed(0.0)])
            .with_notional_percentages(vec![1.0, 0.8])
            .build()?;
        let amortizing_terms = bond_event_grid(
            amortizing.leg(),
            FACE,
            &[CallScheduleEntry::window(start + 90, start + 640, 95.0)],
            &cashflows,
            start,
            TimeGridConfiguration::new(2, 4, DayCounter::Actual365),
        )?;
        assert_eq!(amortizing_terms.calls[1].call_price, Some(76.0));
        assert_eq!(amortizing_terms.calls[2].call_price, Some(76.0));
        Ok(())
    }
}
