//! Contractual loan schedules built with existing coupon and redemption types.

use super::{loan::Loan, loanrepaymentstructure::LoanRepaymentStructure};
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        instrument::AssetClass, marketdatahandling::forwardraterequest::ForwardRateRequest,
        trade::Side,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflow::SimpleCashflow, cashflowtype::CashflowType, leg::Leg},
    models::{
        prepaymentmodel::PrepaymentModel,
        prepaymentmodelconfiguration::PrepaymentModelConfiguration,
    },
    rates::{compounding::Compounding, interestrate::RateDefinition},
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
use std::{collections::HashSet, sync::Arc};

/// Builds contractual principal schedules.
///
/// Defaults belong to this builder:
/// monthly payments, French amortization, Actual/365, unadjusted dates, no
/// prepayment or credit, and 40% recovery. Index tenor and reset frequency come
/// from rate-index metadata. Floating amortization requires an explicit initial
/// all-in rate; the resulting notionals stay fixed when market curves change.
///
/// Set the identifier, origination and maturity dates, positive principal,
/// currency, and coupon list before calling [`Self::build`].
/// Rates, spreads, recovery, and balloon fractions are decimal values.
/// Coupon templates are positional, as for the stepped-coupon bond: the final
/// entry repeats through maturity. Floating-index changes start new reset grids.
#[derive(Clone, Default)]
pub struct MakeLoan<T: Scalar = f64> {
    identifier: Option<String>,
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    notional: Option<f64>,
    coupons: Vec<CashflowType<T>>,
    repayment: Option<LoanRepaymentStructure>,
    currency: Option<Currency>,
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
    prepayment_function: Option<Arc<dyn PrepaymentModel<DualFwd>>>,
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

    /// Sets positional coupon templates, using the stepped-coupon bond convention.
    ///
    /// The list must be nonempty and no longer than the generated payment
    /// schedule. Its final entry repeats through maturity. Each fixed, floating,
    /// or bounded floating template is rebound to the period dates and principal.
    #[must_use]
    pub fn with_coupons(mut self, coupons: Vec<CashflowType<T>>) -> Self {
        self.coupons = coupons;
        self
    }

    /// Sets the contractual amortization structure.
    ///
    /// Defaults to French level initial instalments. Bullet repays all principal
    /// at maturity; balloon leaves a fraction of original principal for maturity.
    /// Principal is calculated at construction, before prepayment or credit.
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

    /// Sets the payment frequency, independently of resets.
    ///
    /// Defaults to monthly. Accrual dates are generated forward without calendar
    /// adjustment; only payment dates use the chosen calendar and convention.
    /// Frequencies without a positive period are rejected during construction.
    #[must_use]
    pub const fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the reset frequency, independently of index tenor.
    ///
    /// Defaults to the index's metadata frequency, or the payment frequency for
    /// daily indices. Each reset must fall on a payment accrual boundary; several
    /// coupons may share a reset and its full index observation interval.
    #[must_use]
    pub const fn with_coupon_reset_frequency(mut self, frequency: Frequency) -> Self {
        self.fixing_frequency = Some(frequency);
        self
    }

    /// Sets the interest accrual day counter.
    ///
    /// Defaults to Actual/365. Used for coupon interest and the initial contractual
    /// amortization calculation; prepayment conversion uses its own interval rule.
    #[must_use]
    pub const fn with_day_counter(mut self, day_counter: DayCounter) -> Self {
        self.day_counter = Some(day_counter);
        self
    }

    /// Attaches the conditional prepayment configuration, checked during build.
    ///
    /// Defaults to no prepayment. The pricer applies the function after scheduled
    /// principal; the generated contractual cashflows do not include prepayments.
    /// A function supplied by [`Self::with_custom_prepayment_model`] takes precedence.
    #[must_use]
    pub fn with_prepayment_configuration(
        mut self,
        prepayment: PrepaymentModelConfiguration,
    ) -> Self {
        self.prepayment = Some(prepayment);
        self
    }

    /// Sets the credit survival curve index.
    ///
    /// Must be a `MarketIndex::Credit` variant. Without an index, valuation
    /// assumes no default; setting one does not change contractual amortization.
    #[must_use]
    pub fn with_credit_curve_index(mut self, credit_index: MarketIndex) -> Self {
        self.credit_index = Some(credit_index);
        self
    }

    /// Sets the fraction of principal recovered on default.
    ///
    /// Defaults to 0.4; construction requires a finite fraction in [0, 1].
    /// The pricer applies recovery to opening principal at the payment date.
    #[must_use]
    pub const fn with_recovery_fraction(mut self, recovery: f64) -> Self {
        self.recovery = Some(recovery);
        self
    }

    /// Sets the business-day fixing lag.
    ///
    /// Defaults to zero; must be nonnegative. The builder moves backwards from
    /// each reset date using the selected calendar and the Preceding convention.
    #[must_use]
    pub const fn with_fixing_lag_business_days(mut self, fixing_lag: i32) -> Self {
        self.fixing_lag = Some(fixing_lag);
        self
    }

    /// Sets the forward-rate tenor.
    ///
    /// Defaults to the index's metadata frequency and must have positive length.
    /// This observation period is independent of payment and reset frequencies;
    /// its start/end dates use `ModifiedFollowing` with the selected calendar.
    #[must_use]
    pub const fn with_index_observation_tenor(mut self, index_tenor: Period) -> Self {
        self.index_tenor = Some(index_tenor);
        self
    }

    /// Sets the payment and fixing calendar.
    ///
    /// Defaults to a null calendar. Contractual accrual boundaries stay unadjusted.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Sets the payment date adjustment convention.
    ///
    /// Defaults to Unadjusted. This affects payment dates, including the final
    /// payment, without moving accrual boundaries or changing fixing conventions.
    #[must_use]
    pub const fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Initial all-in rate used only to generate contractual amortization.
    ///
    /// An annual decimal rate. Required for floating-first French/balloon loans;
    /// otherwise the first fixed coupon supplies the rate for all principal
    /// periods, including later coupon steps. Market forecasts do not replace it.
    #[must_use]
    pub const fn with_initial_amortization_rate(mut self, rate: f64) -> Self {
        self.amortization_rate = Some(rate);
        self
    }

    /// Attaches an arbitrary conditional prepayment function for the pricer.
    ///
    /// Overrides the portable configuration and is omitted from serialization.
    /// The function must return a finite interval probability in [0, 1]. A custom
    /// model needing a refinancing observation must implement
    /// [`PrepaymentModel::requires_refinancing_benchmark`]; a closure uses the default `false`.
    #[must_use]
    pub fn with_custom_prepayment_model(
        mut self,
        function: impl PrepaymentModel<DualFwd> + 'static,
    ) -> Self {
        self.prepayment_function = Some(Arc::new(function));
        self
    }

    /// Builds contractual cashflows from positional coupon templates.
    ///
    /// The last coupon repeats through maturity. Each contiguous run of the
    /// same floating index has its own reset grid; spread or cap/floor changes
    /// keep that grid. Principal uses the explicit initial amortization rate,
    /// or the first fixed coupon's rate when available, and stays contractual.
    ///
    /// # Errors
    /// Rejects missing/invalid terms, empty or oversized coupon lists, non-coupon
    /// entries, unaligned resets, and amortizing floating-first schedules without
    /// an explicit initial amortization rate.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Loan<T>> {
        let identifier = require_loan_term(self.identifier, "Loan identifier")?;
        let start = require_loan_term(self.start_date, "Loan start date")?;
        let end = require_loan_term(self.maturity_date, "Loan maturity date")?;
        let notional = require_loan_term(self.notional, "Loan notional")?;
        let currency = require_loan_term(self.currency, "Loan currency")?;
        let repayment = self.repayment.unwrap_or(LoanRepaymentStructure::French);
        if identifier.trim().is_empty() || start >= end || !notional.is_finite() || notional <= 0.0
        {
            return Err(QSError::InvalidValueErr(
                "Invalid loan dates or principal".into(),
            ));
        }
        let payment_frequency = self.payment_frequency.unwrap_or(Frequency::Monthly);
        let payment_period = require_positive_schedule_period(payment_frequency)?;
        if let Some(frequency) = self.fixing_frequency {
            require_positive_schedule_period(frequency)?;
        }
        if self.index_tenor.is_some_and(|tenor| tenor.length() <= 0) {
            return Err(QSError::InvalidValueErr(
                "Loan index tenor must be positive".into(),
            ));
        }
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
        let schedule = MakeSchedule::new(start, end)
            .with_tenor(payment_period)
            .with_convention(BusinessDayConvention::Unadjusted)
            .with_termination_date_convention(BusinessDayConvention::Unadjusted)
            .forwards()
            .build()?;
        let dates = schedule.dates();
        let period_count = dates.len().saturating_sub(1);
        let last_coupon = self.coupons.last().ok_or_else(|| {
            QSError::InvalidValueErr("Loan requires at least one coupon definition".into())
        })?;
        if period_count == 0 || self.coupons.len() > period_count {
            return Err(QSError::InvalidValueErr(format!(
                "Loan coupon list has {} entries for {period_count} payment periods",
                self.coupons.len()
            )));
        }
        for coupon in &self.coupons {
            coupon.validate_coupon()?;
        }
        let coupons = (0..period_count)
            .map(|position| self.coupons.get(position).unwrap_or(last_coupon))
            .collect::<Vec<_>>();
        let amortization_rate = match (coupons[0], self.amortization_rate) {
            (_, Some(rate)) => rate,
            (CashflowType::FixedRateCoupon(coupon), None) => coupon.rate().rate().value(),
            (_, None) if matches!(repayment, LoanRepaymentStructure::Bullet) => 0.0,
            _ => {
                return Err(QSError::ValueNotSetErr(
                    "Floating-first French/balloon loans require an initial amortization_rate"
                        .into(),
                ))
            }
        };
        let period_rates = dates
            .windows(2)
            .map(|period| amortization_rate * day_counter.year_fraction(period[0], period[1]))
            .collect::<Vec<_>>();
        let notionals =
            calculate_contractual_opening_balances(notional, &period_rates, &repayment)?;
        let fixed_definition = self
            .day_counter
            .map(|basis| RateDefinition::new(basis, Compounding::Simple, Frequency::Annual));
        let mut cashflows = vec![CashflowType::Disbursement(SimpleCashflow::new(
            notional, start,
        ))];
        let mut observations = Vec::with_capacity(period_count);
        let mut previous_index = None;
        let mut reset_dates = Vec::new();
        for (position, period) in dates.windows(2).enumerate() {
            let accrual_start = period[0];
            let accrual_end = period[1];
            let payment_date = calendar.adjust(accrual_end, Some(convention));
            let index = coupon_projection_index(coupons[position]);
            let observation = if let Some(index) = index {
                let index_frequency = index.rate_index_details()?.frequency();
                if previous_index.as_ref() != Some(index) {
                    let segment_end_position = (position + 1..period_count)
                        .find(|next| coupon_projection_index(coupons[*next]) != Some(index))
                        .unwrap_or(period_count);
                    let fixing_frequency =
                        self.fixing_frequency
                            .unwrap_or(if index_frequency == Frequency::Daily {
                                payment_frequency
                            } else {
                                index_frequency
                            });
                    reset_dates.clone_from(
                        MakeSchedule::new(accrual_start, dates[segment_end_position])
                            .with_tenor(require_positive_schedule_period(fixing_frequency)?)
                            .with_convention(BusinessDayConvention::Unadjusted)
                            .with_termination_date_convention(BusinessDayConvention::Unadjusted)
                            .forwards()
                            .build()?
                            .dates(),
                    );
                    if reset_dates
                        .iter()
                        .any(|date| dates.binary_search(date).is_err())
                    {
                        return Err(QSError::InvalidValueErr(
                            "Loan resets must align with payment accrual boundaries".into(),
                        ));
                    }
                }
                let reset_date = reset_dates
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
                let index_tenor = self
                    .index_tenor
                    .unwrap_or(require_positive_schedule_period(index_frequency)?);
                let index_end = calendar.advance(
                    index_start,
                    index_tenor,
                    Some(BusinessDayConvention::ModifiedFollowing),
                    false,
                );
                Some(
                    ForwardRateRequest::new(index.clone(), fixing_date)
                        .with_start_date(index_start)
                        .with_end_date(index_end),
                )
            } else {
                None
            };
            observations.push(observation);
            previous_index = index.cloned();
            let coupon = coupons[position].for_period(
                notionals[position],
                accrual_start,
                accrual_end,
                payment_date,
                fixed_definition,
            )?;
            cashflows.push(match coupon {
                CashflowType::FloatingRateCoupon(coupon) => {
                    CashflowType::FloatingRateCoupon(coupon.with_day_counter(day_counter))
                }
                CashflowType::OptionEmbeddedCoupon(coupon) => {
                    CashflowType::OptionEmbeddedCoupon(coupon.with_day_counter(day_counter))
                }
                coupon => coupon,
            });
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
        prepayment.validate_parameters()?;
        let prepayment = self
            .prepayment_function
            .unwrap_or_else(|| Arc::new(prepayment));
        let indices = coupons
            .iter()
            .filter_map(|coupon| coupon_projection_index(coupon).cloned())
            .collect::<HashSet<_>>();
        let forward_index = if indices.len() == 1 {
            indices.into_iter().next()
        } else {
            None
        };
        let last_payment = calendar.adjust(end, Some(convention));
        let leg = Leg::new(
            0,
            cashflows,
            currency,
            None,
            forward_index,
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

/// Returns the projection index for floating or bounded-floating coupon templates.
const fn coupon_projection_index<T: Scalar>(coupon: &CashflowType<T>) -> Option<&MarketIndex> {
    match coupon {
        CashflowType::FloatingRateCoupon(coupon) => Some(coupon.market_index()),
        CashflowType::OptionEmbeddedCoupon(coupon) => Some(coupon.market_index()),
        _ => None,
    }
}

/// Extracts a mandatory construction term, naming it in a missing-value error.
///
/// # Errors
/// Returns `ValueNotSetErr` when `value` is `None`.
fn require_loan_term<V>(value: Option<V>, name: &str) -> Result<V> {
    value.ok_or_else(|| QSError::ValueNotSetErr(name.into()))
}
/// Converts a schedule frequency into a strictly positive calendar period.
///
/// # Errors
/// Rejects frequencies that have no period or convert to a nonpositive length.
fn require_positive_schedule_period(frequency: Frequency) -> Result<Period> {
    Period::from_frequency(frequency)
        .filter(|period| period.length() > 0)
        .ok_or_else(|| QSError::InvalidValueErr("Unsupported loan frequency".into()))
}

/// Computes one opening contractual principal balance per accrual interval.
///
/// `period_rates` contains annual initial all-in rate times each interval's
/// accrual fraction. French/balloon instalments use cumulative annuity discount
/// factors, reserving the balloon fraction of original principal for maturity.
/// Bullet returns the original principal for every interval. The final balance
/// is redeemed in full by the caller. All arithmetic is `f64`, independent of AD
/// and market discount curves.
///
/// # Errors
/// Rejects empty schedules, nonpositive/nonfinite principal, nonfinite period
/// rates or rates at/below -1, invalid balloon fractions, invalid annuity factors,
/// and principal schedules that become nonpositive or increase.
pub(crate) fn calculate_contractual_opening_balances(
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

    fn default_loan_builder() -> MakeLoan<f64> {
        MakeLoan::default()
            .with_identifier("LOAN".into())
            .with_start_date(Date::new(2026, 1, 1))
            .with_maturity_date(Date::new(2028, 1, 1))
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_coupons(vec![CashflowType::fixed(0.06)])
    }

    #[test]
    fn coupon_lists_repeat_the_last_entry_and_reject_invalid_shapes() -> Result<()> {
        let loan = default_loan_builder()
            .with_coupons(vec![CashflowType::fixed(0.03), CashflowType::fixed(0.05)])
            .build()?;
        let rates = loan
            .contractual_leg()
            .cashflows()
            .iter()
            .filter_map(|cashflow| {
                if let CashflowType::FixedRateCoupon(coupon) = cashflow {
                    Some(coupon.rate().rate())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(rates.len(), 24);
        assert_eq!(rates[0], 0.03);
        assert!(rates[1..].iter().all(|rate| (*rate - 0.05).abs() < 1e-14));
        assert!(default_loan_builder().with_coupons(vec![]).build().is_err());
        assert!(default_loan_builder()
            .with_coupons(vec![CashflowType::fixed(0.03); 25])
            .build()
            .is_err());
        assert!(default_loan_builder()
            .with_coupons(vec![CashflowType::Redemption(SimpleCashflow::new(
                1.0,
                Date::new(2026, 1, 1)
            ))])
            .build()
            .is_err());
        Ok(())
    }

    #[test]
    fn coupon_index_transitions_start_independent_reset_grids() -> Result<()> {
        let mut coupons = vec![CashflowType::fixed(0.03); 6];
        coupons.push(CashflowType::floating(
            MarketIndex::EURIBOR12m,
            0.01,
            None,
            None,
        ));
        coupons.push(CashflowType::floating(
            MarketIndex::EURIBOR12m,
            0.02,
            None,
            None,
        ));
        coupons.extend(vec![
            CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None
            );
            4
        ]);
        coupons.push(CashflowType::floating(
            MarketIndex::EURIBOR3m,
            0.005,
            None,
            None,
        ));
        let loan = default_loan_builder().with_coupons(coupons).build()?;
        let observations = loan.coupon_rate_observations();
        assert!(observations[..6].iter().all(Option::is_none));
        let annual = observations[6]
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Annual observation".into()))?;
        assert_eq!(annual.market_index(), MarketIndex::EURIBOR12m);
        assert_eq!(annual.start_date(), Some(Date::new(2026, 7, 1)));
        assert_eq!(annual.end_date(), Some(Date::new(2027, 7, 1)));
        assert_eq!(
            observations[7]
                .as_ref()
                .map(ForwardRateRequest::fixing_date),
            Some(annual.fixing_date())
        );
        let quarterly = observations[12]
            .as_ref()
            .ok_or_else(|| QSError::ValueNotSetErr("Quarterly observation".into()))?;
        assert_eq!(quarterly.market_index(), MarketIndex::EURIBOR3m);
        assert_eq!(quarterly.start_date(), Some(Date::new(2027, 1, 1)));
        assert_eq!(quarterly.end_date(), Some(Date::new(2027, 4, 1)));
        assert_eq!(loan.contractual_leg().forward_index(), None);
        Ok(())
    }
    #[test]
    fn french_contract_uses_existing_cashflows_and_matches_annuity() -> Result<()> {
        let loan = default_loan_builder()
            .with_maturity_date(Date::new(2056, 1, 1))
            .with_day_counter(DayCounter::Thirty360)
            .build()?;
        let amounts = loan
            .contractual_leg()
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
            .contractual_leg()
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
            let loan = default_loan_builder()
                .with_coupons(vec![CashflowType::floating(index, 0.01, None, None)])
                .with_initial_amortization_rate(0.05)
                .build()?;
            let requests: Vec<_> = loan.coupon_rate_observations().iter().flatten().collect();
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
        let plain = default_loan_builder().build()?;
        let prepaying = default_loan_builder()
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(1.0))
            .build()?;
        let principals = |loan: &Loan<f64>| {
            loan.contractual_leg()
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
            loan.contractual_leg()
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
            let loan = default_loan_builder()
                .with_coupons(vec![CashflowType::fixed(0.0)])
                .with_repayment_structure(structure)
                .with_maturity_date(Date::new(2027, 1, 15))
                .build()?;
            let principal: f64 = loan
                .contractual_leg()
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
        assert!(default_loan_builder().with_notional(-1.0).build().is_err());
        assert!(default_loan_builder()
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None
            )])
            .build()
            .is_err());
        assert!(default_loan_builder()
            .with_coupon_reset_frequency(Frequency::Once)
            .build()
            .is_err());
        assert!(default_loan_builder()
            .with_credit_curve_index(MarketIndex::ESTR)
            .build()
            .is_err());
        assert!(default_loan_builder()
            .with_prepayment_configuration(PrepaymentModelConfiguration::Cpr(1.1))
            .build()
            .is_err());
        assert!(default_loan_builder()
            .with_repayment_structure(LoanRepaymentStructure::Balloon {
                residual_fraction: 1.1
            })
            .build()
            .is_err());
    }
    #[test]
    fn generic_constructor_accepts_custom_contract_without_builder_metadata() -> Result<()> {
        let loan = default_loan_builder().build()?;
        let custom = Loan::new(
            "CUSTOM".into(),
            loan.notional(),
            loan.contractual_leg().clone(),
            loan.coupon_rate_observations().to_vec(),
            Arc::new(PrepaymentModelConfiguration::None),
            None,
            None,
            0.25,
        )?;
        assert!(custom.repayment_structure().is_none());
        assert_eq!(
            custom.contractual_leg().cashflows().len(),
            loan.contractual_leg().cashflows().len()
        );
        Ok(())
    }
}
