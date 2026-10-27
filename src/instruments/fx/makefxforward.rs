use crate::{
    core::{
        makeinstrument::{resolve_date, MakeInstrument},
        trade::Side,
    },
    currencies::currency::Currency,
    instruments::fx::fxforward::{FxForward, FxForwardSettlement},
    time::{
        calendars::{nullcalendar::NullCalendar, traits::IsCalendar},
        date::Date,
        daycounter::DayCounter,
        enums::BusinessDayConvention,
        period::Period,
    },
    utils::errors::{QSError, Result},
};

/// A builder for creating an [`FxForward`] instance.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let fx_fwd = MakeFxForward::default()
///     .with_identifier("EURUSD-1M".to_string())
///     .with_delivery_date(Date::new(2024, 7, 1))
///     .with_forward_price(1.1025)
///     .with_base_currency(Currency::EUR)
///     .with_quote_currency(Currency::USD)
///     .as_deliverable()
///     .build()
///     .expect("failed to build fx forward");
///
/// assert_eq!(fx_fwd.forward_price(), Some(1.1025));
/// assert!(fx_fwd.is_deliverable());
/// ```
#[derive(Clone, Default)]
pub struct MakeFxForward {
    delivery_tenor: Option<Period>,
    identifier: Option<String>,
    delivery_date: Option<Date>,
    forward_price: Option<f64>,
    forward_points: Option<f64>,
    base_currency: Option<Currency>,
    quote_currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    settlement: Option<FxForwardSettlement>,
    fixing_tenor: Option<Period>,
    settlement_currency: Option<Currency>,
    side: Option<Side>,
}

impl MakeFxForward {
    /// Sets delivery date as a tenor from the context evaluation date.
    /// Replaces an explicitly supplied delivery date. Use `build_using` when an external anchor is needed.
    #[must_use]
    pub const fn with_delivery_tenor(mut self, tenor: Period) -> Self {
        self.delivery_tenor = Some(tenor);
        self.delivery_date = None;
        self
    }

    /// Sets the identifier.
    #[must_use]
    pub fn with_identifier(mut self, identifier: String) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Sets the delivery date.
    #[must_use]
    pub const fn with_delivery_date(mut self, date: Date) -> Self {
        self.delivery_date = Some(date);
        self.delivery_tenor = None;
        self
    }

    /// Sets the agreed forward exchange rate.
    #[must_use]
    pub const fn with_forward_rate(mut self, rate: f64) -> Self {
        self.forward_price = Some(rate);
        self
    }

    /// Sets the agreed outright forward price.
    #[must_use]
    pub const fn with_forward_price(mut self, price: f64) -> Self {
        self.forward_price = Some(price);
        self
    }

    /// Sets the forward points quote.
    #[must_use]
    pub const fn with_forward_points(mut self, points: f64) -> Self {
        self.forward_points = Some(points);
        self
    }

    /// Sets the base currency (the currency being bought).
    #[must_use]
    pub const fn with_base_currency(mut self, currency: Currency) -> Self {
        self.base_currency = Some(currency);
        self
    }

    /// Sets the quote currency (the currency being sold).
    #[must_use]
    pub const fn with_quote_currency(mut self, currency: Currency) -> Self {
        self.quote_currency = Some(currency);
        self
    }

    /// Sets the day count convention. Defaults to `Actual360`.
    #[must_use]
    pub const fn with_day_counter(mut self, dc: DayCounter) -> Self {
        self.day_counter = Some(dc);
        self
    }

    /// Sets the settlement convention explicitly.
    #[must_use]
    pub const fn with_settlement(mut self, settlement: FxForwardSettlement) -> Self {
        self.fixing_tenor = None;
        self.settlement_currency = None;
        self.settlement = Some(settlement);
        self
    }

    /// Marks the contract as deliverable.
    #[must_use]
    pub const fn as_deliverable(mut self) -> Self {
        self.fixing_tenor = None;
        self.settlement_currency = None;
        self.settlement = Some(FxForwardSettlement::Deliverable);
        self
    }

    /// Marks the contract as a non-deliverable forward.
    #[must_use]
    pub const fn as_ndf(mut self, fixing_date: Date, settlement_currency: Currency) -> Self {
        self.fixing_tenor = None;
        self.settlement_currency = None;
        self.settlement = Some(FxForwardSettlement::NonDeliverable {
            fixing_date,
            settlement_currency,
        });
        self
    }

    /// Defines an NDF fixing as a tenor from the context evaluation date.
    #[must_use]
    pub const fn as_ndf_tenor(mut self, tenor: Period, currency: Currency) -> Self {
        self.fixing_tenor = Some(tenor);
        self.settlement_currency = Some(currency);
        self.settlement = None;
        self
    }

    /// Stores a compatibility side that `build` does not use.
    ///
    /// Set the direction on `FxForwardTrade` instead.
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }
}

impl MakeInstrument for MakeFxForward {
    type Instrument = FxForward;

    fn is_resolved(&self) -> bool {
        self.delivery_tenor.is_none() && self.fixing_tenor.is_none()
    }

    /// Builds the [`FxForward`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    fn build(&self) -> Result<Self::Instrument> {
        if !self.is_resolved() {
            return Err(QSError::UnresolvedProductErr(
                "MakeFxForward requires an evaluation date to resolve delivery_tenor or fixing_tenor. Supply the corresponding absolute dates or call maker.build_using(&context)".into(),
            ));
        }
        if self.delivery_date == Some(Date::empty()) {
            return Err(QSError::InvalidValueErr(
                "MakeFxForward dates must not be empty".into(),
            ));
        }
        let identifier = self
            .identifier
            .clone()
            .ok_or_else(|| QSError::ValueNotSetErr("Identifier".into()))?;
        let delivery_date = self
            .delivery_date
            .ok_or_else(|| QSError::ValueNotSetErr("Delivery date".into()))?;
        let base_currency = self
            .base_currency
            .ok_or_else(|| QSError::ValueNotSetErr("Base currency".into()))?;
        let quote_currency = self
            .quote_currency
            .ok_or_else(|| QSError::ValueNotSetErr("Quote currency".into()))?;

        let day_counter = self.day_counter.unwrap_or(DayCounter::Actual360);
        let settlement = self.settlement.unwrap_or(FxForwardSettlement::Deliverable);

        if self.forward_price.is_none() && self.forward_points.is_none() {
            return Err(QSError::ValueNotSetErr(
                "Either forward price or forward points".into(),
            ));
        }

        if let FxForwardSettlement::NonDeliverable { fixing_date, .. } = settlement {
            if fixing_date > delivery_date {
                return Err(QSError::InvalidValueErr(
                    "NDF fixing date cannot be after delivery date".into(),
                ));
            }
        }

        Ok(FxForward::new(
            identifier,
            delivery_date,
            self.forward_price,
            self.forward_points,
            base_currency,
            quote_currency,
            day_counter,
            settlement,
        ))
    }

    fn resolve_dates(&self, reference_date: Date) -> Result<Self> {
        let mut resolved = self.clone();
        let date_calendar = &NullCalendar::new();
        let date_convention = BusinessDayConvention::Unadjusted;
        let date_end_of_month = false;
        resolved.delivery_date = Some(resolve_date(
            self.delivery_date,
            self.delivery_tenor,
            reference_date,
            "delivery_date",
            date_calendar,
            date_convention,
            date_end_of_month,
        )?);
        resolved.delivery_tenor = None;
        if let Some(tenor) = self.fixing_tenor {
            resolved.settlement = Some(FxForwardSettlement::NonDeliverable {
                fixing_date: date_calendar.advance(
                    reference_date,
                    tenor,
                    Some(date_convention),
                    date_end_of_month,
                )?,
                settlement_currency: self
                    .settlement_currency
                    .ok_or_else(|| QSError::ValueNotSetErr("NDF settlement currency".into()))?,
            });
            resolved.fixing_tenor = None;
            resolved.settlement_currency = None;
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::MakeFxForward;
    use crate::{
        core::makeinstrument::MakeInstrument,
        currencies::currency::Currency,
        instruments::fx::fxforward::FxForwardSettlement,
        time::{date::Date, daycounter::DayCounter},
    };

    #[test]
    fn builds_deliverable_outright_forward() {
        let fx_forward = MakeFxForward::default()
            .with_identifier("EURUSD-1M".to_string())
            .with_delivery_date(Date::new(2026, 4, 6))
            .with_forward_price(1.1025)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_day_counter(DayCounter::Actual360)
            .build()
            .expect("deliverable outright forward should build");

        assert_eq!(fx_forward.forward_price(), Some(1.1025));
        assert_eq!(fx_forward.forward_points(), None);
        assert!(fx_forward.is_outright());
        assert!(fx_forward.is_deliverable());
    }

    #[test]
    fn builds_ndf_with_forward_points() {
        let fixing_date = Date::new(2026, 4, 3);
        let delivery_date = Date::new(2026, 4, 6);

        let fx_forward = MakeFxForward::default()
            .with_identifier("USDKRW-1M-NDF".to_string())
            .with_delivery_date(delivery_date)
            .with_forward_points(12.5)
            .with_base_currency(Currency::USD)
            .with_quote_currency(Currency::KRW)
            .as_ndf(fixing_date, Currency::USD)
            .build()
            .expect("ndf with forward points should build");

        assert_eq!(fx_forward.forward_price(), None);
        assert_eq!(fx_forward.forward_points(), Some(12.5));
        assert!(fx_forward.has_forward_points());
        assert!(fx_forward.is_ndf());
        assert_eq!(fx_forward.fixing_date(), Some(fixing_date));
        assert_eq!(fx_forward.settlement_currency(), Some(Currency::USD));
        assert_eq!(
            fx_forward.settlement(),
            FxForwardSettlement::NonDeliverable {
                fixing_date,
                settlement_currency: Currency::USD,
            }
        );
    }

    #[test]
    fn rejects_missing_forward_quote() {
        let err = MakeFxForward::default()
            .with_identifier("EURUSD-1M".to_string())
            .with_delivery_date(Date::new(2026, 4, 6))
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .build()
            .expect_err("missing forward quote should fail");

        assert!(err
            .to_string()
            .contains("Either forward price or forward points"));
    }

    #[test]
    fn rejects_ndf_fixing_after_delivery() {
        let err = MakeFxForward::default()
            .with_identifier("USDKRW-1M-NDF".to_string())
            .with_delivery_date(Date::new(2026, 4, 6))
            .with_forward_points(12.5)
            .with_base_currency(Currency::USD)
            .with_quote_currency(Currency::KRW)
            .as_ndf(Date::new(2026, 4, 7), Currency::USD)
            .build()
            .expect_err("invalid ndf fixing should fail");

        assert!(err
            .to_string()
            .contains("NDF fixing date cannot be after delivery date"));
    }
}

#[cfg(test)]
mod resolution_tests {
    use crate::{
        core::makeinstrument::test_support::{empty_market, p, reference},
        prelude::*,
    };

    #[test]
    fn ndf_resolves_relative_delivery_and_fixing_dates() -> Result<()> {
        let market = empty_market(reference());
        let ctx = &market;
        let expiry = reference() + p("1Y")?;
        let fx = MakeFxForward::default()
            .with_identifier("ndf".into())
            .with_delivery_tenor(p("1Y")?)
            .as_ndf_tenor(p("6M")?, Currency::USD)
            .with_base_currency(Currency::EUR)
            .with_quote_currency(Currency::USD)
            .with_forward_price(1.1)
            .build_using(ctx)?;
        assert_eq!(fx.delivery_date(), expiry);
        assert_eq!(fx.fixing_date(), Some(reference() + p("6M")?));
        Ok(())
    }
}
