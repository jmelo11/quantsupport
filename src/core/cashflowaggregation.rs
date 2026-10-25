//! Preserve coupon economics while compressing compatible cashflows.

use crate::instruments::cashflows::{
    optionembeddedcoupon::OptionEmbeddedCoupon, payoffops::PayoffOps,
};
use crate::math::compensatedsum::CompensatedSum;
use crate::{prelude::*, utils::errors::Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Owned numeric contractual terms of a reported coupon.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CouponDetails {
    /// Contractual accrual start date.
    accrual_start: Date,
    /// Contractual accrual end date.
    accrual_end: Date,
    /// Contractual floating index; absent for fixed coupons.
    market_index: Option<MarketIndex>,
    /// Leg-level forecasting/fixing index override, when supplied.
    #[serde(default)]
    forward_index: Option<MarketIndex>,
    /// Signed contractual principal, including the position/leg direction.
    notional: f64,
    /// Annual decimal fixed coupon rate, when fixed.
    rate: Option<f64>,
    /// Annual decimal spread over the floating index, when floating.
    spread: Option<f64>,
    /// Contractual accrual day count.
    day_counter: DayCounter,
    /// Original fixed-rate conventions.
    rate_definition: Option<RateDefinition>,
    /// Overnight observation, averaging and spread-compounding rules.
    overnight_conventions: Option<OvernightCouponConventions>,
    /// Original option payoff; distinct payoffs remain separate.
    payoff: Option<PayoffOps>,
}

impl CouponDetails {
    /// Starts contractual metadata for one accrual period and signed principal.
    #[must_use]
    pub const fn new(
        accrual_start: Date,
        accrual_end: Date,
        notional: f64,
        day_counter: DayCounter,
    ) -> Self {
        Self {
            accrual_start,
            accrual_end,
            market_index: None,
            forward_index: None,
            notional,
            rate: None,
            spread: None,
            day_counter,
            rate_definition: None,
            overnight_conventions: None,
            payoff: None,
        }
    }

    /// Sets a fixed coupon quote and its contractual day-count definition.
    #[must_use]
    pub const fn with_fixed_rate(mut self, rate: InterestRate<f64>) -> Self {
        self.rate = Some(rate.rate());
        self.rate_definition = Some(rate.rate_definition());
        self.day_counter = rate.day_counter();
        self
    }

    /// Sets the contractual floating index and spread.
    #[must_use]
    pub fn with_floating_rate(mut self, index: MarketIndex, spread: f64) -> Self {
        self.market_index = Some(index);
        self.spread = Some(spread);
        self
    }

    /// Sets the optional forecasting/fixing index override.
    #[must_use]
    pub fn with_forward_index(mut self, index: Option<MarketIndex>) -> Self {
        self.forward_index = index;
        self
    }

    /// Sets the optional overnight observation and spread conventions.
    #[must_use]
    pub const fn with_overnight_conventions(
        mut self,
        conventions: Option<OvernightCouponConventions>,
    ) -> Self {
        self.overnight_conventions = conventions;
        self
    }

    /// Sets an option payoff on the floating index.
    #[must_use]
    pub fn with_payoff(mut self, payoff: PayoffOps) -> Self {
        self.payoff = Some(payoff);
        self
    }

    /// Computes an annual rate from reported interest and a supplied convention.
    ///
    /// The compound factor is `1 + interest / notional`; the convention's day
    /// counter measures this coupon's accrual dates. Contractual quotes are unchanged.
    ///
    /// # Errors
    /// Rejects zero/nonfinite principal, invalid dates, nonpositive growth,
    /// nonfinite interest or an invalid compounding frequency.
    pub fn implied_rate(
        &self,
        interest: f64,
        convention: RateDefinition,
    ) -> Result<InterestRate<f64>> {
        let time = convention
            .day_counter()
            .year_fraction(self.accrual_start, self.accrual_end);
        let compound = 1.0 + interest / self.notional;
        let uses_frequency = matches!(
            convention.compounding(),
            Compounding::Compounded
                | Compounding::SimpleThenCompounded
                | Compounding::CompoundedThenSimple
        );
        if !self.notional.is_finite()
            || self.notional == 0.0
            || !interest.is_finite()
            || !time.is_finite()
            || time <= 0.0
            || !compound.is_finite()
            || compound <= 0.0
            || (uses_frequency
                && matches!(
                    convention.frequency(),
                    Frequency::NoFrequency | Frequency::Once | Frequency::OtherFrequency
                ))
        {
            return Err(QSError::InvalidValueErr(
                "Invalid implied coupon rate inputs".into(),
            ));
        }
        let rate = InterestRate::<f64>::implied_rate(
            compound,
            convention.day_counter(),
            convention.compounding(),
            convention.frequency(),
            time,
        )?;
        if !rate.rate().is_finite() {
            return Err(QSError::InvalidValueErr(
                "Nonfinite implied coupon rate".into(),
            ));
        }
        Ok(rate)
    }

    /// Returns the contractual accrual start.
    #[must_use]
    pub const fn accrual_start(&self) -> Date {
        self.accrual_start
    }

    /// Returns the contractual accrual end.
    #[must_use]
    pub const fn accrual_end(&self) -> Date {
        self.accrual_end
    }

    /// Returns the signed contractual principal.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Returns the fixed coupon quote.
    #[must_use]
    pub const fn rate(&self) -> Option<f64> {
        self.rate
    }

    /// Returns the floating coupon spread.
    #[must_use]
    pub const fn spread(&self) -> Option<f64> {
        self.spread
    }

    /// Returns the contractual day count.
    #[must_use]
    pub const fn day_counter(&self) -> DayCounter {
        self.day_counter
    }

    /// Returns the original fixed rate convention.
    #[must_use]
    pub const fn rate_definition(&self) -> Option<RateDefinition> {
        self.rate_definition
    }

    /// Returns the contractual floating index, when present.
    #[must_use]
    pub const fn market_index(&self) -> Option<&MarketIndex> {
        self.market_index.as_ref()
    }

    /// Returns the forecasting index override, when present.
    #[must_use]
    pub const fn forward_index(&self) -> Option<&MarketIndex> {
        self.forward_index.as_ref()
    }

    /// Returns the overnight observation conventions, when present.
    #[must_use]
    pub const fn overnight_conventions(&self) -> Option<&OvernightCouponConventions> {
        self.overnight_conventions.as_ref()
    }

    /// Returns the option payoff, when present.
    #[must_use]
    pub const fn payoff(&self) -> Option<&PayoffOps> {
        self.payoff.as_ref()
    }

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
            let coupon = FixedRateCoupon::new(
                self.notional,
                Box::new(InterestRate::from_rate_definition(rate, definition)),
                self.accrual_start,
                self.accrual_end,
                payment_date,
            );
            Ok(CashflowType::FixedRateCoupon(coupon))
        }
    }
}

/// Injectable consumer of completed numeric cashflow tables.
///
/// Implementations own their state and must not retain an AD graph.
pub trait CashflowAggregator {
    /// Adds the existing cashflow table from one evaluation result.
    ///
    /// # Errors
    /// Rejects invalid input or aggregation overflow. Discard the strategy on failure.
    fn accumulate_cashflows(&mut self, flows: &CashflowsTable) -> Result<()>;

    /// Produces a standard cashflow table using this strategy's grouping rules.
    ///
    /// # Errors
    /// Returns an error if equivalent coupon terms cannot be computed.
    fn into_cashflows_table(self) -> Result<CashflowsTable>;
}

/// Preserves separate payment and receipt positions while merging compatible coupons.
///
/// Gross reports retain native payment signs and never offset opposing positions.
#[derive(Default)]
pub struct GrossCashflowAggregator {
    compressor: CashflowCompressor,
}

impl CashflowAggregator for GrossCashflowAggregator {
    fn accumulate_cashflows(&mut self, flows: &CashflowsTable) -> Result<()> {
        self.compressor.accumulate_cashflows(flows, Grouping::Gross)
    }

    fn into_cashflows_table(self) -> Result<CashflowsTable> {
        self.compressor.into_cashflows_table()
    }
}

/// Nets compatible payments and receipts, deriving coupon quotes from total interest.
///
/// Principal advances and repayments within one reported leg on the same date
/// are combined as capital.
/// If net principal is zero, interest is retained without synthetic coupon terms;
/// an implied rate is undefined for that row.
#[derive(Default)]
pub struct NetCashflowAggregator {
    compressor: CashflowCompressor,
}

impl CashflowAggregator for NetCashflowAggregator {
    fn accumulate_cashflows(&mut self, flows: &CashflowsTable) -> Result<()> {
        self.compressor.accumulate_cashflows(flows, Grouping::Net)
    }

    fn into_cashflows_table(self) -> Result<CashflowsTable> {
        self.compressor.into_cashflows_table()
    }
}

/// Economic grouping policy selected by the injected implementation.
#[derive(Clone, Copy)]
enum Grouping {
    Gross,
    Net,
}

/// Contract terms that must agree before coupons can share an equivalent quote.
#[derive(Clone, PartialEq, Eq, Hash)]
struct CouponIdentity {
    start: Date,
    end: Date,
    index: Option<MarketIndex>,
    forward_index: Option<MarketIndex>,
    day_counter: DayCounter,
    definition: Option<RateDefinition>,
    overnight: Option<OvernightCouponConventions>,
    payoff: Option<PayoffOps>,
    rate: Option<u64>,
    spread: Option<u64>,
}

/// Typed identity for one compatible group; no display strings identify economics.
#[derive(Clone, PartialEq, Eq, Hash)]
struct CashflowIdentity {
    date: Date,
    currency: Currency,
    kind: CashflowKind,
    leg: usize,
    direction: Option<bool>,
    coupon: Option<CouponIdentity>,
    fixing: Option<u64>,
    accrual: u64,
    floor: Option<u64>,
    cap: Option<u64>,
    unique: Option<usize>,
}

/// Numeric totals and representative terms for one economically compatible group.
struct CashflowGroup {
    identity: CashflowIdentity,
    fixing: Option<f64>,
    accrual: f64,
    floor: Option<f64>,
    cap: Option<f64>,
    details: Option<CouponDetails>,
    amount: CompensatedSum,
    notional: CompensatedSum,
    equivalent_rate: bool,
    equivalent_spread: bool,
}

/// Shared grouping engine containing owned numeric values only.
#[derive(Default)]
struct CashflowCompressor {
    // Store groups in insertion order so hash iteration never determines output order.
    groups: Vec<CashflowGroup>,
    indices: HashMap<CashflowIdentity, usize>,
    serial: usize,
}

impl CashflowCompressor {
    /// Merges numeric cashflows into compatible payment and coupon groups.
    fn accumulate_cashflows(&mut self, flows: &CashflowsTable, grouping: Grouping) -> Result<()> {
        for row in 0..flows.payment_dates().len() {
            self.serial += 1;
            let mut kind = flows.cashflow_types()[row].clone();
            if matches!(grouping, Grouping::Net)
                && matches!(kind, CashflowKind::Redemption | CashflowKind::Disbursement)
            {
                kind = CashflowKind::Capital;
            }
            let details = flows.coupon_details()[row].clone();
            let equivalent_rate = matches!(kind, CashflowKind::FixedRateCoupon)
                && details.as_ref().is_some_and(|terms| terms.rate.is_some());
            let equivalent_spread = matches!(kind, CashflowKind::FloatingRateCoupon)
                && details.as_ref().is_some_and(|terms| {
                    !terms
                        .overnight_conventions
                        .as_ref()
                        .is_some_and(OvernightCouponConventions::compound_spread_daily)
                });
            let coupon = details.as_ref().map(|terms| CouponIdentity {
                start: terms.accrual_start,
                end: terms.accrual_end,
                index: terms.market_index.clone(),
                forward_index: terms.forward_index.clone(),
                day_counter: terms.day_counter,
                definition: terms.rate_definition,
                overnight: terms.overnight_conventions.clone(),
                payoff: terms.payoff.clone(),
                rate: if equivalent_rate {
                    None
                } else {
                    terms.rate.map(f64::to_bits)
                },
                spread: if equivalent_spread {
                    None
                } else {
                    terms.spread.map(f64::to_bits)
                },
            });
            let known_without_terms = matches!(
                kind,
                CashflowKind::Redemption
                    | CashflowKind::Disbursement
                    | CashflowKind::Capital
                    | CashflowKind::ConstantAmount
                    | CashflowKind::Interest
            );
            let identity = CashflowIdentity {
                date: flows.payment_dates()[row],
                currency: flows.currencies()[row],
                kind,
                leg: flows.leg_indices()[row],
                direction: match grouping {
                    Grouping::Gross => Some(
                        details
                            .as_ref()
                            .map_or_else(|| flows.amounts()[row], |terms| terms.notional)
                            .is_sign_negative(),
                    ),
                    Grouping::Net => None,
                },
                coupon,
                fixing: flows.fixing()[row].map(f64::to_bits),
                accrual: flows.accrual_periods()[row].to_bits(),
                floor: flows.floorlet_strikes()[row].map(f64::to_bits),
                cap: flows.caplet_strikes()[row].map(f64::to_bits),
                unique: (details.is_none() && !known_without_terms).then_some(self.serial),
            };
            let index = if let Some(index) = self.indices.get(&identity) {
                *index
            } else {
                let index = self.groups.len();
                self.groups.push(CashflowGroup {
                    identity: identity.clone(),
                    fixing: flows.fixing()[row],
                    accrual: flows.accrual_periods()[row],
                    floor: flows.floorlet_strikes()[row],
                    cap: flows.caplet_strikes()[row],
                    details: details.clone(),
                    amount: CompensatedSum::default(),
                    notional: CompensatedSum::default(),
                    equivalent_rate,
                    equivalent_spread,
                });
                self.indices.insert(identity, index);
                index
            };
            let group = &mut self.groups[index];
            group.amount.add(flows.amounts()[row])?;
            if let Some(terms) = details {
                group.notional.add(terms.notional)?;
            }
        }
        Ok(())
    }

    /// Builds the compressed cashflow table and derives compatible coupon rates and spreads.
    fn into_cashflows_table(mut self) -> Result<CashflowsTable> {
        self.groups.sort_by(|a, b| {
            (
                a.identity.date,
                a.identity.currency.to_string(),
                &a.identity.kind,
                a.identity.leg,
            )
                .cmp(&(
                    b.identity.date,
                    b.identity.currency.to_string(),
                    &b.identity.kind,
                    b.identity.leg,
                ))
        });
        let mut result = CashflowsTable::new();
        for mut group in self.groups {
            let amount = group.amount.total();
            if let Some(details) = &mut group.details {
                details.notional = group.notional.total();
                if details.notional == 0.0 {
                    group.details = None;
                } else if group.equivalent_rate || group.equivalent_spread {
                    let accrual = details
                        .day_counter
                        .year_fraction(details.accrual_start, details.accrual_end);
                    let denominator = details.notional * accrual;
                    let rate = amount / denominator;
                    if !denominator.is_finite() || accrual <= 0.0 || !rate.is_finite() {
                        return Err(QSError::InvalidValueErr(
                            "Invalid equivalent coupon accrual or interest".into(),
                        ));
                    }
                    if group.equivalent_rate {
                        details.rate = Some(rate);
                    }
                    if group.equivalent_spread {
                        let fixing = group.fixing.ok_or_else(|| {
                            QSError::ValueNotSetErr("Floating coupon fixing".into())
                        })?;
                        let spread = rate - fixing;
                        if !spread.is_finite() {
                            return Err(QSError::InvalidValueErr(
                                "Nonfinite equivalent coupon spread".into(),
                            ));
                        }
                        details.spread = Some(spread);
                    }
                }
            }
            result.add_cashflow(
                group.identity.date,
                group.identity.kind,
                amount,
                group.fixing,
                group.accrual,
                group.identity.currency,
                group.floor,
                group.cap,
                group.identity.leg,
            );
            if let Some(details) = group.details {
                result.set_last_coupon_details(details);
            }
        }
        Ok(result)
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
        let mut compressor = GrossCashflowAggregator::default();
        compressor.accumulate_cashflows(&flows)?;
        let result = compressor.into_cashflows_table()?;
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
        for cap in [0.04, 0.06, 0.04] {
            let mut changed = base.clone();
            changed.payoff = Some(PayoffOps::Min(
                Box::new(PayoffOps::Index),
                Box::new(PayoffOps::Const(cap)),
            ));
            add(&mut flows, changed, Some(0.05))?;
        }
        let mut compressor = GrossCashflowAggregator::default();
        compressor.accumulate_cashflows(&flows)?;
        assert_eq!(compressor.into_cashflows_table()?.payment_dates().len(), 8);
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
        let mut compressor = GrossCashflowAggregator::default();
        compressor.accumulate_cashflows(&flows)?;
        assert_eq!(compressor.into_cashflows_table()?.payment_dates().len(), 4);
        Ok(())
    }
    #[test]
    fn gross_preserves_opposing_positions_and_net_uses_actual_interest() -> Result<()> {
        let mut flows = CashflowsTable::new();
        add(&mut flows, terms(None, 100.0, 0.04), None)?;
        add(&mut flows, terms(None, -50.0, 0.06), None)?;
        let mut gross = GrossCashflowAggregator::default();
        gross.accumulate_cashflows(&flows)?;
        let gross = gross.into_cashflows_table()?;
        assert_eq!(gross.payment_dates().len(), 2);
        let mut net = NetCashflowAggregator::default();
        net.accumulate_cashflows(&flows)?;
        let net = net.into_cashflows_table()?;
        assert_eq!(net.payment_dates().len(), 1);
        let details = net.coupon_details()[0]
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("net coupon".into()))?;
        assert_eq!(details.notional(), 50.0);
        assert!((details.rate().unwrap_or_default() - 0.02).abs() < 1e-14);
        let CashflowType::FixedRateCoupon(coupon) =
            details.to_coupon(net.payment_dates()[0], None)?
        else {
            panic!("Expected fixed coupon");
        };
        assert!((coupon.amount()? - flows.amounts().iter().sum::<f64>()).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn zero_net_principal_preserves_residual_interest_without_an_implied_rate() -> Result<()> {
        let mut flows = CashflowsTable::new();
        add(&mut flows, terms(None, 100.0, 0.04), None)?;
        add(&mut flows, terms(None, -100.0, 0.06), None)?;
        let interest = flows.amounts().iter().sum::<f64>();
        let mut net = NetCashflowAggregator::default();
        net.accumulate_cashflows(&flows)?;
        let net = net.into_cashflows_table()?;
        assert_eq!(net.amounts(), &[interest]);
        assert_eq!(net.cashflow_types(), &[CashflowKind::FixedRateCoupon]);
        assert!(net.coupon_details()[0].is_none());
        assert_eq!(net.implied_rates(RateDefinition::default())?, vec![None]);
        assert!((summarize_cashflows(&net)?[0].interest() - interest).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn reported_interest_determines_equivalent_rates_and_spreads() -> Result<()> {
        let mut flows = CashflowsTable::new();
        // Quotes deliberately differ from the reported interest (for example, adjusted projections).
        for (kind, index, amount, fixing) in [
            (CashflowKind::FixedRateCoupon, None, 12.0, None),
            (
                CashflowKind::FloatingRateCoupon,
                Some(MarketIndex::SOFR),
                12.0,
                Some(0.02),
            ),
        ] {
            flows.add_cashflow(
                Date::new(2027, 1, 17),
                kind,
                amount,
                fixing,
                365.0 / 360.0,
                Currency::USD,
                None,
                None,
                0,
            );
            flows.set_last_coupon_details(terms(index, 100.0, 0.99));
        }
        let mut gross = GrossCashflowAggregator::default();
        gross.accumulate_cashflows(&flows)?;
        let result = gross.into_cashflows_table()?;
        for (row, details) in result.coupon_details().iter().enumerate() {
            let details = details
                .as_ref()
                .ok_or_else(|| QSError::ValueNotSetErr("coupon".into()))?;
            let amount =
                match details.to_coupon(result.payment_dates()[row], result.fixing()[row])? {
                    CashflowType::FixedRateCoupon(coupon) => coupon.amount()?,
                    CashflowType::FloatingRateCoupon(coupon) => coupon.amount()?,
                    _ => unreachable!(),
                };
            assert!((amount - 12.0).abs() < 1e-12);
        }
        Ok(())
    }

    #[test]
    fn implied_rates_use_actual_growth_accrual_dates_and_requested_conventions() -> Result<()> {
        let mut flows = CashflowsTable::new();
        let mut capped = terms(Some(MarketIndex::SOFR), 100.0, 0.001);
        capped.payoff = Some(PayoffOps::Min(
            Box::new(PayoffOps::Index),
            Box::new(PayoffOps::Const(0.04)),
        ));
        add(&mut flows, capped, Some(0.10))?;
        add(&mut flows, terms(None, -200.0, -0.03), None)?;
        for day_counter in [DayCounter::Actual360, DayCounter::Actual365] {
            for compounding in [
                Compounding::Simple,
                Compounding::Compounded,
                Compounding::Continuous,
                Compounding::SimpleThenCompounded,
                Compounding::CompoundedThenSimple,
            ] {
                let convention =
                    RateDefinition::new(day_counter, compounding, Frequency::Semiannual);
                let rates = flows.implied_rates(convention)?;
                for (row, rate) in rates.into_iter().enumerate() {
                    let rate =
                        rate.ok_or_else(|| QSError::ValueNotSetErr("implied rate".into()))?;
                    let details = flows.coupon_details()[row]
                        .as_ref()
                        .ok_or_else(|| QSError::ValueNotSetErr("coupon".into()))?;
                    let growth = 1.0 + flows.amounts()[row] / details.notional();
                    assert_eq!(rate.rate_definition(), convention);
                    assert!(
                        (rate.compound_factor(details.accrual_start(), details.accrual_end())
                            - growth)
                            .abs()
                            < 1e-12
                    );
                }
            }
        }
        let actual360 = flows.implied_rates(RateDefinition::default())?[0]
            .map(|rate| rate.rate())
            .unwrap_or_default();
        let actual365 = flows.implied_rates(RateDefinition::new(
            DayCounter::Actual365,
            Compounding::Simple,
            Frequency::Annual,
        ))?[0]
            .map(|rate| rate.rate())
            .unwrap_or_default();
        assert!(actual365 > actual360);
        assert!((actual360 - 0.041).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn net_capital_offsets_advances_and_repayments_and_retains_currencies() -> Result<()> {
        let mut flows = CashflowsTable::new();
        for (kind, amount, currency) in [
            (CashflowKind::Redemption, 100.0, Currency::USD),
            (CashflowKind::Disbursement, -80.0, Currency::USD),
            (CashflowKind::Disbursement, -10.0, Currency::EUR),
        ] {
            flows.add_cashflow(
                Date::new(2027, 1, 17),
                kind,
                amount,
                None,
                0.0,
                currency,
                None,
                None,
                0,
            );
        }
        let mut net = NetCashflowAggregator::default();
        net.accumulate_cashflows(&flows)?;
        let net = net.into_cashflows_table()?;
        assert_eq!(
            net.cashflow_types(),
            &[CashflowKind::Capital, CashflowKind::Capital]
        );
        assert_eq!(net.currencies(), &[Currency::EUR, Currency::USD]);
        assert_eq!(net.amounts(), &[-10.0, 20.0]);
        Ok(())
    }

    #[test]
    fn invalid_implied_rate_inputs_are_rejected() {
        let convention = RateDefinition::default();
        for notional in [0.0, f64::NAN, f64::INFINITY] {
            assert!(terms(None, notional, 0.04)
                .implied_rate(1.0, convention)
                .is_err());
        }
        let coupon = terms(None, 100.0, 0.04);
        for amount in [f64::NAN, f64::INFINITY, -100.0, -101.0] {
            assert!(coupon.implied_rate(amount, convention).is_err());
        }
        let mut invalid = coupon.clone();
        invalid.accrual_end = invalid.accrual_start;
        assert!(invalid.implied_rate(1.0, convention).is_err());
        for frequency in [
            Frequency::NoFrequency,
            Frequency::Once,
            Frequency::OtherFrequency,
        ] {
            assert!(coupon
                .implied_rate(
                    1.0,
                    RateDefinition::new(DayCounter::Actual360, Compounding::Compounded, frequency)
                )
                .is_err());
        }
    }
}
