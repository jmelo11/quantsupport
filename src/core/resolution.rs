//! Resolving maker inputs into contracts when a pricing market is available.
//!
//! A maker such as `MakeSwap` stores requested contract terms: for example a
//! start offset of `0D` and a maturity tenor of `5Y`. Those inputs can be created
//! before any valuation date exists. This module supplies the reference date,
//! calendar rules and market observations needed to turn the maker into a
//! concrete instrument with actual dates and principal amounts.
//!
//! `Resolve::resolve` does that conversion without modifying the maker. A trade
//! can retain the maker through `UnresolvedTrade` and defer this conversion until
//! evaluation. A resolved instrument instead retains its contractual terms.
//!
//! Makers retain `Date` and `Period` inputs in separate fields. Resolution
//! produces ordinary concrete instruments; pricing and risk then use that
//! snapshot without changing the maker or consulting a process-wide date.

use chrono::{Days, Months, NaiveDate};

use crate::{
    core::marketdatahandling::{
        constructedelementrequest::ConstructedElementRequest,
        fixingrequest::FixingRequest,
        forwardraterequest::ForwardRateRequest,
        marketdata::{MarketDataProvider, MarketDataRequest},
    },
    time::{
        calendar::Calendar,
        calendars::traits::IsCalendar,
        date::Date,
        enums::{BusinessDayConvention, TimeUnit},
        period::Period,
    },
    utils::errors::{QSError, Result},
};

/// Converts a maker or pending trade into its concrete instrument or trade.
///
/// For example, resolving a `MakeSwap` with `with_start_tenor(0D)` and
/// `with_tenor(5Y)` supplies actual schedule dates from the context. Resolving a
/// floating `MakeLoan` can also obtain its initial coupon fixing or forecast.
/// The input remains unchanged, so it can be evaluated for another market date.
pub trait Resolve {
    /// Concrete instrument, leg, event stream, or other resolved definition.
    type Resolved;

    /// Resolves dates and observations without changing the definition.
    ///
    /// # Errors
    /// Returns an error for incomplete terms, invalid dates, missing required
    /// observations, or construction errors in the resolved product.
    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<Self::Resolved>;
}

/// Inputs used to turn a maker's requested terms into a concrete contract.
///
/// This component joins the maker to a pricing market: the reference date anchors
/// relative tenors; the provider supplies fixings and curves; date conventions
/// control business-day offsets and adjustments. It does not price instruments.
///
/// The reference date defaults to the provider's evaluation date. An explicit
/// anchor can be used for a booked contract; historical observations are still
/// determined relative to the provider's evaluation date. There is no mutable
/// instrument cache, so a definition can be reused across contexts.
#[derive(Clone, Copy)]
pub struct ResolutionContext<'a> {
    market: &'a dyn MarketDataProvider,
    reference_date: Date,
    calendar: Option<&'a Calendar>,
    convention: BusinessDayConvention,
    end_of_month: bool,
    anchor_fixings: bool,
}

impl<'a> ResolutionContext<'a> {
    /// Uses the market provider's evaluation date as the reference date.
    #[must_use]
    pub fn new(market: &'a dyn MarketDataProvider) -> Self {
        Self {
            market,
            reference_date: market.evaluation_date(),
            calendar: None,
            convention: BusinessDayConvention::Unadjusted,
            end_of_month: false,
            anchor_fixings: market.anchor_fixings(),
        }
    }

    /// Overrides historical fixing anchoring for this instrument resolution.
    /// True requires past fixings; false projects all dates from the curve.
    #[must_use]
    pub const fn with_anchor_fixings(mut self, enabled: bool) -> Self {
        self.anchor_fixings = enabled;
        self
    }

    /// Whether past observations are anchored to stored fixings.
    #[must_use]
    pub const fn anchor_fixings(&self) -> bool {
        self.anchor_fixings
    }

    /// Overrides the date against which reference-relative tenors are resolved.
    #[must_use]
    pub const fn with_reference_date(mut self, date: Date) -> Self {
        self.reference_date = date;
        self
    }

    /// Selects the product's date conventions for relative inputs only.
    ///
    /// Supplied absolute dates are retained. Day tenors count business days
    /// when a calendar is supplied; other tenors use calendar arithmetic and
    /// the chosen adjustment. Without a calendar, days are calendar days.
    #[must_use]
    pub const fn with_date_conventions<'b>(
        self,
        calendar: &'b Calendar,
        convention: BusinessDayConvention,
        end_of_month: bool,
    ) -> ResolutionContext<'b>
    where
        'a: 'b,
    {
        ResolutionContext {
            market: self.market,
            reference_date: self.reference_date,
            anchor_fixings: self.anchor_fixings,
            calendar: Some(calendar),
            convention,
            end_of_month,
        }
    }

    /// Date used to anchor reference-relative inputs.
    #[must_use]
    pub const fn reference_date(&self) -> Date {
        self.reference_date
    }

    /// Provider used for fixing and forecast requests during resolution.
    #[must_use]
    pub const fn market_data_provider(&self) -> &'a dyn MarketDataProvider {
        self.market
    }

    /// Resolves a definition with this context.
    ///
    /// # Errors
    /// Propagates the definition's resolution errors.
    pub fn resolve<R: Resolve>(&self, definition: &R) -> Result<R::Resolved> {
        definition.resolve(self)
    }

    /// Resolves a required date or tenor measured from the supplied anchor.
    ///
    /// # Errors
    /// Rejects missing or conflicting inputs and date arithmetic outside the
    /// supported range. Absolute dates are not adjusted a second time.
    pub fn resolve_date(
        &self,
        date: Option<Date>,
        tenor: Option<Period>,
        anchor: Date,
        label: &str,
    ) -> Result<Date> {
        match (date, tenor) {
            (Some(_), Some(_)) => Err(QSError::InvalidValueErr(format!(
                "Supply a date or a tenor for {label}, not both"
            ))),
            (Some(date), None) => Ok(date),
            (None, Some(tenor)) => self.advance_date(anchor, tenor),
            (None, None) => Err(QSError::ValueNotSetErr(label.into())),
        }
    }

    /// Advances an anchor with checked arithmetic and this context's conventions.
    ///
    /// # Errors
    /// Rejects an empty anchor or an offset outside the supported date range.
    pub fn advance_date(&self, anchor: Date, tenor: Period) -> Result<Date> {
        if anchor == Date::empty() {
            return Err(QSError::InvalidValueErr("Empty resolution anchor".into()));
        }
        let base = NaiveDate::from_ymd_opt(anchor.year(), anchor.month(), anchor.day())
            .ok_or_else(date_range_error)?;
        let count = tenor.length().unsigned_abs();
        let forward = tenor.length() >= 0;
        let mut shifted = match tenor.units() {
            TimeUnit::Days if self.calendar.is_some() => {
                // Business-day advancement cannot require fewer calendar days.
                // Reject impossible offsets before walking the calendar.
                shift_days(base, u64::from(count), forward)?;
                let mut day = base;
                let mut remaining = count;
                while remaining > 0 {
                    day = shift_days(day, 1, forward)?;
                    if self
                        .calendar
                        .is_some_and(|cal| cal.is_business_day(&Date::from(day)))
                    {
                        remaining -= 1;
                    }
                }
                day
            }
            TimeUnit::Days => shift_days(base, u64::from(count), forward)?,
            TimeUnit::Weeks => shift_days(base, u64::from(count) * 7, forward)?,
            TimeUnit::Months | TimeUnit::Years => {
                let months = if tenor.units() == TimeUnit::Years {
                    count.checked_mul(12).ok_or_else(date_range_error)?
                } else {
                    count
                };
                if forward {
                    base.checked_add_months(Months::new(months))
                } else {
                    base.checked_sub_months(Months::new(months))
                }
                .ok_or_else(date_range_error)?
            }
        };
        if let Some(calendar) = self.calendar {
            let date = Date::from(shifted);
            let adjusted = if self.end_of_month
                && matches!(tenor.units(), TimeUnit::Months | TimeUnit::Years)
                && calendar.is_end_of_month(&anchor)
            {
                calendar.end_of_month(date)
            } else if tenor.units() == TimeUnit::Days && count > 0 {
                date
            } else {
                calendar.adjust(date, Some(self.convention))
            };
            shifted = NaiveDate::from_ymd_opt(adjusted.year(), adjusted.month(), adjusted.day())
                .ok_or_else(date_range_error)?;
        }
        Ok(Date::from(shifted))
    }

    /// Resolves a term-index observation using the provider's fixing policy.
    ///
    /// With anchoring enabled, past observations require stored fixings. Today
    /// and future dates use the curve. With anchoring disabled all dates use it.
    /// The returned value is a contractual scalar, without pricing-tape nodes.
    ///
    /// # Errors
    /// Propagates missing historical fixings, index conventions, curves, and
    /// invalid observation intervals.
    pub fn resolve_index_rate(&self, observation: &ForwardRateRequest) -> Result<f64> {
        let index = observation.market_index();
        let fixing_date = observation.fixing_date();
        let today = self.market.evaluation_date();
        if self.anchor_fixings && fixing_date < today {
            let request = MarketDataRequest::default()
                .with_fixings_request(vec![FixingRequest::new(index.clone(), fixing_date)]);
            let data = self.market.handle_request(&request)?;
            if let Some(value) = data
                .fixings()
                .get(&index)
                .and_then(|dates| dates.get(&fixing_date))
            {
                return Ok(*value);
            }
            if fixing_date < today {
                return Err(QSError::NotFoundErr(format!(
                    "Missing {index} fixing for {fixing_date} during instrument resolution"
                )));
            }
        }
        let request = MarketDataRequest::default().with_constructed_elements_request(vec![
            ConstructedElementRequest::DiscountCurve {
                market_index: index,
            },
        ]);
        let data = self.market.handle_request(&request)?;
        Ok(observation
            .resolve_index_rate(&data, today, self.anchor_fixings)?
            .value())
    }
}

fn date_range_error() -> QSError {
    QSError::InvalidValueErr("Resolved date is outside the supported date range".into())
}

fn shift_days(date: NaiveDate, count: u64, forward: bool) -> Result<NaiveDate> {
    if forward {
        date.checked_add_days(Days::new(count))
    } else {
        date.checked_sub_days(Days::new(count))
    }
    .ok_or_else(date_range_error)
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::prelude::*;
    use std::{cell::RefCell, rc::Rc};

    /// Parses a tenor used by resolution tests.
    pub(crate) fn p(text: &str) -> Result<Period> {
        Period::from_str(text)
    }

    /// Common market date used by resolution tests.
    pub(crate) fn reference() -> Date {
        Date::new(2026, 2, 24)
    }

    /// Creates labelled EUR discount and projection curves with supplied fixings.
    pub(crate) fn market(date: Date, forward: f64, fixings: FixingStore) -> PricingContext {
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
    pub(crate) fn empty_market(date: Date) -> PricingContext {
        PricingContext::new().with_quote_store(QuoteStore::new(date))
    }

    /// Captures contractual dates, notionals and rates for schedule comparisons.
    pub(crate) fn cashflows<T: Scalar>(leg: &Leg<T>) -> Result<Vec<String>> {
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
    pub(crate) fn assert_legs<T: Scalar>(
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
    pub(crate) fn loan() -> Result<MakeLoan<DualFwd>> {
        Ok(MakeLoan::default()
            .with_identifier("loan".into())
            .with_start_tenor(p("0D")?)
            .with_tenor(p("2Y")?)
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
    use crate::time::calendars::target::TARGET;
    use crate::{
        core::resolution::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn calendar_adjustment_business_day_lags_end_of_month_and_date_errors_are_explicit(
    ) -> Result<()> {
        let market = empty_market(Date::new(2026, 1, 30));
        let calendar = Calendar::TARGET(TARGET::new());
        let ctx = ResolutionContext::new(&market).with_date_conventions(
            &calendar,
            BusinessDayConvention::ModifiedFollowing,
            true,
        );
        assert_eq!(
            ctx.advance_date(Date::new(2026, 1, 30), p("1M")?)?,
            Date::new(2026, 2, 27)
        );
        assert_eq!(
            ctx.advance_date(Date::new(2026, 2, 20), p("2D")?)?,
            reference()
        );
        assert_eq!(
            ctx.advance_date(Date::new(2026, 2, 21), p("0D")?)?,
            Date::new(2026, 2, 23)
        );
        assert_eq!(
            ctx.advance_date(reference(), Period::new(-2, TimeUnit::Days))?,
            Date::new(2026, 2, 20)
        );
        assert!(ctx
            .resolve_date(Some(reference()), Some(p("0D")?), reference(), "start")
            .is_err());
        assert!(ctx.advance_date(Date::empty(), p("1Y")?).is_err());
        assert!(ctx
            .advance_date(reference(), Period::new(i32::MAX, TimeUnit::Years))
            .is_err());
        Ok(())
    }
}
