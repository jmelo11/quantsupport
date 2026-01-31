use serde::{Deserialize, Serialize};

use crate::{
    cashflows::{
        cashflow::{Cashflow, Side},
        traits::InterestAccrual,
    },
    core::traits::HasCurrency,
    currencies::enums::Currency,
    rates::interestrate::RateDefinition,
    time::{date::Date, enums::Frequency},
    visitors::traits::HasCashflows,
};

use super::traits::Structure;
use crate::utils::errors::Result;

/// # `FloatingRateInstrument`
/// A floating rate instrument.
///
/// ## Parameters
/// * `start_date` - The start date.
/// * `end_date` - The end date.
/// * `notional` - The notional.
/// * `spread` - The spread.
/// * `side` - The side.
/// * `cashflows` - The cashflows.
/// * `payment_frequency` - The payment frequency.
/// * `rate_definition` - The rate definition.
/// * `structure` - The structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloatingRateInstrument {
    start_date: Date,
    end_date: Date,
    notional: f64,
    spread: f64,
    side: Side,
    cashflows: Vec<Cashflow>,
    payment_frequency: Frequency,
    rate_definition: RateDefinition,
    structure: Structure,
    currency: Currency,
    discount_curve_id: Option<usize>,
    forecast_curve_id: Option<usize>,
    id: Option<String>,
    issue_date: Option<Date>,
}

impl FloatingRateInstrument {
    /// Creates a new `FloatingRateInstrument`.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    // allowed: high-arity API; refactor deferred
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start_date: Date,
        end_date: Date,
        notional: f64,
        spread: f64,
        side: Side,
        cashflows: Vec<Cashflow>,
        payment_frequency: Frequency,
        rate_definition: RateDefinition,
        structure: Structure,
        currency: Currency,
        discount_curve_id: Option<usize>,
        forecast_curve_id: Option<usize>,
        id: Option<String>,
        issue_date: Option<Date>,
    ) -> Self {
        Self {
            start_date,
            end_date,
            notional,
            spread,
            side,
            cashflows,
            payment_frequency,
            rate_definition,
            structure,
            currency,
            discount_curve_id,
            forecast_curve_id,
            id,
            issue_date,
        }
    }

    /// Returns the issue date.
    #[must_use]
    pub const fn issue_date(&self) -> Option<Date> {
        self.issue_date
    }

    /// Returns the identifier.
    #[must_use]
    pub fn id(&self) -> Option<String> {
        self.id.clone()
    }

    /// Returns the start date.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the end date.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        self.end_date
    }

    /// Returns the notional.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Returns the spread.
    #[must_use]
    pub const fn spread(&self) -> f64 {
        self.spread
    }

    /// Returns the side.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the payment frequency.
    #[must_use]
    pub const fn payment_frequency(&self) -> Frequency {
        self.payment_frequency
    }

    /// Returns the rate definition.
    #[must_use]
    pub const fn rate_definition(&self) -> RateDefinition {
        self.rate_definition
    }

    /// Returns the structure.
    #[must_use]
    pub const fn structure(&self) -> Structure {
        self.structure
    }

    /// Returns the discount curve identifier.
    #[must_use]
    pub const fn discount_curve_id(&self) -> Option<usize> {
        self.discount_curve_id
    }

    /// Returns the forecast curve identifier.
    #[must_use]
    pub const fn forecast_curve_id(&self) -> Option<usize> {
        self.forecast_curve_id
    }

    /// Sets the discount curve identifier and updates all cashflows.
    #[must_use]
    pub fn set_discount_curve_id(mut self, discount_curve_id: usize) -> Self {
        self.discount_curve_id = Some(discount_curve_id);
        self.mut_cashflows()
            .iter_mut()
            .for_each(|cf| cf.set_discount_curve_id(discount_curve_id));
        self
    }

    /// Sets the forecast curve identifier and updates all cashflows.
    #[must_use]
    pub fn set_forecast_curve_id(mut self, forecast_curve_id: usize) -> Self {
        self.forecast_curve_id = Some(forecast_curve_id);
        self.mut_cashflows()
            .iter_mut()
            .for_each(|cf| cf.set_forecast_curve_id(forecast_curve_id));
        self
    }

    /// Sets the spread and updates all floating rate coupons.
    #[must_use]
    pub fn set_spread(mut self, spread: f64) -> Self {
        self.spread = spread;
        self.mut_cashflows().iter_mut().for_each(|cf| {
            if let Cashflow::FloatingRateCoupon(coupon) = cf {
                coupon.set_spread(spread);
            }
        });
        self
    }
}

impl HasCurrency for FloatingRateInstrument {
    fn currency(&self) -> Result<Currency> {
        Ok(self.currency)
    }
}

impl InterestAccrual for FloatingRateInstrument {
    fn accrual_start_date(&self) -> Result<Date> {
        Ok(self.start_date)
    }
    fn accrual_end_date(&self) -> Result<Date> {
        Ok(self.end_date)
    }
    fn accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        let total_accrued_amount = self.cashflows.iter().fold(0.0, |acc, cf| {
            acc + cf.accrued_amount(start_date, end_date).unwrap_or(0.0)
        });
        Ok(total_accrued_amount)
    }
}

impl HasCashflows for FloatingRateInstrument {
    fn cashflows(&self) -> &[Cashflow] {
        &self.cashflows
    }

    fn mut_cashflows(&mut self) -> &mut [Cashflow] {
        &mut self.cashflows
    }
}

#[cfg(test)]
mod test {
    use crate::{
        cashflows::{
            cashflow::{Cashflow, Side},
            traits::{Payable, RequiresFixingRate},
        },
        core::traits::HasCurrency,
        currencies::enums::Currency,
        instruments::makefloatingrateinstrument::MakeFloatingRateInstrument,
        rates::{enums::Compounding, interestrate::RateDefinition},
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        utils::errors::Result,
        visitors::traits::HasCashflows,
    };

    #[test]
    fn test_float_rate_instrument() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Thirty360,
            Compounding::Simple,
            Frequency::Annual,
        );

        let spread = 0.04;

        let instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(spread)
            .with_notional(5_000_000.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        assert_eq!(instrument.start_date(), start_date);
        assert_eq!(instrument.end_date(), end_date);
        assert!((instrument.notional() - 5_000_000.0).abs() < 1e-12);
        assert!((instrument.spread() - spread).abs() < 1e-12);
        assert_eq!(instrument.side(), Side::Receive);
        assert_eq!(instrument.payment_frequency(), Frequency::Semiannual);
        assert_eq!(instrument.rate_definition(), rate_definition);
        assert_eq!(instrument.currency()?, Currency::USD);

        Ok(())
    }

    #[test]
    fn test_set_spread() -> Result<()> {
        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate_definition = RateDefinition::new(
            DayCounter::Thirty360,
            Compounding::Simple,
            Frequency::Annual,
        );

        let spread = 0.04;

        let mut instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_spread(spread)
            .with_notional(5_000_000.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        instrument
            .mut_cashflows()
            .iter_mut()
            .for_each(|cf| cf.set_fixing_rate(0.02));

        for cf in instrument.cashflows() {
            if let Cashflow::FloatingRateCoupon(coupon) = cf {
                assert!((coupon.amount()? - 150000.0).abs() < 1e-6);
                assert!((coupon.spread() - spread).abs() < 1e-12);
            }
        }

        let new_spread = 0.01;
        let new_instrument = instrument.set_spread(new_spread);

        for cf in new_instrument.cashflows() {
            if let Cashflow::FloatingRateCoupon(coupon) = cf {
                assert!((coupon.amount()? - 75000.0).abs() < 1e-6);
                assert!((coupon.spread() - new_spread).abs() < 1e-12);
            }
        }

        Ok(())
    }
}
