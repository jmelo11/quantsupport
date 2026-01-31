// use std::{
//     cell::RefCell,
//     collections::{hash_map::Entry, HashMap},
//     hash::Hash,
// };

// use crate::{
//     cashflows::{
//         cashflow::{Cashflow, Side},
//         traits::{InterestAccrual, Payable},
//     },
//     core::traits::{HasCurrency, HasDiscountCurveId, HasForecastCurveId},
//     currencies::enums::Currency,
//     instruments::{
//         hybridrateinstrument::HybridRateInstrument,
//         instrument::{Instrument, RateType},
//         traits::Structure,
//     },
//     rates::interestrate::{InterestRate, RateDefinition},
//     time::{date::Date, enums::Frequency},
//     utils::errors::AtlasError,
// };

// use super::traits::{ConstVisit, HasCashflows};
// use crate::utils::errors::Result;

// /// # `SimpleCashlowGroup`
// /// Struct that defines a cashflow group.
// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct SimpleCashlowGroup {
//     /// The discount curve identifier
//     pub discount_curve_id: Option<usize>,
//     /// The payment date
//     pub payment_date: Date,
//     /// The side of the cashflow
//     pub side: Side,
// }

// impl Hash for SimpleCashlowGroup {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.discount_curve_id.hash(state);
//         self.side.hash(state);
//     }
// }

// /// # `FixedRateCashflowGroup`
// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct FixedRateCashflowGroup {
//     /// The accrual start date
//     pub accrual_start_date: Date,
//     /// The accrual end date
//     pub accrual_end_date: Date,
//     /// The discount curve identifier
//     pub discount_curve_id: usize,
//     /// The rate definition
//     pub rate_definition: RateDefinition,
//     /// The side of the cashflow
//     pub side: Side,
// }

// impl Hash for FixedRateCashflowGroup {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.accrual_start_date.hash(state);
//         self.accrual_end_date.hash(state);
//         self.discount_curve_id.hash(state);
//         self.rate_definition.hash(state);
//         self.side.hash(state);
//     }
// }

// /// # `FloatingRateCashflowGroup`
// /// Struct that defines a floating rate cashflow group.
// #[derive(Clone, Debug, PartialEq, Eq)]
// pub struct FloatingRateCashflowGroup {
//     /// The accrual start date
//     pub accrual_start_date: Date,
//     /// The accrual end date
//     pub accrual_end_date: Date,
//     /// The fixing date
//     pub fixing_date: Date,
//     /// The discount curve identifier
//     pub discount_curve_id: usize,
//     /// The forecast curve identifier
//     pub forecast_curve_id: usize,
//     /// The rate definition
//     pub rate_definition: RateDefinition,
//     /// The side of the cashflow
//     pub side: Side,
// }

// impl Hash for FloatingRateCashflowGroup {
//     fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
//         self.accrual_start_date.hash(state);
//         self.accrual_end_date.hash(state);
//         self.fixing_date.hash(state);
//         self.discount_curve_id.hash(state);
//         self.forecast_curve_id.hash(state);
//         self.rate_definition.hash(state);
//         self.side.hash(state);
//     }
// }

// /// # `CashflowCompressorConstVisitor`
// /// This visitor is used to compress cashflows into groups to reduce the number of cashflows that need to be processed.
// ///
// /// ## Details
// /// The visitor compresses cashflows into groups based on the following criteria:
// /// - Disbursements: Cashflows are grouped based on the discount curve id and payment date.
// /// - Redemptions: Cashflows are grouped based on the discount curve id and payment date.
// /// - Fixed Rate Coupons: Cashflows are grouped based on the accrual start date, accrual end date, discount curve id, and rate definition.
// /// - Floating Rate Coupons: Cashflows are grouped based on the accrual start date, accrual end date, fixing date, discount curve id, forecast curve id, and rate definition.
// ///
// /// The visitor also calculates the estimated notional, start date, and end date of the instrument.
// /// Rates are recalculated for fixed rate coupons based on interest paid and notional. If its a floating rate coupon,
// /// the spread is calculated as a weighted average of the spreads.
// pub struct CashflowCompressorConstVisitor {
//     disbursements: RefCell<HashMap<SimpleCashlowGroup, Cashflow>>,
//     redemptions: RefCell<HashMap<SimpleCashlowGroup, Cashflow>>,
//     fixed_rate_coupons: RefCell<HashMap<FixedRateCashflowGroup, Cashflow>>,
//     floating_rate_coupons: RefCell<HashMap<FloatingRateCashflowGroup, Cashflow>>,
//     estimated_notional: RefCell<f64>,
//     estimaded_start_date: RefCell<Option<Date>>,
//     estimaded_end_date: RefCell<Option<Date>>,
//     currency: Currency,
// }

// impl CashflowCompressorConstVisitor {
//     /// Creates a new `CashflowCompressorConstVisitor` with the specified currency.
//     #[must_use]
//     pub fn new(currency: Currency) -> Self {
//         Self {
//             disbursements: RefCell::new(HashMap::new()),
//             redemptions: RefCell::new(HashMap::new()),
//             fixed_rate_coupons: RefCell::new(HashMap::new()),
//             floating_rate_coupons: RefCell::new(HashMap::new()),
//             estimated_notional: RefCell::new(0.0),
//             estimaded_start_date: RefCell::new(None),
//             estimaded_end_date: RefCell::new(None),
//             currency,
//         }
//     }

//     /// Converts the compressed cashflows into an `Instrument`.
//     ///
//     /// # Errors
//     /// Returns an error if the estimated start or end date has not been set
//     /// when building the instrument from the compressed cashflows.
//     pub fn as_instrument(&self) -> Result<Instrument> {
//         let mut cashflows = Vec::new();

//         cashflows.extend(self.disbursements.borrow().values().copied());
//         cashflows.extend(self.redemptions.borrow().values().copied());
//         cashflows.extend(self.fixed_rate_coupons.borrow().values().copied());
//         cashflows.extend(self.floating_rate_coupons.borrow().values().copied());

//         // Sort cashflows chronologically based on payment dates
//         cashflows.sort_by_key(Payable::payment_date);

//         // most of the fields do not make sense in this context
//         let instrument = Instrument::HybridRateInstrument(HybridRateInstrument::new(
//             self.estimaded_start_date
//                 .borrow()
//                 .ok_or(AtlasError::ValueNotSetErr("Start date".to_string()))?,
//             self.estimaded_end_date
//                 .borrow()
//                 .ok_or(AtlasError::ValueNotSetErr("End date".to_string()))?,
//             self.estimated_notional.borrow().abs(),
//             Frequency::OtherFrequency,
//             Structure::Other,
//             None,
//             Some(self.currency),
//             None,
//             None,
//             RateType::Suffled,
//             None,
//             None,
//             None,
//             None,
//             None,
//             None,
//             cashflows,
//         ));

//         Ok(instrument)
//     }
// }

// impl<T: HasCashflows> ConstVisit<T> for CashflowCompressorConstVisitor {
//     type Output = Result<()>;

//     #[allow(clippy::too_many_lines)]
//     fn visit(&self, visitable: &T) -> Self::Output {
//         visitable
//             .cashflows()
//             .iter()
//             .try_for_each(|&cf| -> Result<()> {
//                 // validate that the cashflow currency is the same as the instrument currency
//                 if cf.currency()? != self.currency {
//                     return Err(AtlasError::InvalidValueErr(format!(
//                         "Cashflow currency {cashflow_currency} does not match instrument currency {instrument_currency}",
//                         cashflow_currency = String::from(cf.currency()?),
//                         instrument_currency = String::from(self.currency)
//                     )));
//                 }

//                 let payment_date = cf.payment_date();
//                 let mut estimated_start_date = self.estimaded_start_date.borrow_mut();
//                 let mut estimated_end_date = self.estimaded_end_date.borrow_mut();
//                 if let Some(end_date) = *estimated_end_date {
//                     if payment_date > end_date {
//                         *estimated_end_date = Some(payment_date);
//                     }
//                 } else {
//                     *estimated_end_date = Some(payment_date);
//                 }

//                 if let Some(start_date) = *estimated_start_date {
//                     if payment_date < start_date {
//                         *estimated_start_date = Some(payment_date);
//                     }
//                 } else {
//                     *estimated_start_date = Some(payment_date);
//                 }

//                 match cf {
//                     Cashflow::Disbursement(disbursement) => {
//                         let disbursement_amount = disbursement.amount()?;
//                         let group = SimpleCashlowGroup {
//                             discount_curve_id: Some(disbursement.discount_curve_id()?),
//                             payment_date: disbursement.payment_date(),
//                             side: disbursement.side(),
//                         };
//                         let mut disbursements = self.disbursements.borrow_mut();
//                         match disbursements.entry(group) {
//                             Entry::Occupied(mut entry) => {
//                                 if let Cashflow::Disbursement(pos) = entry.get_mut() {
//                                     let new_amount = pos.amount()? + disbursement_amount;
//                                     pos.set_amount(new_amount);
//                                 }
//                             }
//                             Entry::Vacant(entry) => {
//                                 entry.insert(Cashflow::Disbursement(disbursement));
//                             }
//                         }
//                     }
//                     Cashflow::Redemption(redemption) => {
//                         let redemption_amount = redemption.amount()?;
//                         let group = SimpleCashlowGroup {
//                             discount_curve_id: Some(redemption.discount_curve_id()?),
//                             payment_date: redemption.payment_date(),
//                             side: redemption.side(),
//                         };
//                         let mut redemptions = self.redemptions.borrow_mut();
//                         match redemptions.entry(group) {
//                             Entry::Occupied(mut entry) => {
//                                 if let Cashflow::Redemption(pos) = entry.get_mut() {
//                                     let new_amount = pos.amount()? + redemption_amount;
//                                     pos.set_amount(new_amount);
//                                 }
//                             }
//                             Entry::Vacant(entry) => {
//                                 entry.insert(Cashflow::Redemption(redemption));
//                             }
//                         }

//                         let mut estimated_notional = self.estimated_notional.borrow_mut();
//                         *estimated_notional += redemption_amount * redemption.side().sign();
//                     }
//                     Cashflow::FixedRateCoupon(cf) => {
//                         let accrual_start_date = cf.accrual_start_date()?;
//                         let group = FixedRateCashflowGroup {
//                             accrual_start_date,
//                             accrual_end_date: cf.accrual_end_date()?,
//                             discount_curve_id: cf.discount_curve_id()?,
//                             rate_definition: cf.rate().rate_definition(),
//                             side: cf.side(),
//                         };
//                         let mut fixed_rate_coupons = self.fixed_rate_coupons.borrow_mut();
//                         match fixed_rate_coupons.entry(group) {
//                             Entry::Occupied(mut entry) => {
//                                 if let Cashflow::FixedRateCoupon(pos) = entry.get_mut() {
//                                     let interest = pos.amount()? + cf.amount()?;
//                                     let notional = pos.notional() + cf.notional();
//                                     let compound_factor = (notional + interest) / notional;
//                                     let accrual_end_date = cf.accrual_end_date()?;
//                                     let t = cf
//                                         .rate()
//                                         .day_counter()
//                                         .year_fraction(accrual_start_date, accrual_end_date);
//                                     let new_rate = InterestRate::implied_rate(
//                                         compound_factor,
//                                         cf.rate().rate_definition().day_counter(),
//                                         cf.rate().rate_definition().compounding(),
//                                         cf.rate().rate_definition().frequency(),
//                                         t,
//                                     )
//                                     .map_err(|e| {
//                                         AtlasError::EvaluationErr(format!(
//                                             "Failed to compute implied rate in CashflowCompressorConstVisitor: {e}",
//                                         ))
//                                     })?;
//                                     pos.set_rate(new_rate);
//                                     pos.set_notional(notional);
//                                 }
//                             }
//                             Entry::Vacant(entry) => {
//                                 entry.insert(Cashflow::FixedRateCoupon(cf));
//                             }
//                         }

//                         // check if start_accrual_date is less than the current estimated start date
//                         if let Some(start_date) = *estimated_start_date {
//                             if accrual_start_date < start_date {
//                                 *estimated_start_date = Some(accrual_start_date);
//                             }
//                         } else {
//                             *estimated_start_date = Some(accrual_start_date);
//                         }
//                     }
//                     Cashflow::FloatingRateCoupon(cf) => {
//                         let accrual_start_date = cf.accrual_start_date()?;
//                         let group = FloatingRateCashflowGroup {
//                             accrual_start_date,
//                             accrual_end_date: cf.accrual_end_date()?,
//                             fixing_date: cf.fixing_date(),
//                             discount_curve_id: cf.discount_curve_id()?,
//                             forecast_curve_id: cf.forecast_curve_id()?,
//                             rate_definition: cf.rate_definition(),
//                             side: cf.side(),
//                         };
//                         let mut floating_rate_coupons = self.floating_rate_coupons.borrow_mut();
//                         match floating_rate_coupons.entry(group) {
//                             Entry::Occupied(mut entry) => {
//                                 if let Cashflow::FloatingRateCoupon(pos) = entry.get_mut() {
//                                     let total = pos.notional() + cf.notional();
//                                     let w1 = pos.notional() / total;
//                                     let w2 = cf.notional() / total;
//                                     let spread = w1.mul_add(pos.spread(), w2 * cf.spread());
//                                     pos.set_spread(spread);
//                                     pos.set_notional(total);
//                                 }
//                             }
//                             Entry::Vacant(entry) => {
//                                 entry.insert(Cashflow::FloatingRateCoupon(cf));
//                             }
//                         }

//                         // check if start_accrual_date is less than the current estimated start date
//                         if let Some(start_date) = *estimated_start_date {
//                             if accrual_start_date < start_date {
//                                 *estimated_start_date = Some(accrual_start_date);
//                             }
//                         } else {
//                             *estimated_start_date = Some(accrual_start_date);
//                         }
//                     }
//                 }
//                 Ok(())
//             })?;

//         Ok(())
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     use crate::{
//         instruments::{
//             makefixedrateinstrument::MakeFixedRateInstrument,
//             makefloatingrateinstrument::MakeFloatingRateInstrument,
//         },
//         rates::enums::Compounding,
//         time::{daycounter::DayCounter, enums::TimeUnit, period::Period},
//         utils::errors::Result,
//     };

//     #[test]
//     fn test_two_fixed_bullet() -> Result<()> {
//         let start_date = Date::new(2020, 1, 1);
//         let end_date = start_date + Period::new(5, TimeUnit::Years);
//         let rate = InterestRate::new(
//             0.05,
//             Compounding::Compounded,
//             Frequency::Annual,
//             DayCounter::Actual360,
//         );
//         let instrument_a = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let instrument_b = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let visitor = CashflowCompressorConstVisitor::new(Currency::USD);
//         visitor.visit(&instrument_a)?;
//         visitor.visit(&instrument_b)?;

//         let instrument = visitor.as_instrument()?;
//         assert!((instrument.notional() - 200.0).abs() < 1e-12);
//         assert_eq!(instrument.start_date(), start_date);
//         assert_eq!(instrument.end_date(), end_date);
//         assert_eq!(instrument.currency()?, Currency::USD);
//         assert_eq!(instrument.cashflows().len(), instrument_a.cashflows().len());

//         Ok(())
//     }

//     #[test]
//     fn test_fixed_and_floating_bullet() -> Result<()> {
//         let start_date = Date::new(2020, 1, 1);
//         let end_date = start_date + Period::new(5, TimeUnit::Years);
//         let rate = InterestRate::new(
//             0.05,
//             Compounding::Compounded,
//             Frequency::Annual,
//             DayCounter::Actual360,
//         );

//         let instrument_a = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let instrument_b = MakeFloatingRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate_definition(RateDefinition::default())
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_forecast_curve_id(Some(1))
//             .with_spread(0.0)
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let visitor = CashflowCompressorConstVisitor::new(Currency::USD);
//         visitor.visit(&instrument_a)?;
//         visitor.visit(&instrument_b)?;

//         let instrument = visitor.as_instrument()?;
//         assert!((instrument.notional() - 200.0).abs() < 1e-12);
//         assert_eq!(instrument.start_date(), start_date);
//         assert_eq!(instrument.end_date(), end_date);
//         assert_eq!(instrument.currency()?, Currency::USD);

//         Ok(())
//     }

//     #[test]
//     fn test_multiple_fixed_rates() -> Result<()> {
//         let start_date = Date::new(2020, 1, 1);
//         let end_date = start_date + Period::new(5, TimeUnit::Years);
//         let rate1 = InterestRate::new(
//             0.05,
//             Compounding::Compounded,
//             Frequency::Annual,
//             DayCounter::Actual360,
//         );
//         let rate2 = InterestRate::new(
//             0.06,
//             Compounding::Compounded,
//             Frequency::Annual,
//             DayCounter::Actual360,
//         );
//         let instrument_a = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate1)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let instrument_b = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate2)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let visitor = CashflowCompressorConstVisitor::new(Currency::USD);
//         visitor.visit(&instrument_a)?;
//         visitor.visit(&instrument_b)?;

//         let instrument = visitor.as_instrument()?;
//         assert!((instrument.notional() - 200.0).abs() < 1e-12);
//         assert_eq!(instrument.start_date(), start_date);
//         assert_eq!(instrument.end_date(), end_date);
//         assert_eq!(instrument.currency()?, Currency::USD);
//         assert_eq!(instrument.cashflows().len(), instrument_a.cashflows().len());

//         instrument.cashflows().iter().for_each(|cf| {
//             if let Cashflow::FixedRateCoupon(cf) = cf {
//                 assert!((cf.rate().rate() - 0.055).abs() < 0.01);
//             }
//         });

//         Ok(())
//     }

//     #[test]
//     fn test_multiple_floating_spreads() -> Result<()> {
//         let start_date = Date::new(2020, 1, 1);
//         let end_date = start_date + Period::new(5, TimeUnit::Years);
//         let instrument_a = MakeFloatingRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate_definition(RateDefinition::default())
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_forecast_curve_id(Some(1))
//             .with_spread(0.01)
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let instrument_b = MakeFloatingRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate_definition(RateDefinition::default())
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_forecast_curve_id(Some(1))
//             .with_spread(0.02)
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let visitor = CashflowCompressorConstVisitor::new(Currency::USD);
//         visitor.visit(&instrument_a)?;
//         visitor.visit(&instrument_b)?;

//         let instrument = visitor.as_instrument()?;
//         assert!((instrument.notional() - 200.0).abs() < 1e-12);
//         assert_eq!(instrument.start_date(), start_date);
//         assert_eq!(instrument.end_date(), end_date);
//         assert_eq!(instrument.currency()?, Currency::USD);

//         instrument.cashflows().iter().for_each(|cf| {
//             if let Cashflow::FloatingRateCoupon(cf) = cf {
//                 assert!((cf.spread() - 0.015).abs() < 1e-2);
//             }
//         });

//         Ok(())
//     }

//     #[test]
//     fn test_different_rate_definitions() -> Result<()> {
//         let start_date = Date::new(2020, 1, 1);
//         let end_date = start_date + Period::new(5, TimeUnit::Years);
//         let rate1 = InterestRate::new(
//             0.05,
//             Compounding::Compounded,
//             Frequency::Annual,
//             DayCounter::Thirty360,
//         );
//         let rate2 = InterestRate::new(
//             0.06,
//             Compounding::Simple,
//             Frequency::Annual,
//             DayCounter::Actual360,
//         );
//         let instrument_a = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate1)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let instrument_b = MakeFixedRateInstrument::new()
//             .with_start_date(start_date)
//             .with_end_date(end_date)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_rate(rate2)
//             .with_notional(100.0)
//             .with_discount_curve_id(Some(0))
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .bullet()
//             .build()?;

//         let visitor = CashflowCompressorConstVisitor::new(Currency::USD);
//         visitor.visit(&instrument_a)?;
//         visitor.visit(&instrument_b)?;

//         let instrument = visitor.as_instrument()?;
//         assert!((instrument.notional() - 200.0).abs() < 1e-12);
//         assert_eq!(instrument.start_date(), start_date);

//         instrument.cashflows().iter().for_each(|cf| {
//             if let Cashflow::FixedRateCoupon(cf) = cf {
//                 if cf.rate().rate_definition().day_counter() == DayCounter::Thirty360 {
//                     assert!((cf.rate().rate() - 0.05).abs() < 1e-2);
//                 } else {
//                     assert!((cf.rate().rate() - 0.06).abs() < 1e-2);
//                 }
//             }
//         });

//         Ok(())
//     }
// }
