//! Preserve coupon economics while compressing compatible cashflows.

use crate::math::compensatedsum::CompensatedSum;
use crate::instruments::cashflows::{
    optionembeddedcoupon::OptionEmbeddedCoupon, payoffops::PayoffOps,
};
use crate::{prelude::*, utils::errors::Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Owned numeric contractual terms of a reported coupon.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CouponDetails {
    /// Contractual accrual start date.
    pub accrual_start: Date,
    /// Contractual accrual end date.
    pub accrual_end: Date,
    /// Contractual floating index; absent for fixed coupons.
    pub market_index: Option<MarketIndex>,
    /// Leg-level forecasting/fixing index override, when supplied.
    #[serde(default)]
    pub forward_index: Option<MarketIndex>,
    /// Signed contractual principal, including the position/leg direction.
    pub notional: f64,
    /// Annual decimal fixed coupon rate, when fixed.
    pub rate: Option<f64>,
    /// Annual decimal spread over the floating index, when floating.
    pub spread: Option<f64>,
    /// Contractual accrual day count.
    pub day_counter: DayCounter,
    /// Original fixed-rate conventions.
    pub rate_definition: Option<RateDefinition>,
    /// Overnight observation, averaging and spread-compounding rules.
    pub overnight_conventions: Option<OvernightCouponConventions>,
    /// Original option payoff; distinct payoffs remain separate.
    pub payoff: Option<PayoffOps>,
}

impl CouponDetails {
    /// Returns the leg forecasting index, falling back to the contractual index.
    #[must_use]
    pub fn fixing_index(&self) -> Option<&MarketIndex> {
        self.forward_index.as_ref().or(self.market_index.as_ref())
    }

    /// Reconstructs an equivalent numeric coupon from reported contractual terms.
    /// The supplied fixing may be replaced when repricing at another market date.
    ///
    /// # Errors
    /// Rejects incomplete terms or invalid overnight conventions.
    pub fn to_coupon(&self, payment_date: Date, fixing: Option<f64>) -> Result<CashflowType<f64>> {
        if let Some(index) = &self.market_index {
            if let Some(payoff) = &self.payoff {
                let mut coupon = OptionEmbeddedCoupon::new(
                    self.notional,
                    index.clone(),
                    self.spread.unwrap_or(0.0),
                    self.accrual_start,
                    self.accrual_end,
                    payment_date,
                    payoff.clone(),
                )
                .with_day_counter(self.day_counter);
                if let Some(fixing) = fixing {
                    coupon.set_fixing(fixing);
                }
                return Ok(CashflowType::OptionEmbeddedCoupon(coupon));
            }
            let mut coupon = FloatingRateCoupon::new(
                self.notional,
                self.spread.unwrap_or(0.0),
                index.clone(),
                self.accrual_start,
                self.accrual_end,
                payment_date,
            )
            .with_day_counter(self.day_counter);
            if let Some(conventions) = &self.overnight_conventions {
                coupon.set_overnight_conventions(conventions.clone())?;
            }
            if let Some(fixing) = fixing {
                coupon.set_fixing(fixing);
            }
            Ok(CashflowType::FloatingRateCoupon(coupon))
        } else {
            let rate = self
                .rate
                .ok_or_else(|| QSError::ValueNotSetErr("Fixed coupon rate".into()))?;
            let definition = self
                .rate_definition
                .ok_or_else(|| QSError::ValueNotSetErr("Fixed coupon rate definition".into()))?;
            Ok(CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                self.notional,
                Box::new(InterestRate::from_rate_definition(rate, definition)),
                self.accrual_start,
                self.accrual_end,
                payment_date,
            )))
        }
    }
}

struct Group {
    kind: String,
    date: Date,
    currency: Currency,
    fixing: Option<f64>,
    accrual: f64,
    floor: Option<f64>,
    cap: Option<f64>,
    leg: usize,
    details: Option<CouponDetails>,
    amount: CompensatedSum,
    notional: CompensatedSum,
    rate_moment: CompensatedSum,
    spread_moment: CompensatedSum,
    average_rate: bool,
    average_spread: bool,
}

/// Incremental cashflow grouping; all retained values are owned numeric data.
#[derive(Default)]
pub(crate) struct CashflowCompressor {
    groups: BTreeMap<(Date, String, String, String), Group>,
    serial: usize,
}

impl CashflowCompressor {
    pub(crate) fn add(&mut self, flows: &CashflowsTable) -> Result<()> {
        for row in 0..flows.payment_dates().len() {
            let kind = &flows.cashflow_types()[row];
            let details = flows.coupon_details()[row].clone();
            let average_rate = kind == "FixedRateCoupon"
                && details.as_ref().is_some_and(|terms| terms.rate.is_some());
            let average_spread = kind == "FloatingRateCoupon"
                && details.as_ref().is_some_and(|terms| {
                    !terms
                        .overnight_conventions
                        .as_ref()
                        .is_some_and(OvernightCouponConventions::compound_spread_daily)
                });
            let identity = if let Some(terms) = &details {
                let mut identity = terms.clone();
                // Keep payment/receipt directions separate, avoiding zero-net-notional synthetic coupons.
                identity.notional = terms.notional.signum();
                if average_rate {
                    identity.rate = None;
                }
                if average_spread {
                    identity.spread = None;
                }
                format!(
                    "coupon:{identity:?}:{:?}:{:?}:{:?}:{}",
                    flows.fixing()[row].map(f64::to_bits),
                    flows.floorlet_strikes()[row].map(f64::to_bits),
                    flows.caplet_strikes()[row].map(f64::to_bits),
                    flows.accrual_periods()[row].to_bits()
                )
            } else if matches!(
                kind.as_str(),
                "Redemption" | "Disbursement" | "ConstantAmount"
            ) {
                format!("capital:{}", flows.amounts()[row].is_sign_negative())
            } else {
                // Missing contractual terms must never cause unrelated floating coupons to be merged.
                self.serial += 1;
                format!("unclassified:{}", self.serial)
            };
            let identity = format!("{identity}:leg{}", flows.leg_indices()[row]);
            let group = self
                .groups
                .entry((
                    flows.payment_dates()[row],
                    flows.currencies()[row].to_string(),
                    kind.clone(),
                    identity,
                ))
                .or_insert_with(|| Group {
                    kind: kind.clone(),
                    date: flows.payment_dates()[row],
                    currency: flows.currencies()[row],
                    fixing: flows.fixing()[row],
                    accrual: flows.accrual_periods()[row],
                    floor: flows.floorlet_strikes()[row],
                    cap: flows.caplet_strikes()[row],
                    leg: flows.leg_indices()[row],
                    details: details.clone(),
                    amount: CompensatedSum::default(),
                    notional: CompensatedSum::default(),
                    rate_moment: CompensatedSum::default(),
                    spread_moment: CompensatedSum::default(),
                    average_rate,
                    average_spread,
                });
            group.amount.add(flows.amounts()[row])?;
            if let Some(terms) = details {
                group.notional.add(terms.notional)?;
                if average_rate {
                    group
                        .rate_moment
                        .add(terms.notional * terms.rate.unwrap_or(0.0))?;
                }
                if average_spread {
                    group
                        .spread_moment
                        .add(terms.notional * terms.spread.unwrap_or(0.0))?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> CashflowsTable {
        let mut result = CashflowsTable::new();
        for (_, mut group) in self.groups {
            result.add_cashflow(
                group.date,
                group.kind,
                group.amount.total(),
                group.fixing,
                group.accrual,
                group.currency,
                group.floor,
                group.cap,
                group.leg,
            );
            if let Some(details) = &mut group.details {
                details.notional = group.notional.total();
                if details.notional != 0.0 {
                    if group.average_rate {
                        details.rate = Some(group.rate_moment.total() / details.notional);
                    }
                    if group.average_spread {
                        details.spread = Some(group.spread_moment.total() / details.notional);
                    }
                }
                result.set_last_coupon_details(details.clone());
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(index: Option<MarketIndex>, notional: f64, quote: f64) -> CouponDetails {
        let floating = index.is_some();
        CouponDetails {
            accrual_start: Date::new(2026, 1, 15),
            accrual_end: Date::new(2027, 1, 15),
            market_index: index,
            forward_index: None,
            notional,
            rate: (!floating).then_some(quote),
            spread: floating.then_some(quote),
            day_counter: DayCounter::Actual360,
            rate_definition: (!floating).then_some(RateDefinition::default()),
            overnight_conventions: None,
            payoff: None,
        }
    }

    fn add(flows: &mut CashflowsTable, details: CouponDetails, fixing: Option<f64>) -> Result<()> {
        let payment = Date::new(2027, 1, 17);
        let coupon = details.to_coupon(payment, fixing)?;
        let (kind, amount) = match coupon {
            CashflowType::FixedRateCoupon(coupon) => ("FixedRateCoupon", coupon.amount()?),
            CashflowType::FloatingRateCoupon(coupon) => ("FloatingRateCoupon", coupon.amount()?),
            CashflowType::OptionEmbeddedCoupon(coupon) => {
                ("OptionEmbeddedCoupon", coupon.amount()?)
            }
            _ => unreachable!(),
        };
        flows.add_cashflow(
            payment,
            kind.into(),
            amount,
            fixing,
            details
                .day_counter
                .year_fraction(details.accrual_start, details.accrual_end),
            Currency::USD,
            None,
            None,
            0,
        );
        flows.set_last_coupon_details(details);
        Ok(())
    }

    #[test]
    fn weighted_coupons_preserve_types_indices_amounts_and_reprice_from_the_same_index(
    ) -> Result<()> {
        let mut flows = CashflowsTable::new();
        add(&mut flows, terms(None, 100.0, 0.04), None)?;
        add(&mut flows, terms(None, 300.0, 0.06), None)?;
        add(
            &mut flows,
            terms(Some(MarketIndex::SOFR), 100.0, 0.001),
            Some(0.03),
        )?;
        add(
            &mut flows,
            terms(Some(MarketIndex::SOFR), 300.0, 0.003),
            Some(0.03),
        )?;
        add(
            &mut flows,
            terms(Some(MarketIndex::ESTR), 100.0, 0.002),
            Some(0.03),
        )?;
        let mut compressor = CashflowCompressor::default();
        compressor.add(&flows)?;
        let result = compressor.finish();
        assert_eq!(result.payment_dates().len(), 3);
        let mut repriced = 0.0;
        for (row, terms) in result.coupon_details().iter().enumerate() {
            let terms = terms
                .as_ref()
                .ok_or_else(|| QSError::ValueNotSetErr("coupon terms".into()))?;
            let amount = match terms.to_coupon(result.payment_dates()[row], result.fixing()[row])? {
                CashflowType::FixedRateCoupon(coupon) => {
                    assert_eq!(terms.notional, 400.0);
                    assert!((terms.rate.unwrap_or(0.0) - 0.055).abs() < 1e-14);
                    coupon.amount()?
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    if terms.market_index == Some(MarketIndex::SOFR) {
                        assert_eq!(terms.notional, 400.0);
                        assert!((terms.spread.unwrap_or(0.0) - 0.0025).abs() < 1e-14);
                    }
                    let amount = coupon.amount()?;
                    coupon.set_fixing(0.05);
                    repriced += coupon.amount()?;
                    amount
                }
                _ => unreachable!(),
            };
            assert!((amount - result.amounts()[row]).abs() < 1e-12);
        }
        let accrual =
            DayCounter::Actual360.year_fraction(Date::new(2026, 1, 15), Date::new(2027, 1, 15));
        assert!(
            (repriced - ((0.05 + 0.0025) * 400.0 + (0.05 + 0.002) * 100.0) * accrual).abs() < 1e-12
        );
        Ok(())
    }

    #[test]
    fn different_periods_conventions_directions_and_payoffs_do_not_merge() -> Result<()> {
        let mut flows = CashflowsTable::new();
        let base = terms(Some(MarketIndex::SOFR), 100.0, 0.001);
        add(&mut flows, base.clone(), Some(0.03))?;
        let mut changed = base.clone();
        changed.accrual_start = Date::new(2026, 2, 15);
        add(&mut flows, changed, Some(0.03))?;
        let mut changed = base.clone();
        changed.notional = -100.0;
        add(&mut flows, changed, Some(0.03))?;
        let mut changed = base.clone();
        changed.day_counter = DayCounter::Actual365;
        add(&mut flows, changed, Some(0.03))?;
        let mut changed = base.clone();
        changed.forward_index = Some(MarketIndex::ESTR);
        assert_eq!(changed.fixing_index(), Some(&MarketIndex::ESTR));
        add(&mut flows, changed, Some(0.03))?;
        let mut changed = base.clone();
        changed.overnight_conventions =
            Some(OvernightCouponConventions::new().with_lookback_days(2));
        add(&mut flows, changed, Some(0.03))?;
        for cap in [0.04, 0.06] {
            let mut changed = base.clone();
            changed.payoff = Some(PayoffOps::Min(
                Box::new(PayoffOps::Index),
                Box::new(PayoffOps::Const(cap)),
            ));
            add(&mut flows, changed, Some(0.05))?;
        }
        let mut compressor = CashflowCompressor::default();
        compressor.add(&flows)?;
        assert_eq!(compressor.finish().payment_dates().len(), 8);
        Ok(())
    }

    #[test]
    fn daily_compounded_spreads_and_unknown_coupon_terms_are_never_averaged_together() -> Result<()>
    {
        let mut flows = CashflowsTable::new();
        for spread in [0.001, 0.003] {
            let mut coupon = terms(Some(MarketIndex::SOFR), 100.0, spread);
            coupon.overnight_conventions =
                Some(OvernightCouponConventions::new().with_compound_spread_daily(true));
            add(&mut flows, coupon, Some(0.03))?;
        }
        for _ in 0..2 {
            flows.add_cashflow(
                Date::new(2027, 1, 17),
                "FloatingRateCoupon".into(),
                3.0,
                Some(0.03),
                1.0,
                Currency::USD,
                None,
                None,
                0,
            );
        }
        let mut compressor = CashflowCompressor::default();
        compressor.add(&flows)?;
        assert_eq!(compressor.finish().payment_dates().len(), 4);
        Ok(())
    }
}
