//! Explicit construction of concrete products from reusable makers.

use crate::{
    core::marketdatahandling::marketdata::MarketDataProvider,
    time::{
        calendars::traits::IsCalendar, date::Date, enums::BusinessDayConvention, period::Period,
    },
    utils::errors::{QSError, Result},
};

/// Builds a concrete product from contractual parameters, optionally using a market.
///
/// The construction methods borrow the maker and return the same concrete type. A built
/// product retains its dates and contractual terms when valued in another market.
pub trait MakeInstrument: Sized {
    /// Concrete product produced by this maker, including cashflow legs.
    type Instrument;

    /// Whether date inputs and required market observations are already resolved.
    ///
    /// This checks dependencies, not contractual validity. Missing or invalid
    /// contract fields are reported by `build`.
    fn is_resolved(&self) -> bool;

    /// Returns a maker with date tenors resolved against the supplied reference date.
    ///
    /// The original maker is unchanged. Product-specific calendars, conventions,
    /// and date anchors are retained. This resolves dates only; a maker may still
    /// need market observations before [`Self::build`] can succeed.
    ///
    /// # Errors
    /// Propagates missing or conflicting date inputs, invalid conventions, and
    /// date advancement errors.
    fn resolve_dates(&self, reference_date: Date) -> Result<Self>;

    /// Builds using only the maker's supplied parameters.
    ///
    /// # Errors
    /// Returns `QSError::UnresolvedProductErr` for unresolved date inputs or market
    /// observations; supply concrete terms or call `build_using(context)`.
    /// Missing or invalid parameters retain their own
    /// error categories.
    fn build(&self) -> Result<Self::Instrument>;

    /// Builds using the context's evaluation date, fixing policy, and market data.
    ///
    /// The default resolves dates once and calls [`Self::build`]. Makers that
    /// require market observations override this method to resolve them as well.
    ///
    /// # Errors
    /// Propagates invalid parameters, missing market data, and construction errors.
    fn build_using(&self, context: &dyn MarketDataProvider) -> Result<Self::Instrument> {
        self.resolve_dates(context.evaluation_date())?.build()
    }
}

/// Selects an absolute date or advances the supplied anchor using the calendar.
///
/// # Errors
/// Rejects missing, conflicting, or empty dates and offsets outside the supported range.
pub fn resolve_date(
    date: Option<Date>,
    tenor: Option<Period>,
    anchor: Date,
    label: &str,
    calendar: &impl IsCalendar,
    convention: BusinessDayConvention,
    end_of_month: bool,
) -> Result<Date> {
    match (date, tenor) {
        (Some(_), Some(_)) => Err(QSError::InvalidValueErr(format!(
            "Supply a date or a tenor for {label}, not both"
        ))),
        (Some(date), None) if date != Date::empty() => Ok(date),
        (Some(_), None) => Err(QSError::InvalidValueErr(format!(
            "{label} must not be empty"
        ))),
        (None, Some(tenor)) => calendar.advance(anchor, tenor, Some(convention), end_of_month),
        (None, None) => Err(QSError::ValueNotSetErr(label.into())),
    }
}

/// Shared fixtures for instrument construction tests.
#[cfg(test)]
pub mod test_support {
    use crate::prelude::*;
    use std::{cell::RefCell, rc::Rc};

    /// Parses a tenor used by resolution tests.
    ///
    /// # Errors
    /// Rejects malformed tenor strings.
    pub fn p(text: &str) -> Result<Period> {
        Period::from_str(text)
    }

    /// Common market date used by resolution tests.
    #[must_use]
    pub fn reference() -> Date {
        Date::new(2026, 2, 24)
    }

    /// Creates labelled EUR discount and projection curves with supplied fixings.
    #[must_use]
    pub fn market(date: Date, forward: f64, fixings: FixingStore) -> PricingContext {
        let mut elements = ConstructedElementStore::default();
        for (index, rate) in [
            (MarketIndex::ESTR, 0.03),
            (MarketIndex::EURIBOR12m, forward),
        ] {
            let curve = FlatForwardTermStructure::new(
                date,
                DualFwd::from(rate),
                RateDefinition::new(
                    DayCounter::Actual360,
                    Compounding::Simple,
                    Frequency::Annual,
                ),
            )
            .with_pillar_label(index.to_string());
            elements.discount_curves_mut().insert(
                index.clone(),
                DiscountCurveElement::new(index, Rc::new(RefCell::new(curve))),
            );
        }
        PricingContext::new()
            .with_quote_store(QuoteStore::new(date))
            .with_constructed_elements(elements)
            .with_fixing_store(fixings)
            .with_base_currency(Currency::EUR)
            .with_base_index(MarketIndex::ESTR)
    }

    /// Creates a date-only market for resolving product schedules.
    #[must_use]
    pub fn empty_market(date: Date) -> PricingContext {
        PricingContext::new().with_quote_store(QuoteStore::new(date))
    }

    /// Captures contractual dates, notionals and rates for schedule comparisons.
    ///
    /// # Errors
    /// Propagates invalid cashflow amounts and rejects standalone embedded options.
    pub fn cashflows<T: Scalar>(leg: &Leg<T>) -> Result<Vec<String>> {
        leg.cashflows()
            .iter()
            .map(|flow| {
                Ok(match flow {
                    CashflowType::FixedRateCoupon(c) => format!(
                        "fixed:{}:{}:{}:{}:{}",
                        c.accrual_start_date(),
                        c.accrual_end_date(),
                        c.payment_date(),
                        c.notional(),
                        c.rate().rate().value()
                    ),
                    CashflowType::FloatingRateCoupon(c) => format!(
                        "float:{}:{}:{}:{}:{}:{}",
                        c.accrual_start_date(),
                        c.accrual_end_date(),
                        c.payment_date(),
                        c.notional(),
                        c.market_index(),
                        c.spread().value()
                    ),
                    CashflowType::OptionEmbeddedCoupon(c) => format!(
                        "option:{}:{}:{}:{}:{}",
                        c.accrual_start_date(),
                        c.accrual_end_date(),
                        c.payment_date(),
                        c.notional(),
                        c.spread().value()
                    ),
                    CashflowType::Redemption(c) => {
                        format!("redemption:{}:{}", c.payment_date(), c.amount()?)
                    }
                    CashflowType::Disbursement(c) => {
                        format!("disbursement:{}:{}", c.payment_date(), c.amount()?)
                    }
                    CashflowType::ConstantAmount(c) => {
                        format!("constant:{}:{}", c.payment_date(), c.amount()?)
                    }
                    CashflowType::OptionEmbeddedCashflow(_) => {
                        return Err(QSError::InvalidValueErr(
                            "unexpected standalone option".into(),
                        ))
                    }
                })
            })
            .collect()
    }

    /// Compares every contractual cashflow in two resolved products.
    ///
    /// # Errors
    /// Propagates errors while reading contractual cashflows.
    ///
    /// # Panics
    /// Panics when the legs or their contractual cashflows differ.
    pub fn assert_legs<T: Scalar>(
        a: &impl LegsProvider<T>,
        b: &impl LegsProvider<T>,
    ) -> Result<()> {
        assert_eq!(a.legs().len(), b.legs().len());
        for (left, right) in a.legs().iter().zip(b.legs()) {
            assert_eq!(cashflows(left)?, cashflows(right)?);
        }
        Ok(())
    }

    /// Defines a relative-date floating French loan without an initial rate override.
    ///
    /// # Errors
    /// Propagates errors constructing the fixture's tenor inputs.
    pub fn loan() -> Result<MakeLoan<DualFwd>> {
        Ok(MakeLoan::default()
            .with_identifier("loan".into())
            .with_start_tenor(p("0D")?)
            .with_maturity_tenor(p("2Y")?)
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_coupons(vec![CashflowType::floating(
                MarketIndex::EURIBOR12m,
                0.01,
                None,
                None,
            )]))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::makeinstrument::test_support::{cashflows, empty_market, loan, market, reference},
        prelude::*,
    };

    fn build_product<M: MakeInstrument>(
        maker: &M,
        context: &dyn MarketDataProvider,
    ) -> Result<M::Instrument> {
        maker.build_using(context)
    }

    #[test]
    fn build_requires_resolved_dates_and_makers_are_reusable() -> Result<()> {
        let relative = MakeFixedRateDeposit::<f64>::default()
            .with_identifier("deposit".into())
            .with_start_tenor(Period::new(0, TimeUnit::Days))
            .with_tenor(Period::new(1, TimeUnit::Years))
            .with_currency(Currency::EUR)
            .with_notional(100.0)
            .with_rate(0.03)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Simple,
                Frequency::Annual,
            ));
        assert!(!relative.is_resolved());
        assert!(matches!(
            MakeInstrument::build(&relative),
            Err(QSError::UnresolvedProductErr(_))
        ));
        let today = empty_market(reference());
        let first = build_product(&relative, &today)?;
        let later_date = reference() + Period::new(1, TimeUnit::Years);
        let later = empty_market(later_date);
        let second = build_product(&relative, &later)?;
        assert_eq!(first.start_date(), reference());
        assert_eq!(second.start_date(), later_date);
        assert_eq!(build_product(&relative, &today)?.start_date(), reference());
        let dated = MakeInstrument::resolve_dates(&relative, reference())?;
        assert!(dated.is_resolved());
        assert_eq!(dated.build()?.start_date(), reference());
        assert_eq!(dated.build_using(&later)?.start_date(), reference());
        assert!(!relative.is_resolved());
        let absolute = relative.with_start_date(reference());
        assert!(!absolute.is_resolved());
        assert!(matches!(
            absolute.build(),
            Err(QSError::UnresolvedProductErr(_))
        ));
        let absolute = absolute.with_maturity_date(reference() + Period::new(1, TimeUnit::Years));
        assert!(absolute.is_resolved());
        assert_eq!(
            absolute.build()?.maturity_date(),
            reference() + Period::new(1, TimeUnit::Years)
        );
        let missing = MakeFixedRateDeposit::<f64>::default()
            .with_start_date(reference())
            .with_maturity_date(reference() + Period::new(1, TimeUnit::Years));
        assert!(matches!(missing.build(), Err(QSError::ValueNotSetErr(_))));
        Ok(())
    }

    #[test]
    fn dependent_date_tenors_require_build_using() {
        fn assert_unresolved(maker: &impl MakeInstrument) {
            assert!(!maker.is_resolved());
            let Err(error) = maker.build() else {
                panic!("unresolved date inputs must not build");
            };
            assert!(matches!(error, QSError::UnresolvedProductErr(_)));
            assert!(error.to_string().contains("build_using"));
        }

        let tenor = Period::new(3, TimeUnit::Months);
        assert_unresolved(&MakeFixedRateBond::<f64>::default().with_first_coupon_tenor(tenor));
        assert_unresolved(&MakeFloatingRateNote::<f64>::default().with_first_coupon_tenor(tenor));
        assert_unresolved(
            &MakeLeg::<f64>::default().with_disbursements_tenors(vec![(tenor, 100.0)]),
        );
        assert_unresolved(
            &MakeSteppedCouponBond::<f64>::default()
                .with_call_schedule_tenors(vec![(tenor, 100.0)]),
        );
        assert_unresolved(&MakeLoan::<f64>::default().with_prepayment_tenors(vec![(tenor, 0.1)]));
        assert_unresolved(&MakeCapletFloorlet::default().with_payment_tenor(tenor));
        assert_unresolved(&MakeSwaption::<f64>::default().with_swap_tenor(tenor));
        assert_unresolved(
            &MakeOvernightIndexSwap::<f64>::new(
                MarketIndex::ESTR,
                reference(),
                Period::new(1, TimeUnit::Years),
                0.03,
            )
            .with_first_coupon_tenor(tenor),
        );
        assert_unresolved(
            &MakeScriptedProduct::default()
                .with_reference_date(reference())
                .with_event_tenors(vec![(tenor, "value = 0;".into())]),
        );
    }

    #[test]
    fn floating_amortization_errors_at_build_and_stays_fixed_after_construction() -> Result<()> {
        let maker = MakeInstrument::resolve_dates(&loan()?, reference())?;
        assert!(!maker.is_resolved());
        let error = MakeInstrument::build(&maker).err().ok_or_else(|| {
            QSError::UnexpectedErr(
                "floating amortization unexpectedly built without a context".into(),
            )
        })?;
        assert!(matches!(
            error.root_cause(),
            QSError::UnresolvedProductErr(_)
        ));
        assert_eq!(error.code(), "unresolved_product");
        let low = market(reference(), 0.02, FixingStore::default());
        let high = market(reference(), 0.08, FixingStore::default());
        let first = build_product(&maker, &low)?;
        let second = build_product(&maker, &high)?;
        assert_ne!(
            first.initial_amortization_rate(),
            second.initial_amortization_rate()
        );
        assert_eq!(
            first.initial_amortization_rate(),
            build_product(&maker, &low)?.initial_amortization_rate()
        );
        let explicit = maker
            .clone()
            .with_initial_amortization_rate(
                first
                    .initial_amortization_rate()
                    .ok_or_else(|| QSError::ValueNotSetErr("initial rate".into()))?,
            )
            .build()?;
        assert!(maker
            .clone()
            .with_initial_amortization_rate(0.03)
            .is_resolved());
        assert!(maker
            .clone()
            .with_repayment_structure(LoanRepaymentStructure::Bullet)
            .is_resolved());
        let coupon = FloatingRateCoupon::new(
            1.0,
            0.01,
            MarketIndex::EURIBOR12m,
            reference(),
            reference(),
            reference(),
        );
        coupon.set_fixing(0.02);
        let fixed_observation =
            maker.with_coupons(vec![CashflowType::FloatingRateCoupon(coupon.into())]);
        assert!(fixed_observation.is_resolved());
        assert!(fixed_observation.build().is_ok());
        assert_eq!(
            cashflows(first.contractual_leg())?,
            cashflows(explicit.contractual_leg())?
        );
        Ok(())
    }
}
