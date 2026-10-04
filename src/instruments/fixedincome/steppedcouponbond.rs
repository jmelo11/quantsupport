//! Callable bonds with positional coupon and notional schedules.

use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        collateral::Discountable,
        instrument::{AssetClass, Instrument},
        request::LegsProvider,
        trade::{Side, Trade},
    },
    currencies::currency::Currency,
    instruments::{
        cashflows::{cashflow::Cashflow, cashflowtype::CashflowType, leg::Leg},
        fixedincome::callschedule::CallScheduleEntry,
    },
    time::date::Date,
    utils::errors::{QSError, Result},
};

/// A callable bond with positional coupon and outstanding-notional schedules.
///
/// Each notional percentage is the fraction of original principal outstanding
/// during the corresponding coupon period. The percentages must be positive,
/// no greater than one, and non-increasing. As with coupons, the final supplied
/// percentage is carried forward when the payment schedule has more periods.
#[derive(Clone)]
pub struct SteppedCouponBond<T: Scalar> {
    identifier: String,
    units: f64,
    leg: Leg<T>,
    currency: Currency,
    coupons: Vec<CashflowType<T>>,
    notional_percentages: Vec<f64>,
    call_schedule: Vec<CallScheduleEntry>,
}

impl<T: Scalar> SteppedCouponBond<T> {
    /// Creates a stepped-coupon bond from a previously constructed leg.
    ///
    /// # Errors
    /// Returns an error for invalid units, coupon definitions, notional
    /// percentages, or call schedule entries.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: String,
        units: f64,
        leg: Leg<T>,
        currency: Currency,
        coupons: Vec<CashflowType<T>>,
        notional_percentages: Vec<f64>,
        mut call_schedule: Vec<CallScheduleEntry>,
    ) -> Result<Self> {
        if !units.is_finite() || units <= 0.0 {
            return Err(QSError::InvalidValueErr(
                "Stepped-coupon bond units must be finite and positive".into(),
            ));
        }
        if coupons.is_empty() {
            return Err(QSError::InvalidValueErr(
                "Stepped-coupon bond requires at least one coupon definition".into(),
            ));
        }
        for coupon in &coupons {
            coupon.validate_coupon()?;
        }
        validate_notional_percentages(&notional_percentages)?;
        let start_date = leg
            .cashflows()
            .iter()
            .find_map(|cashflow| match cashflow {
                CashflowType::Disbursement(disbursement) => Some(disbursement.payment_date()),
                _ => None,
            })
            .ok_or_else(|| {
                QSError::InvalidValueErr(
                    "Stepped-coupon bond leg requires an initial disbursement".into(),
                )
            })?;
        for call in &call_schedule {
            call.validate()?;
            if call.date() <= start_date || call.end_date() >= leg.last_payment_date() {
                return Err(QSError::InvalidValueErr(format!(
                    "Bond call window {} through {} must follow start {} and precede final payment {}",
                    call.date(),
                    call.end_date(),
                    start_date,
                    leg.last_payment_date()
                )));
            }
        }
        call_schedule.sort_by_key(CallScheduleEntry::date);
        if call_schedule
            .windows(2)
            .any(|calls| calls[0].end_date() >= calls[1].date())
        {
            return Err(QSError::InvalidValueErr(
                "Bond call schedule contains overlapping inclusive windows".into(),
            ));
        }
        Ok(Self {
            identifier,
            units,
            leg,
            currency,
            coupons,
            notional_percentages,
            call_schedule,
        })
    }

    /// Returns the quote units, normally 100.
    #[must_use]
    pub const fn units(&self) -> f64 {
        self.units
    }

    /// Returns the contractual cashflow leg.
    #[must_use]
    pub const fn leg(&self) -> &Leg<T> {
        &self.leg
    }

    /// Returns the positional coupon definitions.
    #[must_use]
    pub fn coupons(&self) -> &[CashflowType<T>] {
        &self.coupons
    }

    /// Returns the positional outstanding-notional percentages.
    #[must_use]
    pub fn notional_percentages(&self) -> &[f64] {
        &self.notional_percentages
    }

    /// Returns the explicit issuer call dates and strikes.
    #[must_use]
    pub fn call_schedule(&self) -> &[CallScheduleEntry] {
        &self.call_schedule
    }

    /// Returns whether the issuer has at least one call date.
    #[must_use]
    pub const fn is_callable(&self) -> bool {
        !self.call_schedule.is_empty()
    }
}

impl<T: Scalar> Instrument for SteppedCouponBond<T> {
    fn identifier(&self) -> String {
        self.identifier.clone()
    }
}

impl<T: Scalar> LegsProvider<T> for SteppedCouponBond<T> {
    fn legs(&self) -> &[Leg<T>] {
        std::slice::from_ref(&self.leg)
    }
}

impl<T: Scalar> Discountable for SteppedCouponBond<T> {
    fn currency(&self) -> Currency {
        self.currency
    }

    fn asset_class(&self) -> AssetClass {
        AssetClass::FixedIncome
    }
}

/// Trade wrapper for a [`SteppedCouponBond`].
pub struct SteppedCouponBondTrade<T: Scalar> {
    instrument: SteppedCouponBond<T>,
    trade_date: Date,
    notional: f64,
    side: Side,
}

impl<T: Scalar> SteppedCouponBondTrade<T> {
    /// Creates a stepped-coupon bond trade.
    #[must_use]
    pub const fn new(
        instrument: SteppedCouponBond<T>,
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

    /// Returns the trade notional.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }
}

impl<T: Scalar> Trade<SteppedCouponBond<T>> for SteppedCouponBondTrade<T> {
    fn instrument(&self) -> &SteppedCouponBond<T> {
        &self.instrument
    }

    fn trade_date(&self) -> Date {
        self.trade_date
    }

    fn side(&self) -> Side {
        self.side
    }
}

impl<T: Scalar> LegsProvider<T> for SteppedCouponBondTrade<T> {
    fn legs(&self) -> &[Leg<T>] {
        self.instrument.legs()
    }
}

impl From<SteppedCouponBond<f64>> for SteppedCouponBond<DualFwd> {
    fn from(value: SteppedCouponBond<f64>) -> Self {
        Self {
            identifier: value.identifier,
            units: value.units,
            leg: value.leg.into(),
            currency: value.currency,
            coupons: value.coupons.into_iter().map(Into::into).collect(),
            notional_percentages: value.notional_percentages,
            call_schedule: value.call_schedule,
        }
    }
}

impl From<SteppedCouponBond<DualFwd>> for SteppedCouponBond<f64> {
    fn from(value: SteppedCouponBond<DualFwd>) -> Self {
        Self {
            identifier: value.identifier,
            units: value.units,
            leg: value.leg.into(),
            currency: value.currency,
            coupons: value.coupons.into_iter().map(Into::into).collect(),
            notional_percentages: value.notional_percentages,
            call_schedule: value.call_schedule,
        }
    }
}

impl From<SteppedCouponBondTrade<f64>> for SteppedCouponBondTrade<DualFwd> {
    fn from(value: SteppedCouponBondTrade<f64>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

impl From<SteppedCouponBondTrade<DualFwd>> for SteppedCouponBondTrade<f64> {
    fn from(value: SteppedCouponBondTrade<DualFwd>) -> Self {
        Self::new(
            value.instrument.into(),
            value.trade_date,
            value.notional,
            value.side,
        )
    }
}

/// Validates original-principal fractions and their amortization ordering.
fn validate_notional_percentages(percentages: &[f64]) -> Result<()> {
    if percentages.is_empty() {
        return Err(QSError::InvalidValueErr(
            "Stepped-coupon bond requires at least one notional percentage".into(),
        ));
    }
    for percentage in percentages {
        if !percentage.is_finite() || *percentage <= 0.0 || *percentage > 1.0 {
            return Err(QSError::InvalidValueErr(format!(
                "Outstanding-notional percentages must be in (0, 1], got {percentage}"
            )));
        }
    }
    if (percentages[0] - 1.0).abs() > 1.0e-12 {
        return Err(QSError::InvalidValueErr(
            "The first outstanding-notional percentage must be 1.0".into(),
        ));
    }
    if percentages.windows(2).any(|values| values[1] > values[0]) {
        return Err(QSError::InvalidValueErr(
            "Outstanding-notional percentages must be non-increasing".into(),
        ));
    }
    Ok(())
}
