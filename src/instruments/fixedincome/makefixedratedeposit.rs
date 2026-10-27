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
        cashflows::makeleg::{MakeLeg, RateType},
        fixedincome::fixedratedeposit::FixedRateDeposit,
    },
    rates::interestrate::{InterestRate, RateDefinition},
    time::{
        calendars::nullcalendar::NullCalendar,
        date::Date,
        enums::{BusinessDayConvention, Frequency},
        period::Period,
    },
    utils::errors::{QSError, Result},
};
use std::marker::PhantomData;

/// A builder for creating a [`FixedRateDeposit`] instance, allowing for a flexible and stepwise construction process.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let rate_def = RateDefinition::new(
///     DayCounter::Actual360,
///     Compounding::Simple,
///     Frequency::Annual,
/// );
///
/// let deposit = MakeFixedRateDeposit::<DualFwd>::default()
///     .with_identifier("DEPO-3M".to_string())
///     .with_start_date(Date::new(2024, 1, 1))
///     .with_maturity_date(Date::new(2024, 4, 1))
///     .with_rate(0.05)
///     .with_notional(1_000_000.0)
///     .with_rate_definition(rate_def)
///     .with_discount_index(Some(MarketIndex::SOFR))
///     .with_currency(Currency::USD)
///     .build()
///     .expect("failed to build fixed rate deposit");
///
/// assert_eq!(deposit.identifier(), "DEPO-3M");
/// ```
#[derive(Clone, Default)]
pub struct MakeFixedRateDeposit<T: Scalar> {
    start_tenor: Option<Period>,
    maturity_tenor: Option<Period>,
    start_date: Option<Date>,
    maturity_date: Option<Date>,
    rate: Option<f64>,
    units: Option<f64>,
    notional: Option<f64>,
    identifier: Option<String>,
    rate_definition: Option<RateDefinition>,
    discount_index: Option<MarketIndex>,
    currency: Option<Currency>,
    side: Option<Side>,
    _marker: PhantomData<T>,
}

impl<T> MakeFixedRateDeposit<T>
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

    /// Sets maturity as a tenor from the resolved start date.
    #[must_use]
    pub const fn with_tenor(self, tenor: Period) -> Self {
        self.with_maturity_tenor(tenor)
    }

    /// Sets the start date of the fixed rate deposit.
    #[must_use]
    pub const fn with_start_date(mut self, start_date: Date) -> Self {
        self.start_date = Some(start_date);
        self.start_tenor = None;
        self
    }

    /// Sets the end date of the fixed rate deposit.
    #[must_use]
    pub const fn with_maturity_date(mut self, maturity_date: Date) -> Self {
        self.maturity_date = Some(maturity_date);
        self.maturity_tenor = None;
        self
    }

    /// Sets the interest rate of the fixed rate deposit.
    #[must_use]
    pub const fn with_rate(mut self, rate: f64) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Sets the notional amount of the fixed rate deposit.
    #[must_use]
    pub const fn with_notional(mut self, notional: f64) -> Self {
        self.notional = Some(notional);
        self
    }

    /// Sets the rate definition of the fixed rate deposit.
    #[must_use]
    pub const fn with_rate_definition(mut self, rate_definition: RateDefinition) -> Self {
        self.rate_definition = Some(rate_definition);
        self
    }

    /// Sets the market index associated with the fixed rate deposit.
    #[must_use]
    pub fn with_discount_index(mut self, discount_index: Option<MarketIndex>) -> Self {
        self.discount_index = discount_index;
        self
    }

    /// Sets the currency of the fixed rate deposit.
    #[must_use]
    pub const fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    /// Sets the units of the fixed rate deposit.
    /// If not set, it defaults to 100.0.
    #[must_use]
    pub const fn with_units(mut self, units: f64) -> Self {
        self.units = Some(units);
        self
    }

    /// Sets the identifier of the fixed rate deposit.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the side of the fixed rate deposit (defaults to `LongReceive` if not set).
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }
}

impl<T: Scalar> MakeInstrument for MakeFixedRateDeposit<T> {
    type Instrument = FixedRateDeposit<T>;

    fn is_resolved(&self) -> bool {
        self.start_tenor.is_none() && self.maturity_tenor.is_none()
    }

    /// Builds the [`FixedRateDeposit`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing or invalid.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeFixedRateDeposit has unresolved date tenors. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if [self.start_date, self.maturity_date].contains(&Some(Date::empty())) {
            return Err(QSError::InvalidValueErr(
                "MakeFixedRateDeposit dates must not be empty".into(),
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
        let rate = self
            .rate
            .ok_or_else(|| QSError::ValueNotSetErr("Rate".into()))?;
        let rate_definition = self
            .rate_definition
            .ok_or_else(|| QSError::ValueNotSetErr("Rate definition".into()))?;
        let currency = self
            .currency
            .ok_or_else(|| QSError::ValueNotSetErr("Currency".into()))?;

        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;

        let units = self.units.unwrap_or(100.0);
        let side = self.side.unwrap_or(Side::LongReceive);

        let interest_rate = InterestRate::from_rate_definition(T::scalar(rate), rate_definition);

        let leg = MakeLeg::<T>::default()
            .with_leg_id(0)
            .with_notional(notional)
            .with_side(side)
            .with_asset_class(AssetClass::FixedIncome)
            .with_currency(currency)
            .with_discount_index(self.discount_index.clone())
            .with_start_date(start_date)
            .with_end_date(maturity_date)
            .with_rate_type(RateType::Fixed)
            .with_rate(interest_rate)
            .with_payment_frequency(Frequency::Once)
            .bullet()
            .build()?;

        Ok(FixedRateDeposit::new(
            identifier,
            units,
            leg,
            self.discount_index.clone(),
            start_date,
            maturity_date,
            currency,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let date_calendar = &NullCalendar::new();
        let date_convention = BusinessDayConvention::Unadjusted;
        let date_end_of_month = false;
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
    fn deposit_resolves_relative_start_and_maturity() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let end = reference() + p("2Y")?;
        let deposit = MakeFixedRateDeposit::<f64>::default()
            .with_identifier("deposit".into())
            .with_start_tenor(p("0D")?)
            .with_tenor(p("2Y")?)
            .with_rate(0.04)
            .with_notional(100_000.0)
            .with_currency(Currency::EUR)
            .with_rate_definition(RateDefinition::new(
                DayCounter::Actual365,
                Compounding::Simple,
                Frequency::Annual,
            ));
        assert_legs(
            &deposit.build_using(ctx)?,
            &deposit
                .with_start_date(reference())
                .with_maturity_date(end)
                .build()?,
        )?;
        Ok(())
    }
}
