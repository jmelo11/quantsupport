use argmin::{
    core::{CostFunction, Error, Executor, State},
    solver::brent::BrentRoot,
};

use crate::{
    core::meta::MarketData,
    instruments::{
        fixedrateinstrument::FixedRateInstrument, floatingrateinstrument::FloatingRateInstrument,
        traits::Structure,
    },
    rates::interestrate::InterestRate,
    utils::errors::Result,
};

use super::{
    fixingvisitor::FixingVisitor,
    npvconstvisitor::NPVConstVisitor,
    traits::{ConstVisit, Visit},
};

/// # `ParValue`
/// `ParValue` is a cost function that calculates the NPV of a generic instrument.
///
/// ## Parameters
/// * `eval` - The instrument to evaluate
/// * `market_data` - The market data to use for evaluation
struct ParValue<'a, T> {
    eval: &'a T,
    npv_visitor: Box<NPVConstVisitor<'a>>,
    fixing_visitor: Box<FixingVisitor<'a>>,
}

impl<'a, T> ParValue<'a, T> {
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(eval: &'a T, market_data: &'a [MarketData]) -> Self {
        let npv_visitor = NPVConstVisitor::new(market_data, true);
        let fixing_visitor = FixingVisitor::new(market_data);
        ParValue {
            eval,
            npv_visitor: Box::new(npv_visitor),
            fixing_visitor: Box::new(fixing_visitor),
        }
    }
}

// cost function for fixed rate instrument
impl CostFunction for ParValue<'_, FixedRateInstrument> {
    type Param = f64;
    type Output = f64;
    fn cost(&self, param: &Self::Param) -> std::result::Result<Self::Output, Error> {
        let rate = self.eval.rate();
        let new_rate = InterestRate::new(
            *param,
            rate.compounding(),
            rate.frequency(),
            rate.day_counter(),
        );

        // new instrument with the new rate
        let inst = self.eval.clone().set_rate(new_rate);

        // visit the instrument to calculate the npv and return the result
        self.npv_visitor.visit(&inst).map_err(Error::from)
    }
}

// cost function for floating rate instrument
impl CostFunction for ParValue<'_, FloatingRateInstrument> {
    type Param = f64;
    type Output = f64;
    fn cost(&self, param: &Self::Param) -> std::result::Result<Self::Output, Error> {
        let new_spread = *param;

        // new instrument with the new spread
        let mut inst = self.eval.clone().set_spread(new_spread);

        // visit the instrument to update the fixing values
        let _ = self.fixing_visitor.visit(&mut inst);

        // visit the instrument to calculate the npv and return the result
        self.npv_visitor.visit(&inst).map_err(Error::from)
    }
}

/// # `ParValueConstVisitor`
/// `ParValueConstVisitor` is a visitor that calculates the par rate or spread of an instrument.
pub struct ParValueConstVisitor<'a> {
    market_data: &'a [MarketData],
}

impl<'a> ParValueConstVisitor<'a> {
    /// Creates a new `ParValueConstVisitor` with the given market data.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(market_data: &'a [MarketData]) -> Self {
        Self { market_data }
    }
}

impl ConstVisit<FixedRateInstrument> for ParValueConstVisitor<'_> {
    type Output = Result<f64>;
    // visit fixed rate instrument
    // use BrentRoot solver to find the par rate
    fn visit(&self, instrument: &FixedRateInstrument) -> Self::Output {
        let (min, max) = match instrument.structure() {
            Structure::EqualPayments => (-0.7, 0.7),
            _ => (-1.0, 1.0),
        };

        let cost = ParValue::new(instrument, self.market_data);
        let solver = BrentRoot::new(min, max, 1e-6);
        let res = Executor::new(cost, solver)
            .configure(|state| state.max_iters(100).target_cost(0.0))
            .run()?;

        let best_param = res.state().get_best_param().copied().ok_or_else(|| {
            crate::utils::errors::AtlasError::EvaluationErr(
                "No optimal parameter found in ParValueConstVisitor for FixedRateInstrument"
                    .to_string(),
            )
        })?;

        Ok(best_param)
    }
}

impl ConstVisit<FloatingRateInstrument> for ParValueConstVisitor<'_> {
    type Output = Result<f64>;
    // visit floating rate instrument
    // use BrentRoot solver to find the par spread
    fn visit(&self, instrument: &FloatingRateInstrument) -> Self::Output {
        let (min, max) = (-1.0, 1.0);
        let cost = ParValue::new(instrument, self.market_data);
        let solver = BrentRoot::new(min, max, 1e-6);
        let res = Executor::new(cost, solver)
            .configure(|state| state.max_iters(100).target_cost(0.0))
            .run()?;

        let best_param = res.state().get_best_param().copied().ok_or_else(|| {
            crate::utils::errors::AtlasError::EvaluationErr(
                "No optimal parameter found in ParValueConstVisitor for FloatingRateInstrument"
                    .to_string(),
            )
        })?;

        Ok(best_param)
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    };

    use crate::{
        cashflows::cashflow::Side,
        core::marketstore::MarketStore,
        currencies::enums::Currency,
        instruments::{
            makefixedrateinstrument::MakeFixedRateInstrument,
            makefloatingrateinstrument::MakeFloatingRateInstrument,
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
            RateDefinition::new(
                DayCounter::Thirty360,
                Compounding::Compounded,
                Frequency::Annual,
            ),
        ));

        let forecast_curve_2 = Arc::new(FlatForwardTermStructure::new(
            ref_date,
            0.03,
            RateDefinition::new(
                DayCounter::Thirty360,
                Compounding::Compounded,
                Frequency::Annual,
            ),
        ));

        let discount_curve = Arc::new(FlatForwardTermStructure::new(
            ref_date,
            0.05,
            RateDefinition::new(
                DayCounter::Thirty360,
                Compounding::Compounded,
                Frequency::Annual,
            ),
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
    fn test_par_value_fixed_equal_payment() -> Result<()> {
        let market_store = create_store()?;
        let ref_date = market_store.reference_date();

        let start_date = ref_date;
        let end_date = start_date + Period::new(10, TimeUnit::Years);
        let notional = 100_000.0;
        let rate = InterestRate::new(
            0.03,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let mut instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate(rate)
            .with_payment_frequency(Frequency::Semiannual)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .with_discount_curve_id(Some(2))
            .with_notional(notional)
            .equal_payments()
            .build()?;

        let indexer = IndexingVisitor::new();
        indexer.visit(&mut instrument)?;

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let parvaluevisitor = ParValueConstVisitor::new(&data);
        let par_value = parvaluevisitor.visit(&instrument)?;

        assert!((par_value - 0.05).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_par_value_fixed_bullet() -> Result<()> {
        let market_store = create_store()?;
        let ref_date = market_store.reference_date();

        let start_date = ref_date;
        let end_date = start_date + Period::new(10, TimeUnit::Years);
        let notional = 100_000.0;
        let rate = InterestRate::new(
            0.03,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );
        let mut instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate(rate)
            .with_payment_frequency(Frequency::Semiannual)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .with_discount_curve_id(Some(2))
            .with_notional(notional)
            .bullet()
            .build()?;
        let indexer = IndexingVisitor::new();
        indexer.visit(&mut instrument)?;

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let parvaluevisitor = ParValueConstVisitor::new(&data);
        let par_value = parvaluevisitor.visit(&instrument)?;

        assert!((par_value - 0.05).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_par_value_fixed_bullet_negative_rate() -> Result<()> {
        let market_store = create_store()?;
        let ref_date = market_store.reference_date();

        let start_date = ref_date;
        let end_date = start_date + Period::new(10, TimeUnit::Years);
        let notional = 100_000.0;
        let rate = InterestRate::new(
            -0.03,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Thirty360,
        );

        let mut instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate(rate)
            .with_payment_frequency(Frequency::Semiannual)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .with_discount_curve_id(Some(2))
            .with_notional(notional)
            .bullet()
            .build()?;
        let indexer = IndexingVisitor::new();
        indexer.visit(&mut instrument)?;

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let parvaluevisitor = ParValueConstVisitor::new(&data);
        let par_value = parvaluevisitor.visit(&instrument)?;

        assert!((par_value - 0.05).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_par_value_floating_bullet() -> Result<()> {
        let market_store = create_store()?;
        let ref_date = market_store.reference_date();

        let start_date = ref_date;
        let end_date = start_date + Period::new(10, TimeUnit::Years);
        let notional = 100_000.0;
        let rate_definition = RateDefinition::new(
            DayCounter::Thirty360,
            Compounding::Compounded,
            Frequency::Annual,
        );

        let spread = 0.04;

        let mut instrument = MakeFloatingRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate_definition(rate_definition)
            .with_payment_frequency(Frequency::Semiannual)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .with_discount_curve_id(Some(2))
            .with_forecast_curve_id(Some(0))
            .with_notional(notional)
            .with_spread(spread)
            .bullet()
            .build()?;

        let indexer = IndexingVisitor::new();
        indexer.visit(&mut instrument)?;

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let parvaluevisitor = ParValueConstVisitor::new(&data);
        let par_value = parvaluevisitor.visit(&instrument)?;

        assert!((par_value - 0.03).abs() < 1e-6);

        Ok(())
    }
}
