// use std::sync::Arc;

// use crate::{
//     math::interpolation::interpolator::Interpolator,
//     rates::{
//         enums::Compounding,
//         interestrate::{InterestRate, RateDefinition},
//         traits::{HasReferenceDate, YieldProvider},
//     },
//     time::{
//         date::Date,
//         enums::{Frequency, TimeUnit},
//         period::Period,
//     },
//     utils::errors::{AtlasError, Result},
// };

// use super::traits::{AdvanceTermStructureInTime, YieldTermStructureTrait};

// /// # `TenorBasedZeroRateTermStructure`
// /// A term structure of zero rates based on tenors.
// ///
// /// ## Parameters
// /// * `reference_date` - The reference date of the term structure
// /// * `tenors` - The tenors of the term structure
// /// * `spreads` - The spreads of the term structure
// /// * `rate_definition` - The rate definition of the term structure
// /// * `interpolation` - The interpolation method of the term structure
// /// * `enable_extrapolation` - Enable extrapolation
// #[derive(Clone)]
// pub struct TenorBasedZeroRateTermStructure {
//     reference_date: Date,
//     tenors: Vec<Period>,
//     spreads: Vec<f64>,
//     rate_definition: RateDefinition,
//     year_fractions: Vec<f64>,
//     interpolation: Interpolator,
//     enable_extrapolation: bool,
// }

// impl TenorBasedZeroRateTermStructure {
//     /// Creates a new `TenorBasedZeroRateTermStructure`.
//     ///
//     /// # Arguments
//     /// * `reference_date` - The reference date of the term structure
//     /// * `tenors` - The tenors of the term structure
//     /// * `spreads` - The spreads of the term structure
//     /// * `rate_definition` - The rate definition of the term structure
//     /// * `interpolation` - The interpolation method of the term structure
//     /// * `enable_extrapolation` - Enable extrapolation
//     ///
//     /// # Errors
//     /// Returns an error if the year fractions for the provided tenors
//     /// cannot be computed.
//     pub fn new(
//         reference_date: Date,
//         tenors: Vec<Period>,
//         spreads: Vec<f64>,
//         rate_definition: RateDefinition,
//         interpolation: Interpolator,
//         enable_extrapolation: bool,
//     ) -> Result<Self> {
//         let year_fractions = tenors
//             .iter()
//             .map(|x| {
//                 let date = reference_date + *x;
//                 rate_definition
//                     .day_counter()
//                     .year_fraction(reference_date, date)
//             })
//             .collect();

//         Ok(Self {
//             reference_date,
//             tenors,
//             spreads,
//             rate_definition,
//             year_fractions,
//             interpolation,
//             enable_extrapolation,
//         })
//     }

//     /// Returns the tenors of the term structure.
//     #[must_use]
//     pub const fn tenors(&self) -> &Vec<Period> {
//         &self.tenors
//     }

//     /// Returns the spreads of the term structure.
//     #[must_use]
//     pub const fn spreads(&self) -> &Vec<f64> {
//         &self.spreads
//     }
// }

// impl HasReferenceDate for TenorBasedZeroRateTermStructure {
//     fn reference_date(&self) -> Date {
//         self.reference_date
//     }
// }

// impl YieldProvider for TenorBasedZeroRateTermStructure {
//     fn discount_factor(&self, date: Date) -> Result<f64> {
//         let year_fraction = self
//             .rate_definition
//             .day_counter()
//             .year_fraction(self.reference_date(), date);

//         let spread = self.interpolation.interpolate(
//             year_fraction,
//             &self.year_fractions,
//             &self.spreads,
//             self.enable_extrapolation,
//         )?;
//         let rate = InterestRate::from_rate_definition(spread, self.rate_definition);
//         Ok(1.0 / rate.compound_factor(self.reference_date, date))
//     }

//     fn forward_rate(
//         &self,
//         start_date: Date,
//         end_date: Date,
//         comp: Compounding,
//         freq: Frequency,
//     ) -> Result<f64> {
//         let start_df = self.discount_factor(start_date)?;
//         let end_df = self.discount_factor(end_date)?;

//         let compound = start_df / end_df;
//         let t = self
//             .rate_definition
//             .day_counter()
//             .year_fraction(self.reference_date, end_date);
//         let rate = InterestRate::implied_rate(
//             compound,
//             self.rate_definition.day_counter(),
//             comp,
//             freq,
//             t,
//         )?;
//         Ok(rate.rate())
//     }
// }

// impl AdvanceTermStructureInTime for TenorBasedZeroRateTermStructure {
//     fn advance_to_period(&self, period: Period) -> Result<Arc<dyn YieldTermStructureTrait>> {
//         let new_reference_date = self.reference_date + period;
//         Ok(Arc::new(Self::new(
//             new_reference_date,
//             self.tenors.clone(),
//             self.spreads.clone(),
//             self.rate_definition,
//             self.interpolation,
//             self.enable_extrapolation,
//         )?))
//     }

//     fn advance_to_date(&self, date: Date) -> Result<Arc<dyn YieldTermStructureTrait>> {
//         let days = i32::try_from(date - self.reference_date)
//             .map_err(|_| AtlasError::InvalidValueErr("Day count should fit in i32".to_string()))?;
//         let period = Period::new(days, TimeUnit::Days);
//         self.advance_to_period(period)
//     }
// }

// impl YieldTermStructureTrait for TenorBasedZeroRateTermStructure {}

// #[cfg(test)]
// mod tests {
//     use crate::{
//         math::interpolation::interpolator::Interpolator,
//         rates::{
//             enums::Compounding, interestrate::RateDefinition, traits::YieldProvider,
//             yieldtermstructure::tenorbasedzeroratetermstructure::TenorBasedZeroRateTermStructure,
//         },
//         time::{
//             date::Date,
//             enums::{Frequency, TimeUnit},
//             period::Period,
//         },
//         utils::errors::Result,
//     };

//     #[test]
//     fn test_zero_rate() -> Result<()> {
//         let reference_date = Date::new(2021, 12, 1);
//         let rate_definition = RateDefinition::default();

//         let interpolation = Interpolator::Linear;
//         let enable_extrapolation = true;

//         let years = [1, 2, 3, 4, 5];
//         let spreads = [0.01, 0.02, 0.03, 0.04, 0.05];
//         let tenors = years
//             .iter()
//             .map(|x| Period::new(*x, TimeUnit::Years))
//             .collect();

//         let zero_rate_term_structure = TenorBasedZeroRateTermStructure::new(
//             reference_date,
//             tenors,
//             spreads.to_vec(),
//             rate_definition,
//             interpolation,
//             enable_extrapolation,
//         )?;

//         for (i, &x) in years.iter().enumerate() {
//             let forward_rate = zero_rate_term_structure
//                 .forward_rate(
//                     reference_date,
//                     reference_date + Period::new(x, TimeUnit::Years),
//                     Compounding::Simple,
//                     Frequency::Annual,
//                 )
//                 .unwrap_or_else(|e| {
//                     panic!("forward_rate should succeed in test_forward_rate_by_tenor: {e}")
//                 });
//             let expected_rate = spreads[i];
//             assert!((forward_rate - expected_rate).abs() < 1e-10);
//         }

//         Ok(())
//     }
// }
