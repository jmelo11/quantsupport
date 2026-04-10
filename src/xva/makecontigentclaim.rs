use crate::{
    core::{collateral::Discountable, trade::Side},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{
        cashflow::Cashflow,
        cashflowtype::CashflowType,
        coupons::{LinearCoupon, NonLinearCoupon},
        leg::Leg,
    },
    time::date::Date,
    utils::errors::{QSError, Result},
    xva::{claimevaluationstrategy::ClaimEvaluationStrategy, contigentclaim::ContingentClaim},
};

/// Builder for [`ContingentClaim`].
///
/// Uses the same `with_*` + `build()` pattern as [`MakeLeg`] and [`MakeSwap`].
///
/// ## Example
/// ```ignore
/// let claim = MakeContingentClaim::default()
///     .with_trade_id("swap_1".into())
///     .with_leg_id(0)
///     .with_payment_date(Date::new(2025, 6, 15))
///     .with_currency(Currency::USD)
///     .with_notional(1_000_000.0)
///     .with_side(Side::LongReceive)
///     .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount: 15_000.0 })
///     .build()
///     .expect("failed to build claim");
/// ```
pub struct MakeContingentClaim {
    trade_id: Option<String>,
    leg_id: Option<usize>,
    payment_date: Option<Date>,
    fixing_date: Option<Date>,
    accrual_start: Option<Date>,
    accrual_end: Option<Date>,
    currency: Option<Currency>,
    foreign_currency: Option<Currency>,
    notional: Option<f64>,
    side: Option<Side>,
    evaluation_strategy: Option<ClaimEvaluationStrategy>,
    index: Option<MarketIndex>,
}

impl Default for MakeContingentClaim {
    fn default() -> Self {
        Self {
            trade_id: None,
            leg_id: None,
            payment_date: None,
            fixing_date: None,
            accrual_start: None,
            accrual_end: None,
            currency: None,
            foreign_currency: None,
            notional: None,
            side: None,
            evaluation_strategy: None,
            index: None,
        }
    }
}

impl MakeContingentClaim {
    #[must_use]
    pub fn with_trade_id(mut self, trade_id: String) -> Self {
        self.trade_id = Some(trade_id);
        self
    }

    #[must_use]
    pub const fn with_leg_id(mut self, leg_id: usize) -> Self {
        self.leg_id = Some(leg_id);
        self
    }

    #[must_use]
    pub const fn with_payment_date(mut self, payment_date: Date) -> Self {
        self.payment_date = Some(payment_date);
        self
    }

    #[must_use]
    pub const fn with_fixing_date(mut self, fixing_date: Date) -> Self {
        self.fixing_date = Some(fixing_date);
        self
    }

    #[must_use]
    pub const fn with_accrual_start(mut self, accrual_start: Date) -> Self {
        self.accrual_start = Some(accrual_start);
        self
    }

    #[must_use]
    pub const fn with_accrual_end(mut self, accrual_end: Date) -> Self {
        self.accrual_end = Some(accrual_end);
        self
    }

    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    #[must_use]
    pub const fn with_foreign_currency(mut self, currency: Currency) -> Self {
        self.foreign_currency = Some(currency);
        self
    }

    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    #[must_use]
    pub fn with_evaluation_strategy(mut self, strategy: ClaimEvaluationStrategy) -> Self {
        self.evaluation_strategy = Some(strategy);
        self
    }

    #[must_use]
    pub fn with_index(mut self, index: MarketIndex) -> Self {
        self.index = Some(index);
        self
    }

    pub fn build(self) -> Result<ContingentClaim> {
        let trade_id = self
            .trade_id
            .ok_or_else(|| QSError::InvalidValueErr("trade_id is required".into()))?;
        let leg_id = self
            .leg_id
            .ok_or_else(|| QSError::InvalidValueErr("leg_id is required".into()))?;
        let payment_date = self
            .payment_date
            .ok_or_else(|| QSError::InvalidValueErr("payment_date is required".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::InvalidValueErr("currency is required".into()))?;
        let notional = self
            .notional
            .ok_or_else(|| QSError::InvalidValueErr("notional is required".into()))?;
        let side = self
            .side
            .ok_or_else(|| QSError::InvalidValueErr("side is required".into()))?;
        let evaluation_strategy = self
            .evaluation_strategy
            .ok_or_else(|| QSError::InvalidValueErr("evaluation_strategy is required".into()))?;

        Ok(ContingentClaim::new(
            trade_id,
            leg_id,
            payment_date,
            self.fixing_date,
            self.accrual_start,
            self.accrual_end,
            currency,
            self.foreign_currency,
            notional,
            side,
            evaluation_strategy,
            self.index,
        ))
    }
}

/// Trait for types that can be decomposed into a flat list of [`ContingentClaim`]s.
pub trait IntoContingentClaims {
    fn into_contingent_claims(&self, trade_id: &str) -> Result<Vec<ContingentClaim>>;
}

impl IntoContingentClaims for Leg<f64> {
    fn into_contingent_claims(&self, trade_id: &str) -> Result<Vec<ContingentClaim>> {
        let mut claims = Vec::with_capacity(self.cashflows().len());

        for cf in self.cashflows().iter() {
            let base = MakeContingentClaim::default()
                .with_trade_id(trade_id.to_string())
                .with_leg_id(self.id())
                .with_currency(self.currency())
                .with_side(self.side());

            let claim = match cf {
                CashflowType::FixedRateCoupon(coupon) => {
                    let amount = Cashflow::<f64>::amount(coupon)?;
                    base.with_payment_date(Cashflow::<f64>::payment_date(coupon))
                        .with_accrual_start(coupon.accrual_start_date())
                        .with_accrual_end(coupon.accrual_end_date())
                        .with_notional(coupon.notional())
                        .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount })
                        .build()?
                }
                CashflowType::FloatingRateCoupon(coupon) => {
                    let forward_index = self
                        .forward_index()
                        .cloned()
                        .unwrap_or_else(|| coupon.market_index().clone());
                    base.with_payment_date(Cashflow::<f64>::payment_date(coupon))
                        .with_fixing_date(coupon.accrual_start_date())
                        .with_accrual_start(coupon.accrual_start_date())
                        .with_accrual_end(coupon.accrual_end_date())
                        .with_notional(LinearCoupon::<f64>::notional(coupon))
                        .with_index(forward_index)
                        .with_evaluation_strategy(ClaimEvaluationStrategy::LinearRate {
                            spread: coupon.spread(),
                            day_counter: coupon.day_counter(),
                        })
                        .build()?
                }
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    let forward_index = self
                        .forward_index()
                        .cloned()
                        .unwrap_or_else(|| coupon.market_index().clone());
                    let day_counter = coupon
                        .market_index()
                        .rate_index_details()
                        .map_or(crate::time::daycounter::DayCounter::Actual360, |d| {
                            d.rate_definition().day_counter()
                        });
                    base.with_payment_date(NonLinearCoupon::<f64>::payment_date(coupon))
                        .with_fixing_date(NonLinearCoupon::<f64>::accrual_start_date(coupon))
                        .with_accrual_start(NonLinearCoupon::<f64>::accrual_start_date(coupon))
                        .with_accrual_end(NonLinearCoupon::<f64>::accrual_end_date(coupon))
                        .with_notional(NonLinearCoupon::<f64>::notional(coupon))
                        .with_index(forward_index)
                        .with_evaluation_strategy(ClaimEvaluationStrategy::NonLinearRate {
                            payoff_ops: coupon.payoff_ops().clone(),
                            spread: coupon.spread(),
                            strike: 0.0,
                            day_counter,
                        })
                        .build()?
                }
                CashflowType::Redemption(cf) => {
                    let amount = Cashflow::<f64>::amount(cf)?;
                    base.with_payment_date(cf.payment_date())
                        .with_notional(amount)
                        .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount })
                        .build()?
                }
                CashflowType::Disbursement(cf) => {
                    let amount = Cashflow::<f64>::amount(cf)?;
                    base.with_payment_date(cf.payment_date())
                        .with_notional(amount)
                        .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount })
                        .build()?
                }
                CashflowType::ConstantAmount(cf) => {
                    let amount = Cashflow::<f64>::amount(cf)?;
                    base.with_payment_date(cf.payment_date())
                        .with_notional(amount)
                        .with_evaluation_strategy(ClaimEvaluationStrategy::Deterministic { amount })
                        .build()?
                }
                CashflowType::OptionEmbeddedCashflow(cf) => {
                    let forward_index = self
                        .forward_index()
                        .cloned()
                        .unwrap_or_else(|| cf.market_index().clone());
                    base.with_payment_date(self.last_payment_date())
                        .with_notional(cf.value())
                        .with_index(forward_index)
                        .with_evaluation_strategy(ClaimEvaluationStrategy::SpotPayoff {
                            payoff_ops: cf.payoff_ops().clone(),
                            strike: 0.0,
                            observation_date: self.last_payment_date(),
                        })
                        .build()?
                }
            };
            claims.push(claim);
        }

        Ok(claims)
    }
}

impl IntoContingentClaims for Vec<Leg<f64>> {
    fn into_contingent_claims(&self, trade_id: &str) -> Result<Vec<ContingentClaim>> {
        let mut claims = Vec::new();
        for leg in self {
            claims.extend(leg.into_contingent_claims(trade_id)?);
        }
        Ok(claims)
    }
}
