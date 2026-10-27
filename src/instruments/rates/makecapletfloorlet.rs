//! Context-resolved single-period rate options.

use super::capletfloorlet::{CapletFloorlet, CapletFloorletType};
use crate::{
    core::makeinstrument::{resolve_date, MakeInstrument},
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    time::{
        calendar::Calendar, calendars::nullcalendar::NullCalendar, date::Date,
        daycounter::DayCounter, enums::BusinessDayConvention, period::Period,
    },
    utils::errors::{QSError, Result},
    volatility::volatilityindexing::Strike,
};

/// Defines a caplet/floorlet without requiring dates at construction.
/// Fixing defaults to accrual start and payment defaults to accrual end.
#[derive(Clone, Default)]
pub struct MakeCapletFloorlet {
    start_accrual_date: Option<Date>,
    start_tenor: Option<Period>,
    end_accrual_date: Option<Date>,
    end_tenor: Option<Period>,
    fixing_date: Option<Date>,
    fixing_tenor: Option<Period>,
    payment_date: Option<Date>,
    payment_tenor: Option<Period>,
    identifier: Option<String>,
    market_index: Option<MarketIndex>,
    currency: Option<Currency>,
    payoff_type: Option<CapletFloorletType>,
    strike: Option<Strike>,
    day_counter: Option<DayCounter>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
}

impl MakeCapletFloorlet {
    /// Sets the product identifier.
    #[must_use]
    pub fn with_identifier(mut self, value: String) -> Self {
        self.identifier = Some(value);
        self
    }

    /// Sets the observed rate index.
    #[must_use]
    pub fn with_market_index(mut self, value: MarketIndex) -> Self {
        self.market_index = Some(value);
        self
    }

    /// Sets the payoff currency.
    #[must_use]
    pub const fn with_currency(mut self, value: Currency) -> Self {
        self.currency = Some(value);
        self
    }

    /// Sets caplet or floorlet direction.
    #[must_use]
    pub const fn with_payoff_type(mut self, value: CapletFloorletType) -> Self {
        self.payoff_type = Some(value);
        self
    }

    /// Sets the absolute or forward-relative strike.
    #[must_use]
    pub const fn with_strike(mut self, value: Strike) -> Self {
        self.strike = Some(value);
        self
    }

    /// Sets payoff accrual independently of index quoting.
    #[must_use]
    pub const fn with_day_counter(mut self, value: DayCounter) -> Self {
        self.day_counter = Some(value);
        self
    }

    /// Sets the calendar for date resolution.
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

    /// Sets an absolute start accrual date.
    #[must_use]
    pub const fn with_start_accrual_date(mut self, date: Date) -> Self {
        self.start_accrual_date = Some(date);
        self.start_tenor = None;
        self
    }

    /// Sets start accrual date as a tenor from the context evaluation date.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_accrual_date = None;
        self.start_tenor = Some(tenor);
        self
    }

    /// Sets an absolute end accrual date.
    #[must_use]
    pub const fn with_end_accrual_date(mut self, date: Date) -> Self {
        self.end_accrual_date = Some(date);
        self.end_tenor = None;
        self
    }

    /// Sets end accrual date as a tenor from the resolved accrual start.
    #[must_use]
    pub const fn with_end_tenor(mut self, tenor: Period) -> Self {
        self.end_accrual_date = None;
        self.end_tenor = Some(tenor);
        self
    }

    /// Sets an absolute fixing date.
    #[must_use]
    pub const fn with_fixing_date(mut self, date: Date) -> Self {
        self.fixing_date = Some(date);
        self.fixing_tenor = None;
        self
    }

    /// Sets fixing date as a tenor from the resolved accrual start; negative tenors express a fixing lag.
    #[must_use]
    pub const fn with_fixing_tenor(mut self, tenor: Period) -> Self {
        self.fixing_date = None;
        self.fixing_tenor = Some(tenor);
        self
    }

    /// Sets an absolute payment date.
    #[must_use]
    pub const fn with_payment_date(mut self, date: Date) -> Self {
        self.payment_date = Some(date);
        self.payment_tenor = None;
        self
    }

    /// Sets payment date as a tenor from the resolved accrual end; positive tenors express a payment lag.
    #[must_use]
    pub const fn with_payment_tenor(mut self, tenor: Period) -> Self {
        self.payment_date = None;
        self.payment_tenor = Some(tenor);
        self
    }

    /// Sets the accrual start date.
    #[must_use]
    pub const fn with_start_date(self, date: Date) -> Self {
        self.with_start_accrual_date(date)
    }

    /// Sets the accrual end date.
    #[must_use]
    pub const fn with_end_date(self, date: Date) -> Self {
        self.with_end_accrual_date(date)
    }

    /// Sets the accrual length from the resolved start.
    #[must_use]
    pub const fn with_tenor(self, tenor: Period) -> Self {
        self.with_end_tenor(tenor)
    }
}

impl MakeInstrument for MakeCapletFloorlet {
    type Instrument = CapletFloorlet;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none()
            && self.end_tenor.is_none()
            && self.fixing_tenor.is_none()
            && self.payment_tenor.is_none()
    }

    /// Builds an explicitly dated caplet/floorlet.
    ///
    /// # Errors
    /// Rejects missing fields, unresolved tenors, and inconsistent date ordering.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeCapletFloorlet has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if [
            self.start_accrual_date,
            self.end_accrual_date,
            self.fixing_date,
            self.payment_date,
        ]
        .contains(&Some(Date::empty()))
        {
            return Err(QSError::InvalidValueErr(
                "MakeCapletFloorlet dates must not be empty".into(),
            ));
        }
        let start = required(self.start_accrual_date, "Accrual start")?;
        let end = required(self.end_accrual_date, "Accrual end")?;
        let fixing = self.fixing_date.unwrap_or(start);
        let payment = self.payment_date.unwrap_or(end);
        if end <= start || fixing > end || payment < end {
            return Err(QSError::InvalidValueErr(
                "Invalid caplet accrual, fixing, or payment dates".into(),
            ));
        }
        let mut product = CapletFloorlet::new(
            required(self.identifier.clone(), "Identifier")?,
            required(self.market_index.clone(), "Market index")?,
            required(self.currency, "Currency")?,
            fixing,
            start,
            end,
            payment,
            required(self.payoff_type, "Payoff type")?,
            required(self.strike, "Strike")?,
        );
        if let Some(day_counter) = self.day_counter {
            product = product.with_day_counter(day_counter);
        }
        Ok(product)
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let calendar = self
            .calendar
            .clone()
            .unwrap_or_else(|| Calendar::NullCalendar(NullCalendar::new()));
        let date_calendar = &calendar;
        let date_convention = self
            .business_day_convention
            .unwrap_or(BusinessDayConvention::Unadjusted);
        let date_end_of_month = false;
        let start = resolve_date(
            self.start_accrual_date,
            self.start_tenor,
            reference_date,
            "Accrual start",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?;
        let end = resolve_date(
            self.end_accrual_date,
            self.end_tenor,
            start,
            "Accrual end",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?;
        let fixing = if self.fixing_date.is_some() || self.fixing_tenor.is_some() {
            resolve_date(
                self.fixing_date,
                self.fixing_tenor,
                start,
                "Fixing date",
                date_calendar,
                date_convention,
                date_end_of_month,
            )?
        } else {
            start
        };
        let payment = if self.payment_date.is_some() || self.payment_tenor.is_some() {
            resolve_date(
                self.payment_date,
                self.payment_tenor,
                end,
                "Payment date",
                date_calendar,
                date_convention,
                date_end_of_month,
            )?
        } else {
            end
        };
        Ok(self
            .clone()
            .with_start_date(start)
            .with_end_date(end)
            .with_fixing_date(fixing)
            .with_payment_date(payment))
    }
}

fn required<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| QSError::ValueNotSetErr(label.into()))
}

#[cfg(test)]
mod tests {
    use crate::{
        core::makeinstrument::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn caplet_resolves_relative_accrual_fixing_and_payment_dates() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let caplet = MakeCapletFloorlet::default()
            .with_identifier("caplet".into())
            .with_start_tenor(p("1Y")?)
            .with_tenor(p("3M")?)
            .with_fixing_tenor(Period::new(-2, TimeUnit::Days))
            .with_payment_tenor(p("2D")?)
            .with_currency(Currency::EUR)
            .with_market_index(MarketIndex::EURIBOR3m)
            .with_payoff_type(CapletFloorletType::Caplet)
            .with_strike(Strike::Absolute(0.04))
            .build_using(ctx)?;
        assert_eq!(caplet.start_accrual_date(), reference() + p("1Y")?);
        assert_eq!(caplet.fixing_date(), reference() + p("1Y")? - 2);
        assert_eq!(caplet.payment_date(), reference() + p("15M")? + 2);
        Ok(())
    }
}
