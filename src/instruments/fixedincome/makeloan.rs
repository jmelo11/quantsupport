//! Contractual loan schedules built with existing coupon and redemption types.

use super::{
    loan::Loan, loanratedefinition::LoanRateDefinition,
    loanrepaymentstructure::LoanRepaymentStructure,
};
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        instrument::AssetClass, marketdatahandling::forwardraterequest::ForwardRateRequest,
        trade::Side,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{
        cashflow::SimpleCashflow, cashflowtype::CashflowType, fixedratecoupon::FixedRateCoupon,
        floatingratecoupon::FloatingRateCoupon, leg::Leg,
    },
    models::{
        prepaymentmodel::PrepaymentModel,
        prepaymentmodelconfiguration::PrepaymentModelConfiguration,
    },
    rates::{
        compounding::Compounding,
        interestrate::{InterestRate, RateDefinition},
    },
    time::{
        calendar::Calendar,
        calendars::{nullcalendar::NullCalendar, traits::IsCalendar},
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency, TimeUnit},
        period::Period,
        schedule::MakeSchedule,
    },
    utils::errors::{QSError, Result},
};
use serde::{Deserialize, Serialize};
use std::{marker::PhantomData, sync::Arc};

/// Builds contractual principal schedules.
///
/// Defaults belong to this builder:
/// monthly payments, French amortization, Actual/365, unadjusted dates, no
/// prepayment or credit, and 40% recovery. Index tenor and reset frequency come
/// from rate-index metadata. Floating amortization requires an explicit initial
/// all-in rate; the resulting notionals stay fixed when market curves change.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct MakeLoan<T: Scalar = f64> {
    identifier: Option<String>,
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    notional: Option<f64>,
    rate: Option<LoanRateDefinition>,
    repayment: Option<LoanRepaymentStructure>,
    currency: Option<Currency>,
    discount_index: Option<MarketIndex>,
    payment_frequency: Option<Frequency>,
    fixing_frequency: Option<Frequency>,
    day_counter: Option<DayCounter>,
    amortization_rate: Option<f64>,
    prepayment: Option<PrepaymentModelConfiguration>,
    credit_index: Option<MarketIndex>,
    recovery: Option<f64>,
    fixing_lag: Option<i32>,
    index_tenor: Option<Period>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    #[serde(skip)]
    prepayment_function: Option<Arc<dyn PrepaymentModel<DualFwd>>>,
    #[serde(skip)]
    marker: PhantomData<T>,
}

impl<T: Scalar> MakeLoan<T> {
    /// Sets the contract identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the origination accrual date.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self
    }

    /// Sets the final contractual accrual date.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self
    }

    /// Sets the original positive principal.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the fixed, floating, or mixed contractual rate definition.
    #[must_use]
    pub fn with_rate(mut self, rate: LoanRateDefinition) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the contractual amortization structure.
    #[must_use]
    pub const fn with_repayment_structure(mut self, repayment: LoanRepaymentStructure) -> Self {
        self.repayment = Some(repayment);
        self
    }

    /// Sets the contract currency.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the default discount curve index.
    #[must_use]
    pub fn with_discount_index(mut self, discount_index: MarketIndex) -> Self {
        self.discount_index = Some(discount_index);
        self
    }

    /// Sets the payment frequency, independently of resets.
    #[must_use]
    pub const fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the reset frequency, independently of index tenor.
    #[must_use]
    pub const fn with_fixing_frequency(mut self, frequency: Frequency) -> Self {
        self.fixing_frequency = Some(frequency);
        self
    }

    /// Sets the interest accrual day counter.
    #[must_use]
    pub const fn with_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.day_counter = Some(day_counter);
        self
    }

    /// Attaches the conditional prepayment configuration, checked during build.
    #[must_use]
    pub fn with_prepayment(mut self, prepayment: PrepaymentModelConfiguration) -> Self {
        self.prepayment = Some(prepayment);
        self
    }

    /// Sets the credit survival curve index.
    #[must_use]
    pub fn with_credit_index(mut self, credit_index: MarketIndex) -> Self {
        self.credit_index = Some(credit_index);
        self
    }

    /// Sets the fraction of principal recovered on default.
    #[must_use]
    pub const fn with_recovery(mut self, recovery: f64) -> Self {
        self.recovery = Some(recovery);
        self
    }

    /// Sets the business-day fixing lag.
    #[must_use]
    pub const fn with_fixing_lag(mut self, fixing_lag: i32) -> Self {
        self.fixing_lag = Some(fixing_lag);
        self
    }

    /// Sets the forward-rate tenor.
    #[must_use]
    pub const fn with_index_tenor(mut self, index_tenor: Period) -> Self {
        self.index_tenor = Some(index_tenor);
        self
    }

    /// Sets the payment and fixing calendar.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Sets the payment date adjustment convention.
    #[must_use]
    pub const fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Initial all-in rate used only to generate contractual amortization.
    #[must_use]
    pub const fn with_amortization_rate(mut self, rate: f64) -> Self {
        self.amortization_rate = Some(rate);
        self
    }

    /// Attaches an arbitrary conditional prepayment function for the pricer.
    #[must_use]
    pub fn with_prepayment_function(
        mut self,
        function: impl PrepaymentModel<DualFwd> + 'static,
    ) -> Self {
        self.prepayment_function = Some(Arc::new(function));
        self
    }

    /// Builds existing cashflow variants without applying the prepayment function.
    ///
    /// # Errors
    /// Rejects missing or invalid terms, unsupported reset boundaries, and
    /// floating amortization without an explicit initial all-in rate.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Loan<T>> {
        let identifier = required(self.identifier, "Loan identifier")?;
        let start = required(self.start_date, "Loan start date")?;
        let end = required(self.maturity_date, "Loan maturity date")?;
        let notional = required(self.notional, "Loan notional")?;
        let currency = required(self.currency, "Loan currency")?;
        let discount_index = required(self.discount_index, "Loan discount index")?;
        let rate = required(self.rate, "Loan rate definition")?;
        let repayment = self.repayment.unwrap_or(LoanRepaymentStructure::French);
        if identifier.trim().is_empty() || start >= end || !notional.is_finite() || notional <= 0.0
        {
            return Err(QSError::InvalidValueErr(
                "Invalid loan dates or principal".into(),
            ));
        }
        let payment_frequency = self.payment_frequency.unwrap_or(Frequency::Monthly);
        let payment_period = valid_period(payment_frequency)?;
        let calendar = self
            .calendar
            .unwrap_or_else(|| Calendar::NullCalendar(NullCalendar::new()));
        let convention = self
            .business_day_convention
            .unwrap_or(BusinessDayConvention::Unadjusted);
        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual365);
        let fixing_lag = self.fixing_lag.unwrap_or(0);
        if fixing_lag < 0 {
            return Err(QSError::InvalidValueErr(
                "Loan fixing lag must be nonnegative".into(),
            ));
        }
        let index = match &rate {
            LoanRateDefinition::Fixed(_) => None,
            LoanRateDefinition::Floating { index, .. }
            | LoanRateDefinition::Mixed { index, .. } => Some(index),
        };
        let index_frequency = index
            .map(|index| {
                index
                    .rate_index_details()
                    .map(|details| details.frequency())
            })
            .transpose()?
            .unwrap_or(payment_frequency);
        let fixing_frequency =
            self.fixing_frequency
                .unwrap_or(if index_frequency == Frequency::Daily {
                    payment_frequency
                } else {
                    index_frequency
                });
        let reset_period = valid_period(fixing_frequency)?;
        let index_tenor = self.index_tenor.unwrap_or(valid_period(index_frequency)?);
        if index_tenor.length() <= 0 {
            return Err(QSError::InvalidValueErr(
                "Loan index tenor must be positive".into(),
            ));
        }
        let schedule = MakeSchedule::new(start, end)
            .with_tenor(payment_period)
            .with_convention(BusinessDayConvention::Unadjusted)
            .with_termination_date_convention(BusinessDayConvention::Unadjusted)
            .forwards()
            .build()?;
        let dates = schedule.dates();
        let amortization_rate = match (&rate, self.amortization_rate) {
            (_, Some(rate)) => rate,
            (
                LoanRateDefinition::Fixed(rate)
                | LoanRateDefinition::Mixed {
                    fixed_rate: rate, ..
                },
                None,
            ) => *rate,
            (LoanRateDefinition::Floating { .. }, None)
                if matches!(repayment, LoanRepaymentStructure::Bullet) =>
            {
                0.0
            }
            _ => {
                return Err(QSError::ValueNotSetErr(
                    "Floating French/balloon loans require an initial amortization_rate".into(),
                ));
            }
        };
        let period_rates = dates
            .windows(2)
            .map(|period| amortization_rate * day_counter.year_fraction(period[0], period[1]))
            .collect::<Vec<_>>();
        let notionals = amortization_schedule(notional, &period_rates, &repayment)?;
        let first_reset = match &rate {
            LoanRateDefinition::Mixed { switch_date, .. } => {
                if *switch_date <= start
                    || *switch_date >= end
                    || dates.binary_search(switch_date).is_err()
                {
                    return Err(QSError::InvalidValueErr(
                        "Mixed loan switch must be an interior payment boundary".into(),
                    ));
                }
                *switch_date
            }
            _ => start,
        };
        let resets = if index.is_some() {
            MakeSchedule::new(first_reset, end)
                .with_tenor(reset_period)
                .with_convention(BusinessDayConvention::Unadjusted)
                .with_termination_date_convention(BusinessDayConvention::Unadjusted)
                .forwards()
                .build()?
                .dates()
                .clone()
        } else {
            Vec::new()
        };
        if resets.iter().any(|date| dates.binary_search(date).is_err()) {
            return Err(QSError::InvalidValueErr(
                "Loan resets must align with payment accrual boundaries".into(),
            ));
        }
        let fixed_definition =
            RateDefinition::new(day_counter, Compounding::Simple, Frequency::Annual);
        let mut cashflows = vec![CashflowType::Disbursement(SimpleCashflow::new(
            notional, start,
        ))];
        let mut observations = Vec::new();
        for (position, period) in dates.windows(2).enumerate() {
            let accrual_start = period[0];
            let accrual_end = period[1];
            let payment_date = calendar.adjust(accrual_end, Some(convention));
            let fixed_rate = match &rate {
                LoanRateDefinition::Fixed(rate) => Some(*rate),
                LoanRateDefinition::Mixed {
                    fixed_rate,
                    switch_date,
                    ..
                } if accrual_start < *switch_date => Some(*fixed_rate),
                _ => None,
            };
            if let Some(rate) = fixed_rate {
                if !rate.is_finite() {
                    return Err(QSError::InvalidValueErr(
                        "Loan coupon must be finite".into(),
                    ));
                }
                cashflows.push(CashflowType::FixedRateCoupon(FixedRateCoupon::new(
                    notionals[position],
                    Box::new(InterestRate::from_rate_definition(
                        T::scalar(rate),
                        fixed_definition,
                    )),
                    accrual_start,
                    accrual_end,
                    payment_date,
                )));
                observations.push(None);
            } else {
                let index =
                    index.ok_or_else(|| QSError::ValueNotSetErr("Loan floating index".into()))?;
                let spread = match &rate {
                    LoanRateDefinition::Floating { spread, .. }
                    | LoanRateDefinition::Mixed { spread, .. } => *spread,
                    LoanRateDefinition::Fixed(_) => 0.0,
                };
                if !spread.is_finite() {
                    return Err(QSError::InvalidValueErr(
                        "Loan spread must be finite".into(),
                    ));
                }
                let reset_date = resets
                    .iter()
                    .copied()
                    .take_while(|date| *date <= accrual_start)
                    .last()
                    .ok_or_else(|| QSError::ValueNotSetErr("Loan reset date".into()))?;
                let fixing_date = calendar.advance(
                    reset_date,
                    Period::new(-fixing_lag, TimeUnit::Days),
                    Some(BusinessDayConvention::Preceding),
                    false,
                );
                let index_start =
                    calendar.adjust(reset_date, Some(BusinessDayConvention::ModifiedFollowing));
                let index_end = calendar.advance(
                    index_start,
                    index_tenor,
                    Some(BusinessDayConvention::ModifiedFollowing),
                    false,
                );
                observations.push(Some(
                    ForwardRateRequest::new(index.clone(), fixing_date)
                        .with_start_date(index_start)
                        .with_end_date(index_end),
                ));
                cashflows.push(CashflowType::FloatingRateCoupon(
                    FloatingRateCoupon::new(
                        notionals[position],
                        T::scalar(spread),
                        index.clone(),
                        accrual_start,
                        accrual_end,
                        payment_date,
                    )
                    .with_day_counter(day_counter),
                ));
            }
            let next = notionals.get(position + 1).copied().unwrap_or(0.0);
            let principal = (notionals[position] - next).max(0.0);
            if principal > 0.0 {
                cashflows.push(CashflowType::Redemption(SimpleCashflow::new(
                    principal,
                    payment_date,
                )));
            }
        }
        let prepayment = self.prepayment.unwrap_or_default();
        prepayment.validate()?;
        let prepayment = self
            .prepayment_function
            .unwrap_or_else(|| Arc::new(prepayment));
        let last_payment = calendar.adjust(end, Some(convention));
        let leg = Leg::new(
            0,
            cashflows,
            currency,
            Some(discount_index),
            index.cloned(),
            None,
            None,
            Side::LongReceive,
            true,
            AssetClass::FixedIncome,
            start,
            last_payment,
        );
        Loan::new(
            identifier,
            notional,
            leg,
            observations,
            prepayment,
            Some(repayment),
            self.credit_index,
            self.recovery.unwrap_or(0.4),
        )
    }
}

fn required<V>(value: Option<V>, name: &str) -> Result<V> {
    value.ok_or_else(|| QSError::ValueNotSetErr(name.into()))
}
fn valid_period(frequency: Frequency) -> Result<Period> {
    Period::from_frequency(frequency)
        .filter(|period| period.length() > 0)
        .ok_or_else(|| QSError::InvalidValueErr("Unsupported loan frequency".into()))
}

/// Opening contractual balances; all arithmetic is ordinary f64, independent of AD.
pub(crate) fn amortization_schedule(
    principal: f64,
    period_rates: &[f64],
    repayment: &LoanRepaymentStructure,
) -> Result<Vec<f64>> {
    if period_rates.is_empty()
        || !principal.is_finite()
        || principal <= 0.0
        || period_rates
            .iter()
            .any(|rate| !rate.is_finite() || *rate <= -1.0)
    {
        return Err(QSError::InvalidValueErr(
            "Invalid loan amortization rate or schedule".into(),
        ));
    }
    let residual = match repayment {
        LoanRepaymentStructure::Balloon { residual_fraction } => *residual_fraction,
        _ => 0.0,
    };
    if !residual.is_finite() || !(0.0..=1.0).contains(&residual) {
        return Err(QSError::InvalidValueErr(
            "Loan balloon fraction must be in [0, 1]".into(),
        ));
    }
    if matches!(repayment, LoanRepaymentStructure::Bullet) {
        return Ok(vec![principal; period_rates.len()]);
    }
    let mut discount = 1.0;
    let mut annuity = 0.0;
    for rate in period_rates {
        discount /= 1.0 + rate;
        annuity += discount;
    }
    let payment = (principal * residual).mul_add(-discount, principal) / annuity;
    if !payment.is_finite() || annuity <= 0.0 {
        return Err(QSError::InvalidValueErr(
            "Invalid loan annuity factor".into(),
        ));
    }
    let mut balance = principal;
    let mut notionals = Vec::with_capacity(period_rates.len());
    for (position, rate) in period_rates.iter().enumerate() {
        notionals.push(balance);
        if position + 1 < period_rates.len() {
            let next = balance.mul_add(1.0 + rate, -payment);
            if !next.is_finite() || next <= 0.0 || next > principal.mul_add(1e-10, balance) {
                return Err(QSError::InvalidValueErr(
                    "Loan amortization must retain positive, nonincreasing principal".into(),
                ));
            }
            balance = next.min(balance);
        }
    }
    Ok(notionals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::cashflows::cashflow::Cashflow;

    fn builder() -> MakeLoan<f64> {
        MakeLoan::default()
            .with_identifier("LOAN".into())
            .with_start_date(Date::new(2026, 1, 1))
            .with_maturity_date(Date::new(2028, 1, 1))
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_discount_index(MarketIndex::ESTR)
            .with_rate(LoanRateDefinition::Fixed(0.06))
    }
    #[test]
    fn french_contract_uses_existing_cashflows_and_matches_annuity() -> Result<()> {
        let loan = builder()
            .with_maturity_date(Date::new(2056, 1, 1))
            .with_day_counter(DayCounter::Thirty360)
            .build()?;
        let amounts = loan
            .leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| match cashflow {
                CashflowType::FixedRateCoupon(coupon) => Some(coupon.amount()),
                CashflowType::Redemption(principal) => Some(principal.amount()),
                _ => None,
            })
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(amounts.len(), 720);
        for payment in amounts.chunks(2) {
            assert!((payment[0] + payment[1] - 599.550_525_152_756_9).abs() < 1e-6);
        }
        let principal: f64 = loan
            .leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| {
                if let CashflowType::Redemption(principal) = cashflow {
                    principal.amount().ok()
                } else {
                    None
                }
            })
            .sum();
        assert!((principal - loan.notional()).abs() < 1e-7);
        Ok(())
    }
    #[test]
    fn index_frequency_defines_reset_requests_and_tenor() -> Result<()> {
        for (index, frequency) in [
            (MarketIndex::EURIBOR1m, Frequency::Monthly),
            (MarketIndex::EURIBOR3m, Frequency::Quarterly),
            (MarketIndex::EURIBOR6m, Frequency::Semiannual),
            (MarketIndex::EURIBOR12m, Frequency::Annual),
            (MarketIndex::TermSOFR1m, Frequency::Monthly),
            (MarketIndex::TermSOFR3m, Frequency::Quarterly),
            (MarketIndex::TermSOFR6m, Frequency::Semiannual),
            (MarketIndex::TermSOFR12m, Frequency::Annual),
            (MarketIndex::TIBOR3m, Frequency::Quarterly),
            (MarketIndex::TIBOR6m, Frequency::Semiannual),
        ] {
            assert_eq!(index.rate_index_details()?.frequency(), frequency);
            let loan = builder()
                .with_rate(LoanRateDefinition::Floating {
                    index,
                    spread: 0.01,
                })
                .with_amortization_rate(0.05)
                .build()?;
            let requests: Vec<_> = loan.observations().iter().flatten().collect();
            assert_eq!(requests.len(), 24);
            let period = Period::from_frequency(frequency)
                .ok_or_else(|| QSError::ValueNotSetErr("Index period".into()))?;
            let first = requests[0];
            assert_eq!(
                first.end_date(),
                Some(Date::new(2026, 1, 1).advance(period.length(), period.units()))
            );
            let distinct = requests
                .iter()
                .map(|request| request.fixing_date())
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(distinct.len(), frequency as usize * 2);
        }
        Ok(())
    }
    #[test]
    fn prepayment_does_not_modify_contractual_cashflows() -> Result<()> {
        let plain = builder().build()?;
        let prepaying = builder()
            .with_prepayment(PrepaymentModelConfiguration::Cpr(1.0))
            .build()?;
        let principals = |loan: &Loan<f64>| {
            loan.leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::Redemption(principal) = cashflow {
                        Some(principal.amount())
                    } else {
                        None
                    }
                })
                .collect::<Result<Vec<_>>>()
        };
        assert_eq!(principals(&plain)?, principals(&prepaying)?);
        let coupons = |loan: &Loan<f64>| {
            loan.leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::FixedRateCoupon(coupon) = cashflow {
                        Some(coupon.notional())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(coupons(&plain), coupons(&prepaying));
        let ad: Loan<DualFwd> = prepaying.into();
        assert_eq!(ad.notional(), 100_000.0);
        Ok(())
    }
    #[test]
    fn stubs_bullet_balloon_and_zero_rates_repay_principal() -> Result<()> {
        for structure in [
            LoanRepaymentStructure::French,
            LoanRepaymentStructure::Bullet,
            LoanRepaymentStructure::Balloon {
                residual_fraction: 0.5,
            },
        ] {
            let loan = builder()
                .with_rate(LoanRateDefinition::Fixed(0.0))
                .with_repayment_structure(structure)
                .with_maturity_date(Date::new(2027, 1, 15))
                .build()?;
            let principal: f64 = loan
                .leg()
                .cashflows()
                .iter()
                .filter_map(|cashflow| {
                    if let CashflowType::Redemption(principal) = cashflow {
                        principal.amount().ok()
                    } else {
                        None
                    }
                })
                .sum();
            assert!((principal - loan.notional()).abs() < 1e-7);
        }
        Ok(())
    }
    #[test]
    fn invalid_terms_and_missing_floating_amortization_are_rejected() {
        assert!(MakeLoan::<f64>::default().build().is_err());
        assert!(builder().with_notional(-1.0).build().is_err());
        assert!(builder()
            .with_rate(LoanRateDefinition::Floating {
                index: MarketIndex::EURIBOR12m,
                spread: 0.01
            })
            .build()
            .is_err());
        assert!(builder()
            .with_fixing_frequency(Frequency::Once)
            .build()
            .is_err());
        assert!(builder()
            .with_credit_index(MarketIndex::ESTR)
            .build()
            .is_err());
        assert!(builder()
            .with_prepayment(PrepaymentModelConfiguration::Cpr(1.1))
            .build()
            .is_err());
        assert!(builder()
            .with_repayment_structure(LoanRepaymentStructure::Balloon {
                residual_fraction: 1.1
            })
            .build()
            .is_err());
    }
    #[test]
    fn generic_constructor_accepts_custom_contract_without_builder_metadata() -> Result<()> {
        let loan = builder().build()?;
        let custom = Loan::new(
            "CUSTOM".into(),
            loan.notional(),
            loan.leg().clone(),
            loan.observations().to_vec(),
            Arc::new(PrepaymentModelConfiguration::None),
            None,
            None,
            0.25,
        )?;
        assert!(custom.repayment().is_none());
        assert_eq!(custom.leg().cashflows().len(), loan.leg().cashflows().len());
        Ok(())
    }
}
