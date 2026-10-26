//! Reusable credit-default-swap terms and context-based date resolution.

use super::creditdefaultswap::CreditDefaultSwap;
use crate::{
    core::resolution::{ResolutionContext, Resolve},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::{
        calendar::Calendar,
        calendars::nullcalendar::NullCalendar,
        date::Date,
        daycounter::DayCounter,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
    utils::errors::{QSError, Result},
};

/// Defines a CDS using either explicit dates or tenors.
#[derive(Clone, Default)]
pub struct MakeCreditDefaultSwap {
    start_date: Option<Date>,
    start_tenor: Option<Period>,
    maturity_date: Option<Date>,
    maturity_tenor: Option<Period>,
    identifier: Option<String>,
    credit_index: Option<MarketIndex>,
    discount_index: Option<MarketIndex>,
    currency: Option<Currency>,
    spread: Option<f64>,
    recovery: Option<f64>,
    premium_frequency: Option<Frequency>,
    day_counter: Option<DayCounter>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
}

impl MakeCreditDefaultSwap {
    /// Sets the product identifier.
    #[must_use]
    pub fn with_identifier(mut self, value: String) -> Self {
        self.identifier = Some(value);
        self
    }

    /// Sets the borrower credit index.
    #[must_use]
    pub fn with_credit_index(mut self, value: MarketIndex) -> Self {
        self.credit_index = Some(value);
        self
    }

    /// Sets the discount curve index.
    #[must_use]
    pub fn with_discount_index(mut self, value: MarketIndex) -> Self {
        self.discount_index = Some(value);
        self
    }

    /// Sets the premium currency.
    #[must_use]
    pub const fn with_currency(mut self, value: Currency) -> Self {
        self.currency = Some(value);
        self
    }

    /// Sets the annual decimal running spread.
    #[must_use]
    pub const fn with_spread(mut self, value: f64) -> Self {
        self.spread = Some(value);
        self
    }

    /// Sets the recovery fraction; defaults to 0.4.
    #[must_use]
    pub const fn with_recovery(mut self, value: f64) -> Self {
        self.recovery = Some(value);
        self
    }

    /// Sets premium frequency; defaults to Quarterly.
    #[must_use]
    pub const fn with_premium_frequency(mut self, value: Frequency) -> Self {
        self.premium_frequency = Some(value);
        self
    }

    /// Sets premium accrual; defaults to Actual360.
    #[must_use]
    pub const fn with_day_counter(mut self, value: DayCounter) -> Self {
        self.day_counter = Some(value);
        self
    }

    /// Sets date resolution conventions independently of the pricing policy.
    #[must_use]
    pub fn with_calendar(mut self, value: Calendar) -> Self {
        self.calendar = Some(value);
        self
    }

    /// Sets relative-date adjustment; defaults to Unadjusted.
    #[must_use]
    pub const fn with_business_day_convention(mut self, value: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(value);
        self
    }

    /// Sets an absolute start date.
    #[must_use]
    pub const fn with_start_date(mut self, date: Date) -> Self {
        self.start_date = Some(date);
        self.start_tenor = None;
        self
    }

    /// Sets start date as a tenor from the resolution reference date.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_date = None;
        self.start_tenor = Some(tenor);
        self
    }

    /// Sets an absolute maturity date.
    #[must_use]
    pub const fn with_maturity_date(mut self, date: Date) -> Self {
        self.maturity_date = Some(date);
        self.maturity_tenor = None;
        self
    }

    /// Sets maturity date as a tenor from the resolved start date.
    #[must_use]
    pub const fn with_maturity_tenor(mut self, tenor: Period) -> Self {
        self.maturity_date = None;
        self.maturity_tenor = Some(tenor);
        self
    }

    /// Sets maturity as a tenor from the resolved start date.
    #[must_use]
    pub const fn with_tenor(self, tenor: Period) -> Self {
        self.with_maturity_tenor(tenor)
    }

    /// Builds an explicitly dated CDS.
    ///
    /// # Errors
    /// Rejects missing terms, unresolved relative dates, or invalid CDS terms.
    pub fn build(self) -> Result<CreditDefaultSwap> {
        if self.start_tenor.is_some() || self.maturity_tenor.is_some() {
            return Err(QSError::InvalidValueErr(
                "Relative dates require maker.resolve(&ResolutionContext)".into(),
            ));
        }
        CreditDefaultSwap::new(
            required(self.identifier, "Identifier")?,
            required(self.credit_index, "Credit index")?,
            required(self.discount_index, "Discount index")?,
            required(self.currency, "Currency")?,
            required(self.start_date, "Start date")?,
            required(self.maturity_date, "Maturity date")?,
            required(self.spread, "Spread")?,
            self.recovery.unwrap_or(0.4),
            self.premium_frequency.unwrap_or(Frequency::Quarterly),
            self.day_counter.unwrap_or(DayCounter::Actual360),
        )
    }
}

impl Resolve for MakeCreditDefaultSwap {
    type Resolved = CreditDefaultSwap;

    fn resolve(&self, context: &ResolutionContext<'_>) -> Result<Self::Resolved> {
        let calendar = self
            .calendar
            .clone()
            .unwrap_or_else(|| Calendar::NullCalendar(NullCalendar::new()));
        let dates = context.with_date_conventions(
            &calendar,
            self.business_day_convention
                .unwrap_or(BusinessDayConvention::Unadjusted),
            false,
        );
        let start = dates.resolve_date(
            self.start_date,
            self.start_tenor,
            context.reference_date(),
            "Start date",
        )?;
        let maturity = dates.resolve_date(
            self.maturity_date,
            self.maturity_tenor,
            start,
            "Maturity date",
        )?;
        self.clone()
            .with_start_date(start)
            .with_maturity_date(maturity)
            .build()
    }
}

fn required<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| QSError::ValueNotSetErr(label.into()))
}

#[cfg(test)]
mod tests {
    use crate::{
        core::resolution::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn cds_resolves_relative_start_and_maturity() -> Result<()> {
        let market = empty_market(reference());
        let ctx = ResolutionContext::new(&market);
        let cds = MakeCreditDefaultSwap::default()
            .with_identifier("cds".into())
            .with_start_tenor(p("0D")?)
            .with_tenor(p("5Y")?)
            .with_credit_index(MarketIndex::Credit("borrower".into()))
            .with_discount_index(MarketIndex::ESTR)
            .with_currency(Currency::EUR)
            .with_spread(0.01)
            .resolve(&ctx)?;
        assert_eq!(cds.start_date(), reference());
        assert_eq!(cds.maturity_date(), reference() + p("5Y")?);
        Ok(())
    }
}
