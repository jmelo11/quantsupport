use crate::{
    cashflows::traits::Payable,
    core::{meta::MarketData, traits::Registrable},
    time::daycounter::DayCounter,
    utils::errors::{AtlasError, Result},
};

use super::traits::{ConstVisit, HasCashflows};

/// # `DurationConstVisitor`
/// `DurationConstVisitor` is a visitor that calculates the Duration of an instrument.
/// It assumes that the cashflows of the instrument have already been indexed and fixed.
///
/// ## Parameters
/// * `market_data` - The market data to use for Duration calculation
/// * `include_today_cashflows` - Flag to include cashflows with payment date equal to the reference date
pub struct DurationConstVisitor<'a> {
    market_data: &'a [MarketData],
}

impl<'a> DurationConstVisitor<'a> {
    /// Creates a new `DurationConstVisitor` with the given market data.
    ///
    /// # Arguments
    /// * `market_data` - A slice of market data to use for duration calculations
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(market_data: &'a [MarketData]) -> Self {
        DurationConstVisitor { market_data }
    }
}

impl<T: HasCashflows> ConstVisit<T> for DurationConstVisitor<'_> {
    type Output = Result<f64>;
    fn visit(&self, visitable: &T) -> Self::Output {
        let duration = visitable
            .cashflows()
            .iter()
            .try_fold((0.0, 0.0), |mut acc, cf| {
                let id = cf.id()?;

                let cf_market_data =
                    self.market_data
                        .get(id)
                        .ok_or(AtlasError::NotFoundErr(format!(
                            "Market data for cashflow with id {id}"
                        )))?;

                if cf_market_data.reference_date() <= cf.payment_date() {
                    return Ok(acc);
                }

                let year_fraction = DayCounter::Actual365
                    .year_fraction(cf_market_data.reference_date(), cf.payment_date());

                let df = cf_market_data.df()?;
                let fx = cf_market_data.fx()?;
                let flag = cf.side().sign();

                let aux_amount = cf.amount()? * df / fx * flag;

                acc.0 += aux_amount * year_fraction;
                acc.1 += aux_amount;

                Ok(acc)
            });

        match duration {
            Ok((d1, d2)) => Ok(d1 / d2),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {

    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    };

    use rayon::{
        prelude::{IntoParallelIterator, ParallelIterator},
        slice::ParallelSliceMut,
    };

    use crate::{
        cashflows::cashflow::Side,
        core::marketstore::MarketStore,
        currencies::enums::Currency,
        instruments::{
            fixedrateinstrument::FixedRateInstrument,
            makefixedrateinstrument::MakeFixedRateInstrument,
        },
        models::{simplemodel::SimpleModel, traits::Model},
        rates::{
            enums::Compounding,
            interestrate::{InterestRate, RateDefinition},
            interestrateindex::{iborindex::IborIndex, overnightindex::OvernightIndex},
            traits::HasReferenceDate,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        visitors::{indexingvisitor::IndexingVisitor, traits::Visit},
    };

    use super::*;

    pub fn create_store() -> Result<MarketStore> {
        let ref_date = Date::new(2021, 9, 1);
        let local_currency = Currency::USD;
        let mut market_store = MarketStore::new(ref_date, local_currency);

        let forecast_curve_1 = Arc::new(FlatForwardTermStructure::new(
            ref_date,
            0.02,
            RateDefinition::default(),
        ));

        let forecast_curve_2 = Arc::new(FlatForwardTermStructure::new(
            ref_date,
            0.03,
            RateDefinition::default(),
        ));

        let discount_curve = Arc::new(FlatForwardTermStructure::new(
            ref_date,
            0.05,
            RateDefinition::default(),
        ));

        let mut ibor_fixings = HashMap::new();
        ibor_fixings.insert(Date::new(2021, 9, 1), 0.02); // today
        ibor_fixings.insert(Date::new(2021, 8, 31), 0.02); // yesterday

        let ibor_index = IborIndex::new(forecast_curve_1.reference_date())
            .with_fixings(ibor_fixings)
            .with_term_structure(forecast_curve_1)
            .with_frequency(Frequency::Annual);

        let overnight_fixings =
            make_fixings(ref_date - Period::new(1, TimeUnit::Years), ref_date, 0.06);
        let overnigth_index = OvernightIndex::new(forecast_curve_2.reference_date())
            .with_term_structure(forecast_curve_2)
            .with_fixings(overnight_fixings);

        market_store
            .mut_index_store()
            .add_index(0, Arc::new(RwLock::new(ibor_index)))?;

        market_store
            .mut_index_store()
            .add_index(1, Arc::new(RwLock::new(overnigth_index)))?;

        let discount_index =
            IborIndex::new(discount_curve.reference_date()).with_term_structure(discount_curve);

        market_store
            .mut_index_store()
            .add_index(2, Arc::new(RwLock::new(discount_index)))?;
        Ok(market_store)
    }

    fn make_fixings(start: Date, end: Date, rate: f64) -> HashMap<Date, f64> {
        let mut fixings = HashMap::new();
        let mut seed = start;
        let mut init = 100.0;
        while seed <= end {
            fixings.insert(seed, init);
            seed = seed + Period::new(1, TimeUnit::Days);
            init *= 1.0 + rate * 1.0 / 360.0;
        }
        fixings
    }

    #[test]
    fn generator_tests() {
        fn duration(instruments: &mut [FixedRateInstrument]) -> f64 {
            let store = create_store()
                .unwrap_or_else(|e| panic!("market store creation should succeed: {e}"));
            let mut duration = 0.0;
            let indexer = IndexingVisitor::new();
            for inst in instruments.iter_mut() {
                indexer
                    .visit(inst)
                    .unwrap_or_else(|e| panic!("indexing visit should succeed: {e}"));
            }

            let model = SimpleModel::new(&store);
            let data = model
                .gen_market_data(&indexer.request())
                .unwrap_or_else(|e| panic!("market data generation should succeed: {e}"));

            let duration_visitor = DurationConstVisitor::new(&data);
            for inst in instruments.iter() {
                duration += duration_visitor
                    .visit(inst)
                    .unwrap_or_else(|e| panic!("duration visit should succeed: {e}"));
            }
            duration
        }

        let market_store =
            create_store().unwrap_or_else(|e| panic!("market store creation should succeed: {e}"));
        let ref_date = market_store.reference_date();

        let start_date = ref_date;
        let end_date = start_date + Period::new(10, TimeUnit::Years);
        let notional = 100000.0;
        let rate = InterestRate::new(
            0.05,
            Compounding::Simple,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        // par build
        let mut instruments: Vec<FixedRateInstrument> = (0..150000)
            .into_par_iter() // Create a parallel iterator
            .map(|_| {
                MakeFixedRateInstrument::new()
                    .with_start_date(start_date)
                    .with_end_date(end_date)
                    .with_rate(rate)
                    .with_payment_frequency(Frequency::Semiannual)
                    .with_side(Side::Receive)
                    .with_currency(Currency::USD)
                    .bullet()
                    .with_discount_curve_id(Some(2))
                    .with_notional(notional)
                    .build()
                    .unwrap_or_else(|e| panic!("instrument build should succeed: {e}"))
            })
            .collect(); // Collect the results into a Vec<_>

        instruments.par_rchunks_mut(1000).for_each(|chunk| {
            duration(chunk);
        });
    }
}
