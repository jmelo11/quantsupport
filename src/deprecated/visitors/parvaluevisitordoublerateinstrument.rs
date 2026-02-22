// use argmin::{
//     core::{CostFunction, Error, Executor, State},
//     solver::brent::BrentRoot,
// };

// use crate::{
//     cashflows::{cashflow::Cashflow, simplecashflow::SimpleCashflow, traits::Payable},
//     core::{
//         meta::MarketData,
//         traits::{HasCurrency, Registrable},
//     },
//     instruments::doublerateinstrument::DoubleRateInstrument,
//     utils::errors::Result,
// };

// use super::{
//     fixingvisitor::FixingVisitor,
//     npvconstvisitor::NPVConstVisitor,
//     traits::{ConstVisit, HasCashflows, Visit},
// };

// /// # `ParValue`
// /// `ParValue` is a cost function that calculates the NPV of a generic instrument.
// ///
// /// ## Parameters
// /// * `eval` - The instrument to evaluate
// /// * `market_data` - The market data to use for evaluation

// #[derive(Clone, Debug)]
// struct TmpInstrument {
//     cashflows: Vec<Cashflow>,
// }

// impl TmpInstrument {
//     #[allow(clippy::missing_const_for_fn)]
//     #[must_use]
//     pub fn new(cashflows: Vec<Cashflow>) -> Self {
//         Self { cashflows }
//     }
//     pub fn set_rate_value(mut self, rate: f64) -> Self {
//         self.mut_cashflows().iter_mut().for_each(|cf| match cf {
//             Cashflow::FixedRateCoupon(coupon) => coupon.set_rate_value(rate),
//             Cashflow::FloatingRateCoupon(coupon) => coupon.set_spread(rate),
//             _ => {}
//         });
//         self
//     }
// }

// impl HasCashflows for TmpInstrument {
//     fn cashflows(&self) -> &[Cashflow] {
//         &self.cashflows
//     }
//     fn mut_cashflows(&mut self) -> &mut [Cashflow] {
//         &mut self.cashflows
//     }
// }

// struct ParValue<'a, T> {
//     eval: &'a T,
//     npv_visitor: Box<NPVConstVisitor<'a>>,
//     fixing_visitor: Box<FixingVisitor<'a>>,
// }

// impl<'a, T> ParValue<'a, T> {
//     #[allow(clippy::missing_const_for_fn)]
//     #[must_use]
//     pub fn new(eval: &'a T, market_data: &'a [MarketData]) -> Self {
//         let npv_visitor = NPVConstVisitor::new(market_data, true);
//         let fixing_visitor = FixingVisitor::new(market_data);
//         ParValue {
//             eval,
//             npv_visitor: Box::new(npv_visitor),
//             fixing_visitor: Box::new(fixing_visitor),
//         }
//     }
// }

// // cost function for TmpInstrument
// impl CostFunction for ParValue<'_, TmpInstrument> {
//     type Param = f64;
//     type Output = f64;
//     fn cost(&self, param: &Self::Param) -> std::result::Result<Self::Output, Error> {
//         let mut inst = self.eval.clone().set_rate_value(*param);

//         // visit the instrument to update the fixing values
//         let _ = self.fixing_visitor.visit(&mut inst);

//         // visit the instrument to calculate the npv and return the result
//         self.npv_visitor.visit(&inst).map_err(Error::from)
//     }
// }

// /// # `ParValueConstVisitor`
// /// `ParValueConstVisitor` is a visitor that calculates the par rate or spread of a double-rate instrument.
// pub struct ParValueConstVisitor<'a> {
//     market_data: &'a [MarketData],
// }

// impl<'a> ParValueConstVisitor<'a> {
//     /// Creates a new `ParValueConstVisitor` with the given market data.
//     #[allow(clippy::missing_const_for_fn)]
//     #[must_use]
//     pub fn new(market_data: &'a [MarketData]) -> Self {
//         Self { market_data }
//     }
// }

// impl ConstVisit<DoubleRateInstrument> for ParValueConstVisitor<'_> {
//     type Output = Result<(f64, f64)>;
//     // visit double rate instrument
//     // use BrentRoot solver to find the par rate for the first and second rate
//     fn visit(&self, instrument: &DoubleRateInstrument) -> Self::Output {
//         let change_rate_date = instrument.change_rate_date();
//         let notional_at_change_rate = instrument.notional_at_change_rate().unwrap_or(0.0);
//         let currency = instrument.currency()?;
//         let side = instrument.side();

//         let (mut first_part_cashflows, mut second_part_cashflows): (Vec<Cashflow>, Vec<Cashflow>) =
//             instrument
//                 .cashflows()
//                 .iter()
//                 .copied()
//                 .partition(|cf| cf.payment_date() <= change_rate_date);
//         // buscar id de cashflow con fecha de pago igual a change rate date
//         let id = first_part_cashflows
//             .iter()
//             .filter(|cf| cf.payment_date() == change_rate_date)
//             .map(Cashflow::id)
//             .collect::<Result<Vec<usize>>>()?
//             .into_iter()
//             .next()
//             .ok_or_else(|| {
//                 crate::utils::errors::AtlasError::InvalidValueErr(
//                     "No cashflow found at change_rate_date in ParValueConstVisitor for DoubleRateInstrument"
//                         .to_string(),
//                 )
//             })?;

//         let sc = SimpleCashflow::new(change_rate_date, currency, side)
//             .with_amount(notional_at_change_rate)
//             .with_id(id);
//         first_part_cashflows.push(Cashflow::Redemption(sc));

//         let sc = SimpleCashflow::new(change_rate_date, currency, side.inverse())
//             .with_amount(notional_at_change_rate)
//             .with_id(id);
//         second_part_cashflows.push(Cashflow::Disbursement(sc));

//         let first_part_instrument = TmpInstrument::new(first_part_cashflows);
//         let second_part_instrument = TmpInstrument::new(second_part_cashflows);

//         let (min, max) = (-1.0, 1.0);

//         let cost = ParValue::new(&first_part_instrument, self.market_data);
//         let solver = BrentRoot::new(min, max, 1e-6);
//         let res = Executor::new(cost, solver)
//             .configure(|state| state.max_iters(100).target_cost(0.0))
//             .run()?;
//         let first_rate_par_value = res
//             .state()
//             .get_best_param()
//             .copied()
//             .ok_or_else(|| {
//                 crate::utils::errors::AtlasError::EvaluationErr(
//                     "No optimal parameter found for first rate in ParValueConstVisitor for DoubleRateInstrument"
//                         .to_string(),
//                 )
//             })?;

//         let cost = ParValue::new(&second_part_instrument, self.market_data);
//         let solver = BrentRoot::new(min, max, 1e-6);
//         let res = Executor::new(cost, solver)
//             .configure(|state| state.max_iters(100).target_cost(0.0))
//             .run()?;
//         let second_rate_par_value = res
//             .state()
//             .get_best_param()
//             .copied()
//             .ok_or_else(|| {
//                 crate::utils::errors::AtlasError::EvaluationErr(
//                     "No optimal parameter found for second rate in ParValueConstVisitor for DoubleRateInstrument"
//                         .to_string(),
//                 )
//             })?;

//         Ok((first_rate_par_value, second_rate_par_value))
//     }
// }

// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::{
//         cashflows::cashflow::Side,
//         core::marketstore::MarketStore,
//         currencies::enums::Currency,
//         instruments::{instrument::RateType, makedoublerateinstrument::MakeDoubleRateInstrument},
//         models::{simplemodel::SimpleModel, traits::Model},
//         rates::{
//             enums::Compounding,
//             interestrate::RateDefinition,
//             interestrateindex::{iborindex::IborIndex, overnightindex::OvernightIndex},
//             traits::HasReferenceDate,
//             yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
//         },
//         time::{
//             date::Date,
//             daycounter::DayCounter,
//             enums::{Frequency, TimeUnit},
//             period::Period,
//         },
//         visitors::indexingvisitor::IndexingVisitor,
//     };
//     use std::{
//         collections::HashMap,
//         sync::{Arc, RwLock},
//     };

//     pub fn create_store() -> Result<MarketStore> {
//         let ref_date = Date::new(2021, 9, 1);
//         let local_currency = Currency::USD;
//         let mut market_store = MarketStore::new(ref_date, local_currency);

//         let forecast_curve_1 = Arc::new(FlatForwardTermStructure::new(
//             ref_date,
//             0.02,
//             RateDefinition::new(
//                 DayCounter::Thirty360,
//                 Compounding::Compounded,
//                 Frequency::Annual,
//             ),
//         ));

//         let forecast_curve_2 = Arc::new(FlatForwardTermStructure::new(
//             ref_date,
//             0.03,
//             RateDefinition::new(
//                 DayCounter::Thirty360,
//                 Compounding::Compounded,
//                 Frequency::Annual,
//             ),
//         ));

//         let discount_curve = Arc::new(FlatForwardTermStructure::new(
//             ref_date,
//             0.05,
//             RateDefinition::new(
//                 DayCounter::Thirty360,
//                 Compounding::Compounded,
//                 Frequency::Annual,
//             ),
//         ));

//         let mut ibor_fixings = HashMap::new();
//         ibor_fixings.insert(Date::new(2021, 9, 1), 0.02); // today
//         ibor_fixings.insert(Date::new(2021, 8, 31), 0.02); // yesterday

//         let ibor_index = IborIndex::new(forecast_curve_1.reference_date())
//             .with_fixings(ibor_fixings)
//             .with_term_structure(forecast_curve_1)
//             .with_frequency(Frequency::Annual);

//         let overnight_fixings =
//             make_fixings(ref_date - Period::new(1, TimeUnit::Years), ref_date, 0.06);
//         let overnigth_index = OvernightIndex::new(forecast_curve_2.reference_date())
//             .with_term_structure(forecast_curve_2)
//             .with_fixings(overnight_fixings);

//         market_store
//             .mut_index_store()
//             .add_index(0, Arc::new(RwLock::new(ibor_index)))?;

//         market_store
//             .mut_index_store()
//             .add_index(1, Arc::new(RwLock::new(overnigth_index)))?;

//         let discount_index =
//             IborIndex::new(discount_curve.reference_date()).with_term_structure(discount_curve);

//         market_store
//             .mut_index_store()
//             .add_index(2, Arc::new(RwLock::new(discount_index)))?;
//         Ok(market_store)
//     }

//     fn make_fixings(start: Date, end: Date, rate: f64) -> HashMap<Date, f64> {
//         let mut fixings = HashMap::new();
//         let mut seed = start;
//         let mut init = 100.0;
//         while seed <= end {
//             fixings.insert(seed, init);
//             seed = seed + Period::new(1, TimeUnit::Days);
//             init *= 1.0 + rate * 1.0 / 360.0;
//         }
//         fixings
//     }

//     #[test]
//     fn test_par_value_floating_then_fixed_rate() -> Result<()> {
//         let market_store =
//             create_store().unwrap_or_else(|e| panic!("market store creation should succeed: {e}"));
//         let ref_date = market_store.reference_date();
//         let start_date = ref_date;

//         let rate_type = RateType::FloatingThenFixed;
//         let first_part_rate_definition = RateDefinition::new(
//             DayCounter::Thirty360,
//             Compounding::Compounded,
//             Frequency::Annual,
//         );

//         let first_part_rate = 0.05;
//         let second_part_rate_definition = RateDefinition::new(
//             DayCounter::Thirty360,
//             Compounding::Compounded,
//             Frequency::Annual,
//         );

//         let second_part_rate = 0.02;

//         let mut instrument = MakeDoubleRateInstrument::new()
//             .with_start_date(start_date)
//             .with_tenor(Period::new(9, TimeUnit::Years))
//             .with_tenor_change_rate(Period::new(4, TimeUnit::Years))
//             .with_tenor_grace_period(Period::new(2, TimeUnit::Years))
//             .with_rate_type(rate_type)
//             .with_first_part_rate_definition(first_part_rate_definition)
//             .with_first_part_rate(first_part_rate)
//             .with_second_part_rate_definition(second_part_rate_definition)
//             .with_second_part_rate(second_part_rate)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_notional(100.0)
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .with_discount_curve_id(Some(2))
//             .with_forecast_curve_id(Some(1))
//             .build()?;

//         let indexer = IndexingVisitor::new();
//         indexer.visit(&mut instrument)?;

//         let model = SimpleModel::new(&market_store);
//         let data = model.gen_market_data(&indexer.request())?;

//         let parvaluevisitor = ParValueConstVisitor::new(&data);
//         let (first_rate_par_value, second_rate_par_value) = parvaluevisitor.visit(&instrument)?;

//         print!(
//             "first_rate_par_value: {first_rate_par_value:?}, second_rate_par_value: {second_rate_par_value:?}"
//         );

//         assert!((first_rate_par_value - 0.02).abs() < 1e-6);
//         assert!((second_rate_par_value - 0.05).abs() < 1e-6);

//         Ok(())
//     }

//     #[test]
//     fn test_par_value_fixed_then_floating_rate() -> Result<()> {
//         let market_store =
//             create_store().unwrap_or_else(|e| panic!("market store creation should succeed: {e}"));
//         let ref_date = market_store.reference_date();
//         let start_date = ref_date;

//         let rate_type = RateType::FixedThenFloating;
//         let first_part_rate_definition = RateDefinition::new(
//             DayCounter::Thirty360,
//             Compounding::Compounded,
//             Frequency::Annual,
//         );

//         let first_part_rate = 0.05;
//         let second_part_rate_definition = RateDefinition::new(
//             DayCounter::Thirty360,
//             Compounding::Compounded,
//             Frequency::Annual,
//         );

//         let second_part_rate = 0.02;

//         let mut instrument = MakeDoubleRateInstrument::new()
//             .with_start_date(start_date)
//             .with_tenor(Period::new(9, TimeUnit::Years))
//             .with_tenor_change_rate(Period::new(4, TimeUnit::Years))
//             .with_tenor_grace_period(Period::new(2, TimeUnit::Years))
//             .with_rate_type(rate_type)
//             .with_first_part_rate_definition(first_part_rate_definition)
//             .with_first_part_rate(first_part_rate)
//             .with_second_part_rate_definition(second_part_rate_definition)
//             .with_second_part_rate(second_part_rate)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_notional(100.0)
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .with_discount_curve_id(Some(2))
//             .with_forecast_curve_id(Some(1))
//             .build()?;

//         let indexer = IndexingVisitor::new();
//         indexer.visit(&mut instrument)?;

//         let model = SimpleModel::new(&market_store);
//         let data = model.gen_market_data(&indexer.request())?;

//         let parvaluevisitor = ParValueConstVisitor::new(&data);
//         let (first_rate_par_value, second_rate_par_value) = parvaluevisitor.visit(&instrument)?;

//         print!(
//             "first_rate_par_value: {first_rate_par_value:?}, second_rate_par_value: {second_rate_par_value:?}"
//         );

//         assert!((first_rate_par_value - 0.05).abs() < 1e-6);
//         assert!((second_rate_par_value - 0.02).abs() < 1e-6);

//         Ok(())
//     }

//     #[test]
//     fn test_par_value_fixed_then_fixed_rate() -> Result<()> {
//         let market_store =
//             create_store().unwrap_or_else(|e| panic!("market store creation should succeed: {e}"));
//         let ref_date = market_store.reference_date();
//         let start_date = ref_date;

//         let rate_type = RateType::FixedThenFixed;
//         let first_part_rate_definition = RateDefinition::new(
//             DayCounter::Thirty360,
//             Compounding::Compounded,
//             Frequency::Annual,
//         );

//         let first_part_rate = 0.05;
//         let second_part_rate_definition = RateDefinition::new(
//             DayCounter::Thirty360,
//             Compounding::Compounded,
//             Frequency::Annual,
//         );

//         let second_part_rate = 0.02;

//         let mut instrument = MakeDoubleRateInstrument::new()
//             .with_start_date(start_date)
//             .with_tenor(Period::new(9, TimeUnit::Years))
//             .with_tenor_change_rate(Period::new(4, TimeUnit::Years))
//             .with_tenor_grace_period(Period::new(2, TimeUnit::Years))
//             .with_rate_type(rate_type)
//             .with_first_part_rate_definition(first_part_rate_definition)
//             .with_first_part_rate(first_part_rate)
//             .with_second_part_rate_definition(second_part_rate_definition)
//             .with_second_part_rate(second_part_rate)
//             .with_payment_frequency(Frequency::Semiannual)
//             .with_notional(100.0)
//             .with_side(Side::Receive)
//             .with_currency(Currency::USD)
//             .with_discount_curve_id(Some(2))
//             .with_forecast_curve_id(Some(1))
//             .build()?;

//         let indexer = IndexingVisitor::new();
//         indexer.visit(&mut instrument)?;

//         let model = SimpleModel::new(&market_store);
//         let data = model.gen_market_data(&indexer.request())?;

//         let parvaluevisitor = ParValueConstVisitor::new(&data);
//         let (first_rate_par_value, second_rate_par_value) = parvaluevisitor.visit(&instrument)?;

//         print!(
//             "first_rate_par_value: {first_rate_par_value:?}, second_rate_par_value: {second_rate_par_value:?}"
//         );

//         assert!((first_rate_par_value - 0.05).abs() < 1e-6);
//         assert!((second_rate_par_value - 0.05).abs() < 1e-6);

//         Ok(())
//     }
// }
