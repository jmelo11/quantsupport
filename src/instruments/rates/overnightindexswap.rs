use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::Discountable,
        instrument::Instrument,
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    indices::marketindex::MarketIndex,
    instruments::cashflows::{cashflowtype::CashflowType, leg::Leg},
    time::date::Date,
    utils::errors::{QSError, Result},
    xva::makecontigentclaim::IntoContingentClaims,
};

/// A fixed-rate versus daily-compounded overnight indexed swap.
#[derive(Clone)]
pub struct OvernightIndexSwap<T: Scalar> {
    identifier: String,
    legs: Vec<Leg<T>>,
    forward_index: MarketIndex,
    currency: Currency,
}

impl<T: Scalar> OvernightIndexSwap<T> {
    /// Creates an OIS with a fixed leg followed by an overnight floating leg.
    ///
    /// # Errors
    /// Rejects non-overnight indices, empty or mismatched-currency legs,
    /// non-fixed cashflows in the fixed leg, and floating coupons without
    /// overnight conventions or with a different market index.
    pub fn new(
        identifier: String,
        fixed_leg: Leg<T>,
        floating_leg: Leg<T>,
        forward_index: MarketIndex,
        currency: Currency,
    ) -> Result<Self> {
        let details = forward_index.rate_index_details()?;
        if !details.is_in_arrears() || details.frequency() != crate::time::enums::Frequency::Daily {
            return Err(QSError::InvalidValueErr(
                "OIS requires a daily overnight rate index".into(),
            ));
        }
        if fixed_leg.cashflows().is_empty()
            || floating_leg.cashflows().is_empty()
            || fixed_leg.currency() != currency
            || floating_leg.currency() != currency
            || details.currency() != currency
            || floating_leg.forward_index() != Some(&forward_index)
            || !fixed_leg
                .cashflows()
                .iter()
                .all(|cashflow| matches!(cashflow, CashflowType::FixedRateCoupon(_)))
            || !floating_leg.cashflows().iter().all(|cashflow| {
                matches!(cashflow, CashflowType::FloatingRateCoupon(coupon)
                    if coupon.overnight_conventions().is_some()
                        && coupon.market_index() == &forward_index)
            })
        {
            return Err(QSError::InvalidValueErr(
                "OIS requires fixed and overnight coupon legs in its index currency".into(),
            ));
        }
        Ok(Self {
            identifier,
            legs: vec![fixed_leg, floating_leg],
            forward_index,
            currency,
        })
    }

    /// Returns the fixed leg (leg 0).
    #[must_use]
    pub fn fixed_leg(&self) -> &Leg<T> {
        &self.legs[0]
    }

    /// Returns the daily-compounded overnight leg (leg 1).
    #[must_use]
    pub fn floating_leg(&self) -> &Leg<T> {
        &self.legs[1]
    }

    /// Returns the overnight market index.
    #[must_use]
    pub fn forward_index(&self) -> MarketIndex {
        self.forward_index.clone()
    }

    /// Returns the currency of both legs.
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
}

impl<T: Scalar> Instrument for OvernightIndexSwap<T> {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T: Scalar> LegsProvider<T> for OvernightIndexSwap<T> {
    fn legs(&self) -> &[Leg<T>] {
        &self.legs
    }
}

/// A trade of an overnight indexed swap.
pub struct OvernightIndexSwapTrade<T: Scalar> {
    instrument: OvernightIndexSwap<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T: Scalar> OvernightIndexSwapTrade<T> {
    /// Creates an OIS trade.
    #[must_use]
    pub const fn new(
        instrument: OvernightIndexSwap<T>,
        trade_date: Date,
        notional: f64,
        side: Side,
    ) -> Self {
        Self {
            instrument,
            trade_date,
            notional,
            side,
        }
    }

    /// Returns the notional amount of the trade.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl<T: Scalar> LegsProvider<T> for OvernightIndexSwapTrade<T> {
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

impl<T: Scalar> Trade<OvernightIndexSwap<T>> for OvernightIndexSwapTrade<T> {
    fn instrument(&self) -> &OvernightIndexSwap<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl From<OvernightIndexSwap<f64>> for OvernightIndexSwap<DualFwd> {
    fn from(value: OvernightIndexSwap<f64>) -> Self {
        Self {
            identifier: value.identifier,
            legs: value.legs.into_iter().map(Into::into).collect(),
            forward_index: value.forward_index,
            currency: value.currency,
        }
    }
}

impl From<OvernightIndexSwap<DualFwd>> for OvernightIndexSwap<f64> {
    fn from(value: OvernightIndexSwap<DualFwd>) -> Self {
        Self {
            identifier: value.identifier,
            legs: value.legs.into_iter().map(Into::into).collect(),
            forward_index: value.forward_index,
            currency: value.currency,
        }
    }
}

impl From<OvernightIndexSwapTrade<f64>> for OvernightIndexSwapTrade<DualFwd> {
    fn from(value: OvernightIndexSwapTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<OvernightIndexSwapTrade<DualFwd>> for OvernightIndexSwapTrade<f64> {
    fn from(value: OvernightIndexSwapTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl OvernightIndexSwapTrade<f64> {
    /// Decomposes the trade into contingent claims using its instrument identifier.
    ///
    /// # Errors
    /// Returns an error if claim construction fails.
    pub fn into_contingent_claims(
        &self,
    ) -> Result<Vec<crate::xva::contigentclaim::ContingentClaim>> {
        let trade_id = self.instrument().identifier();
        self.instrument()
            .legs()
            .to_vec()
            .into_contingent_claims(&trade_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instruments::{
            cashflows::floatingratecoupon::FloatingRateCoupon,
            rates::makeovernightindexswap::MakeOvernightIndexSwap,
        },
        time::{enums::TimeUnit, period::Period},
    };

    fn make_ois() -> OvernightIndexSwap<f64> {
        MakeOvernightIndexSwap::new(
            MarketIndex::SOFR,
            Date::new(2026, 1, 5),
            Period::new(1, TimeUnit::Years),
            0.04,
        )
        .with_identifier("SOFR_OIS".into())
        .with_notional(1_000_000.0)
        .with_spread(0.001)
        .build()
        .unwrap()
    }

    #[test]
    fn ois_builder_preserves_contract_and_generic_legs() {
        let ois = make_ois();
        assert_eq!(ois.identifier(), "SOFR_OIS");
        assert_eq!(ois.forward_index(), MarketIndex::SOFR);
        assert_eq!(ois.currency(), Currency::USD);
        assert_eq!(ois.legs().len(), 2);
        assert_eq!(ois.fixed_leg().side(), Side::LongReceive);
        assert_eq!(ois.floating_leg().side(), Side::PayShort);
        assert!((ois.floating_leg().spread().unwrap() - 0.001).abs() < 1e-12);
        let CashflowType::FixedRateCoupon(fixed) = &ois.fixed_leg().cashflows()[0] else {
            panic!("expected fixed coupon")
        };
        let CashflowType::FloatingRateCoupon(floating) = &ois.floating_leg().cashflows()[0] else {
            panic!("expected overnight coupon")
        };
        assert!((fixed.notional() - 1_000_000.0).abs() < 1e-12);
        assert!((fixed.rate().rate() - 0.04).abs() < 1e-12);
        assert!(floating.overnight_conventions().is_some());
    }

    #[test]
    fn ois_constructor_rejects_regular_coupon_semantics() {
        let ois = make_ois();
        let mut floating_leg = ois.floating_leg().clone();
        let CashflowType::FloatingRateCoupon(coupon) = &floating_leg.cashflows()[0] else {
            panic!("expected overnight coupon")
        };
        let regular_coupon = CashflowType::FloatingRateCoupon(FloatingRateCoupon::new(
            coupon.notional(),
            coupon.spread(),
            coupon.market_index().clone(),
            coupon.accrual_start_date(),
            coupon.accrual_end_date(),
            coupon.payment_date(),
        ));
        floating_leg.cashflows_mut()[0] = regular_coupon;
        assert!(OvernightIndexSwap::new(
            "INVALID_OIS".into(),
            ois.fixed_leg().clone(),
            floating_leg,
            MarketIndex::SOFR,
            Currency::USD,
        )
        .is_err());
    }

    #[test]
    fn ois_constructor_rejects_term_index_and_mismatched_legs() {
        let ois = make_ois();
        assert!(OvernightIndexSwap::new(
            "TERM_OIS".into(),
            ois.fixed_leg().clone(),
            ois.floating_leg().clone(),
            MarketIndex::EURIBOR6m,
            Currency::EUR,
        )
        .is_err());
        assert!(OvernightIndexSwap::new(
            "WRONG_CURRENCY".into(),
            ois.fixed_leg().clone(),
            ois.floating_leg().clone(),
            MarketIndex::SOFR,
            Currency::EUR,
        )
        .is_err());
        assert!(OvernightIndexSwap::new(
            "WRONG_FIXED_LEG".into(),
            ois.floating_leg().clone(),
            ois.floating_leg().clone(),
            MarketIndex::SOFR,
            Currency::USD,
        )
        .is_err());
    }

    #[test]
    fn ois_scalar_and_trade_roundtrip_preserves_overnight_conventions() {
        let ois = make_ois();
        let date = Date::new(2026, 1, 5);
        let expected_payment = ois.floating_leg().last_payment_date();
        let trade = OvernightIndexSwapTrade::new(ois, date, 1_000_000.0, Side::PayShort);
        let dual_trade: OvernightIndexSwapTrade<DualFwd> = trade.into();
        assert_eq!(dual_trade.legs().len(), 2);
        let trade: OvernightIndexSwapTrade<f64> = dual_trade.into();
        assert_eq!(trade.instrument().identifier(), "SOFR_OIS");
        assert_eq!(trade.instrument().forward_index(), MarketIndex::SOFR);
        assert_eq!(trade.instrument().currency(), Currency::USD);
        assert_eq!(trade.trade_date(), date);
        assert_eq!(trade.side(), Side::PayShort);
        assert!((trade.notional() - 1_000_000.0).abs() < 1e-12);
        assert_eq!(trade.legs()[1].last_payment_date(), expected_payment);
        let CashflowType::FloatingRateCoupon(coupon) = &trade.legs()[1].cashflows()[0] else {
            panic!("expected overnight coupon")
        };
        assert!(coupon.overnight_conventions().is_some());
        assert!((coupon.spread() - 0.001).abs() < 1e-12);
        assert!((coupon.notional() - 1_000_000.0).abs() < 1e-12);
        assert!(!trade.into_contingent_claims().unwrap().is_empty());
    }
}
