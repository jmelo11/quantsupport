use crate::{
    core::trade::Side,
    currencies::currency::Currency,
    instruments::fx::fxforward::{FxForward, FxForwardSettlement},
    time::{date::Date, daycounter::DayCounter},
    utils::errors::{QSError, Result},
};

/// A builder for creating an [`FxForward`] instance.
#[derive(Default)]
pub struct MakeFxForward {
    identifier: Option<String>,
    delivery_date: Option<Date>,
    forward_price: Option<f64>,
    forward_points: Option<f64>,
    base_currency: Option<Currency>,
    quote_currency: Option<Currency>,
    day_counter: Option<DayCounter>,
    settlement: Option<FxForwardSettlement>,
    side: Option<Side>,
}

impl MakeFxForward {
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
        self.settlement = Some(settlement);
        self
    }

    /// Marks the contract as deliverable.
    #[must_use]
    pub const fn as_deliverable(mut self) -> Self {
        self.settlement = Some(FxForwardSettlement::Deliverable);
        self
    }

    /// Marks the contract as a non-deliverable forward.
    #[must_use]
    pub const fn as_ndf(mut self, fixing_date: Date, settlement_currency: Currency) -> Self {
        self.settlement = Some(FxForwardSettlement::NonDeliverable {
            fixing_date,
            settlement_currency,
        });
        self
    }

    /// Sets the side (defaults to `LongRecieve` — buying base currency).
    #[must_use]
    pub const fn with_side(mut self, side: Side) -> Self {
        self.side = Some(side);
        self
    }

    /// Builds the [`FxForward`] instance.
    ///
    /// # Errors
    /// Returns an error if any of the required fields are missing.
    pub fn build(self) -> Result<FxForward> {
        let identifier = self
            .identifier
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
}

#[cfg(test)]
mod tests {
    use super::MakeFxForward;
    use crate::{
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
