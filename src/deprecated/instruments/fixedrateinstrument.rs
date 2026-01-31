use serde::{Deserialize, Serialize};

use super::traits::Structure;
use crate::{
    cashflows::{
        cashflow::{Cashflow, Side},
        traits::Payable,
    },
    core::traits::HasCurrency,
    currencies::enums::Currency,
    rates::interestrate::InterestRate,
    time::{date::Date, enums::Frequency},
    utils::errors::{AtlasError, Result},
    visitors::traits::HasCashflows,
};

/// # `FixedRateInstrument`
/// A fixed rate instrument.
///
/// ## Parameters
/// * `start_date` - The start date.
/// * `end_date` - The end date.
/// * `notional` - The notional.
/// * `rate` - The rate.
/// * `cashflows` - The cashflows.
/// * `structure` - The structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixedRateInstrument {
    start_date: Date,
    end_date: Date,
    notional: f64,
    rate: InterestRate,
    payment_frequency: Frequency,
    cashflows: Vec<Cashflow>,
    structure: Structure,
    side: Side,
    currency: Currency,
    discount_curve_id: Option<usize>,
    id: Option<String>,
    issue_date: Option<Date>,
    yield_rate: Option<InterestRate>,
}

impl FixedRateInstrument {
    /// Creates a new `FixedRateInstrument` with the specified parameters.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    // allowed: high-arity API; refactor deferred
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start_date: Date,
        end_date: Date,
        notional: f64,
        rate: InterestRate,
        payment_frequency: Frequency,
        cashflows: Vec<Cashflow>,
        structure: Structure,
        side: Side,
        currency: Currency,
        discount_curve_id: Option<usize>,
        id: Option<String>,
        issue_date: Option<Date>,
        yield_rate: Option<InterestRate>,
    ) -> Self {
        Self {
            start_date,
            end_date,
            notional,
            rate,
            payment_frequency,
            cashflows,
            structure,
            side,
            currency,
            discount_curve_id,
            id,
            issue_date,
            yield_rate,
        }
    }

    /// Returns the identifier of this instrument.
    #[must_use]
    pub fn id(&self) -> Option<String> {
        self.id.clone()
    }

    /// Returns the start date of this instrument.
    #[must_use]
    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    /// Returns the end date (maturity) of this instrument.
    #[must_use]
    pub const fn end_date(&self) -> Date {
        self.end_date
    }

    /// Returns the notional amount of this instrument.
    #[must_use]
    pub const fn notional(&self) -> f64 {
        self.notional
    }

    /// Returns the fixed interest rate of this instrument.
    #[must_use]
    pub const fn rate(&self) -> InterestRate {
        self.rate
    }

    /// Returns the structure of this instrument.
    #[must_use]
    pub const fn structure(&self) -> Structure {
        self.structure
    }

    /// Returns the payment frequency of this instrument.
    #[must_use]
    pub const fn payment_frequency(&self) -> Frequency {
        self.payment_frequency
    }

    /// Returns the identifier of the discount curve used for valuation.
    #[must_use]
    pub const fn discount_curve_id(&self) -> Option<usize> {
        self.discount_curve_id
    }

    /// Returns the side (pay or receive) of this instrument.
    #[must_use]
    pub const fn side(&self) -> Side {
        self.side
    }

    /// Returns the issue date of this instrument.
    #[must_use]
    pub const fn issue_date(&self) -> Option<Date> {
        self.issue_date
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

    /// Sets the interest rate and updates all fixed rate coupons.
    #[must_use]
    pub fn set_rate(mut self, rate: InterestRate) -> Self {
        self.rate = rate;
        self.mut_cashflows().iter_mut().for_each(|cf| {
            if let Cashflow::FixedRateCoupon(coupon) = cf {
                coupon.set_rate(rate);
            }
        });
        self
    }
}

impl HasCurrency for FixedRateInstrument {
    fn currency(&self) -> Result<Currency> {
        Ok(self.currency)
    }
}

/// # `BondAccrual`
/// Implements fixed rate bond accrual using a yield rate.
/// The yield rate is used to discount the cashflows to between the start and
/// end dates and calculate the accrued amount.
pub trait BondAccrual: HasCashflows {
    /// Returns the yield rate used for bond accrual calculations.
    fn yield_rate(&self) -> Option<InterestRate>;

    /// Calculates the accrued amount for a bond between two dates.
    ///
    /// # Errors
    /// Returns an error if required rate data is missing to discount cashflows.
    fn bond_accrued_amount(&self, start_date: Date, end_date: Date) -> Result<f64> {
        let ini_pv = self.discounted_cashflows(start_date)?;
        let end_pv = self.discounted_cashflows(end_date)?;
        let accrual = self.matured_amount_accrual(start_date, end_date)?;
        // let maturing_cashflows = self
        //     .cashflows()
        //     .iter()
        //     .filter(|cf| cf.payment_date() == end_date)
        //     .fold(0.0, |acc, cf| acc + cf.amount().unwrap());
        // Ok(end_pv - ini_pv + maturing_cashflows)
        Ok(end_pv - ini_pv + accrual)
    }

    /// Calculates the accrual of cash paid between two dates.
    ///
    /// # Errors
    /// Returns an error if underlying cashflow data is unavailable.
    fn matured_amount_accrual(&self, from: Date, to: Date) -> Result<f64> {
        // let rate = self
        //     .yield_rate()
        //     .ok_or(AtlasError::NotFoundErr("Yield rate".to_string()))?;

        let cashflows = self
            .cashflows()
            .iter()
            .filter(|cf| cf.payment_date() >= from && cf.payment_date() < to)
            .collect::<Vec<&Cashflow>>();

        cashflows.iter().try_fold(0.0, |acc, cf| {
            //amount += cf.amount().unwrap() / rate.discount_factor(cf.payment_date(), to);
            Ok(acc + cf.amount()?)
        })
    }

    /// Calculates the present value of cashflows from the evaluation date forward using the yield rate.
    ///
    /// # Errors
    /// Returns an error if a yield rate is not available to discount cashflows.
    fn discounted_cashflows(&self, evaluation_date: Date) -> Result<f64> {
        let rate = self
            .yield_rate()
            .ok_or(AtlasError::NotFoundErr("Yield rate".to_string()))?;

        self
            .cashflows()
            .iter()
            .filter(|cf| cf.payment_date() >= evaluation_date)
            .try_fold(0.0, |acc, cf| {
                let npv = cf.amount()?
                    * rate.discount_factor(evaluation_date, cf.payment_date())
                    * cf.side().sign();
                Ok(acc + npv)
            })
    }
}

impl BondAccrual for FixedRateInstrument {
    fn yield_rate(&self) -> Option<InterestRate> {
        self.yield_rate
    }
}

impl HasCashflows for FixedRateInstrument {
    fn cashflows(&self) -> &[Cashflow] {
        &self.cashflows
    }

    fn mut_cashflows(&mut self) -> &mut [Cashflow] {
        &mut self.cashflows
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        cashflows::{
            cashflow::{Cashflow, Side},
            traits::Payable,
        },
        currencies::enums::Currency,
        instruments::{
            fixedrateinstrument::BondAccrual, makefixedrateinstrument::MakeFixedRateInstrument,
        },
        rates::{enums::Compounding, interestrate::InterestRate},
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
    fn bond_accrual_bullet_instrument() -> Result<()> {
        let start_date = Date::new(2024, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate = InterestRate::new(
            0.06,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let yield_rate = InterestRate::new(
            0.07,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(5_000_000.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .with_yield_rate(yield_rate)
            .bullet()
            .build()?;

        let date = start_date + Period::new(1, TimeUnit::Months);
        let mut accrual_aux =
            instrument.bond_accrued_amount(date, date + Period::new(1, TimeUnit::Months))?;
        assert!((accrual_aux - 27_385.1934467).abs() < 1e-6);

        let date = start_date + Period::new(2, TimeUnit::Months);
        accrual_aux =
            instrument.bond_accrued_amount(date, date + Period::new(1, TimeUnit::Months))?;
        assert!((accrual_aux - 27_540.0333112).abs() < 1e-6);

        let date = start_date + Period::new(3, TimeUnit::Months);
        accrual_aux =
            instrument.bond_accrued_amount(date, date + Period::new(6, TimeUnit::Months))?;
        assert!((accrual_aux - 165_982.433650).abs() < 1e-6);

        let date = start_date + Period::new(54, TimeUnit::Months);
        accrual_aux =
            instrument.bond_accrued_amount(date, date + Period::new(6, TimeUnit::Months))?;
        assert!((accrual_aux - 171_307.0814148).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_set_rate() -> Result<()> {
        let start_date = Date::new(2024, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);
        let rate = InterestRate::new(
            0.06,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(5_000_000.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        for cf in instrument.cashflows() {
            if let Cashflow::FixedRateCoupon(coupon) = cf {
                assert!((coupon.amount()? - 150000.0).abs() < 1e-6);
                assert_eq!(coupon.rate(), rate);
            }
        }

        let new_rate = InterestRate::new(
            0.03,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let new_instrument = instrument.set_rate(new_rate);

        for cf in new_instrument.cashflows() {
            if let Cashflow::FixedRateCoupon(coupon) = cf {
                assert!((coupon.amount()? - 75000.0).abs() < 1e-6);
                assert_eq!(coupon.rate(), new_rate);
            }
        }

        Ok(())
    }
}
