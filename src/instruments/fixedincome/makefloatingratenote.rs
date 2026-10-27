use crate::{
    ad::scalar::Scalar,
    core::{
        instrument::AssetClass,
        makeinstrument::{resolve_date, MakeInstrument},
        trade::Side,
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::{
        cashflows::makeleg::{MakeLeg, PaymentStructure, RateType},
        fixedincome::floatingratenote::FloatingRateNote,
    },
    time::{
        calendar::Calendar,
        calendars::nullcalendar::NullCalendar,
        date::Date,
        enums::{BusinessDayConvention, DateGenerationRule, Frequency},
        period::Period,
    },
    utils::errors::{QSError, Result},
};
use std::marker::PhantomData;

/// A builder for creating a [`FloatingRateNote`] instance, allowing for a flexible and stepwise construction process.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let frn = MakeFloatingRateNote::<DualFwd>::default()
///     .with_identifier("FRN-2Y".to_string())
///     .with_start_date(Date::new(2024, 1, 1))
///     .with_maturity_date(Date::new(2026, 1, 1))
///     .with_spread(0.005)
///     .with_notional(1_000_000.0)
///     .with_forward_index(MarketIndex::TermSOFR3m)
///     .with_currency(Currency::USD)
///     .with_payment_frequency(Frequency::Quarterly)
///     .with_payment_structure(PaymentStructure::Bullet)
///     .build()
///     .expect("failed to build floating rate note");
///
/// assert_eq!(frn.identifier(), "FRN-2Y");
/// ```
#[derive(Clone, Default)]
pub struct MakeFloatingRateNote<T: Scalar> {
    start_tenor: Option<Period>,
    maturity_tenor: Option<Period>,
    first_coupon_tenor: Option<Period>,
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    spread: Option<f64>,
    units: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    forward_index: Option<MarketIndex>,
    discount_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    payment_frequency: Option<Frequency>,
    payment_structure: Option<PaymentStructure>,
    calendar: Option<Calendar>,
    business_day_convention: Option<BusinessDayConvention>,
    date_generation_rule: Option<DateGenerationRule>,
    end_of_month: Option<bool>,
    first_coupon_date: Option<Date>,
    _marker: PhantomData<T>,
}

impl<T> MakeFloatingRateNote<T>
where
    T: Scalar,
{
    /// Sets start date as a tenor from the context evaluation date.
    /// Replaces an explicitly supplied start date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_start_tenor(mut self, tenor: Period) -> Self {
        self.start_tenor = Some(tenor);
        self.start_date = None;
        self
    }

    /// Sets maturity date as a tenor from the resolved start date.
    /// Replaces an explicitly supplied maturity date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_maturity_tenor(mut self, tenor: Period) -> Self {
        self.maturity_tenor = Some(tenor);
        self.maturity_date = None;
        self
    }

    /// Sets first coupon date as a tenor from the resolved start date.
    /// Replaces an explicitly supplied first coupon date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_first_coupon_tenor(mut self, tenor: Period) -> Self {
        self.first_coupon_tenor = Some(tenor);
        self.first_coupon_date = None;
        self
    }

    /// Sets maturity as a tenor from the resolved start date.
    #[must_use]
    pub const fn with_tenor(self, tenor: Period) -> Self {
        self.with_maturity_tenor(tenor)
    }

    /// Sets the start date of the note.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self.start_tenor = None;
        self
    }

    /// Sets the maturity date of the note.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self.maturity_tenor = None;
        self
    }

    /// Sets the spread over the floating index rate.
    #[must_use]
    pub const fn with_spread(mut self, spread: f64) -> Self {
        self.spread = Some(spread);
        self
    }

    /// Sets the notional amount of the note.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the market index for the floating rate reference.
    #[must_use]
    pub fn with_forward_index(mut self, forward_index: MarketIndex) -> Self {
        self.forward_index = Some(forward_index);
        self
    }

    /// Sets the currency of the note.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the units of the note. Defaults to 100.0 if not set.
    #[must_use]
    pub const fn with_units(mut self, units: f64) -> Self {
        self.units = Some(units);
        self
    }

    /// Sets the identifier of the note.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the side of the note (defaults to `LongReceive` if not set).
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the coupon payment frequency (e.g., `Quarterly`, `Semiannual`).
    #[must_use]
    pub const fn with_payment_frequency(mut self, frequency: Frequency) -> Self {
        self.payment_frequency = Some(frequency);
        self
    }

    /// Sets the payment structure (e.g., `Bullet`, `EqualRedemptions`). Defaults to `Bullet`.
    #[must_use]
    pub const fn with_payment_structure(mut self, structure: PaymentStructure) -> Self {
        self.payment_structure = Some(structure);
        self
    }

    /// Sets the calendar for business day adjustments.
    #[must_use]
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Sets the business day convention.
    #[must_use]
    pub const fn with_business_day_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.business_day_convention = Some(convention);
        self
    }

    /// Sets the date generation rule.
    #[must_use]
    pub const fn with_date_generation_rule(mut self, rule: DateGenerationRule) -> Self {
        self.date_generation_rule = Some(rule);
        self
    }

    /// Sets the end-of-month flag for schedule generation.
    #[must_use]
    pub const fn with_end_of_month(mut self, eom: bool) -> Self {
        self.end_of_month = Some(eom);
        self
    }

    /// Sets the first coupon date (for long/short first coupon periods).
    #[must_use]
    pub const fn with_first_coupon_date(mut self, date: Date) -> Self {
        self.first_coupon_date = Some(date);
        self.first_coupon_tenor = None;
        self
    }
}

impl<T: Scalar> MakeInstrument for MakeFloatingRateNote<T> {
    type Instrument = FloatingRateNote<T>;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none()
            && self.maturity_tenor.is_none()
            && self.first_coupon_tenor.is_none()
    }

    /// Builds the [`FloatingRateNote`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeFloatingRateNote has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if [self.start_date, self.maturity_date, self.first_coupon_date]
            .contains(&Some(Date::empty()))
        {
            return Err(QSError::InvalidValueErr(
                "MakeFloatingRateNote dates must not be empty".into(),
            ));
        }
        let notional = self
            .notional
            .ok_or_else(|| QSError::ValueNotSetErr("Notional".into()))?;
        let start_date = self
            .start_date
            .ok_or_else(|| QSError::ValueNotSetErr("Start date".into()))?;
        let maturity_date = self
            .maturity_date
            .ok_or_else(|| QSError::ValueNotSetErr("Maturity date".into()))?;
        let spread = self.spread.unwrap_or(0.0);
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;
        let forward_index = self
            .forward_index
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Forward index".into()))?;
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;

        let units = self.units.unwrap_or(100.0);
        let side = self.side.unwrap_or(Side::LongReceive);
        let payment_frequency = self.payment_frequency.unwrap_or(Frequency::Quarterly);
        let structure = self.payment_structure.unwrap_or(PaymentStructure::Bullet);

        let leg = MakeLeg::<T>::default()
            .with_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_asset_class(AssetClass::FixedIncome)
            .with_currency(currency)
            .with_forward_index(forward_index.clone())
            .with_discount_index(self.discount_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Floating)
            .with_spread(spread)
            .with_payment_frequency(payment_frequency)
            .with_structure(structure)
            .with_calendar(self.calendar.clone())
            .with_business_day_convention(self.business_day_convention)
            .with_date_generation_rule(self.date_generation_rule)
            .with_end_of_month(self.end_of_month)
            .with_first_coupon_date(self.first_coupon_date)
            .build()?;

        Ok(FloatingRateNote::new(
            identifier,
            units,
            leg,
            self.discount_index.clone(),
            Some(forward_index),
            currency,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let calendar = self
            .calendar
            .clone()
            .unwrap_or_else(|| Calendar::NullCalendar(NullCalendar::new()));
        let convention = self
            .business_day_convention
            .unwrap_or(BusinessDayConvention::Unadjusted);
        let date_calendar = &calendar;
        let date_convention = convention;
        let date_end_of_month = self.end_of_month.unwrap_or(false);
        resolved.start_date = Some(resolve_date(
            self.start_date,
            self.start_tenor,
            reference_date,
            "start_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.start_tenor = None;
        resolved.maturity_date = Some(resolve_date(
            self.maturity_date,
            self.maturity_tenor,
            resolved
                .start_date
                .ok_or_else(|| QSError::ValueNotSetErr("start_date".into()))?,
            "maturity_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.maturity_tenor = None;
        if self.first_coupon_date.is_some() || self.first_coupon_tenor.is_some() {
            resolved.first_coupon_date = Some(resolve_date(
                self.first_coupon_date,
                self.first_coupon_tenor,
                resolved
                    .start_date
                    .ok_or_else(|| QSError::ValueNotSetErr("start_date".into()))?,
                "first_coupon_date",
                date_calendar,
                date_convention,
                date_end_of_month,
            )?);
        }
        resolved.first_coupon_tenor = None;
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::makeinstrument::test_support::{assert_legs, empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn note_resolves_relative_start_maturity_and_first_coupon() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let end = reference() + p("2Y")?;
        let first = reference() + p("3M")?;
        let note = MakeFloatingRateNote::<f64>::default()
            .with_identifier("note".into())
            .with_start_tenor(p("0D")?)
            .with_tenor(p("2Y")?)
            .with_first_coupon_tenor(p("3M")?)
            .with_spread(0.01)
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_forward_index(MarketIndex::EURIBOR6m)
            .with_payment_frequency(Frequency::Semiannual);
        assert_legs(
            &note.build_using(ctx)?,
            &note
                .with_start_date(reference())
                .with_maturity_date(end)
                .with_first_coupon_date(first)
                .build()?,
        )?;
        Ok(())
    }
}
