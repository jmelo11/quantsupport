use crate::{
    cashflows::{cashflow::Side, traits::Payable},
    core::{meta::MarketData, traits::Registrable},
    time::date::Date,
    utils::errors::{AtlasError, Result},
};

use super::traits::{ConstVisit, HasCashflows};
use std::collections::BTreeMap;

/// # `NPVByDateConstVisitor`
/// `NPVByDateConstVisitor` is a visitor that calculates the NPV of an instrument and returns the result in a `BTreeMap`
/// where the key is the payment date of the cashflow and the value is the NPV of the cashflow.
/// It assumes that the cashflows of the instrument have already been indexed and fixed.
pub struct NPVByDateConstVisitor<'a> {
    market_data: &'a [MarketData],
    include_today_cashflows: bool,
    reference_date: Date,
}

impl<'a> NPVByDateConstVisitor<'a> {
    /// Creates a new `NPVByDateConstVisitor`.
    ///
    /// # Arguments
    ///
    /// * `reference_date` - The reference date for NPV calculations
    /// * `market_data` - A slice of market data for discount factors and FX rates
    /// * `include_today_cashflows` - Whether to include cashflows on the reference date
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(
        reference_date: Date,
        market_data: &'a [MarketData],
        include_today_cashflows: bool,
    ) -> Self {
        Self {
            market_data,
            include_today_cashflows,
            reference_date,
        }
    }
    /// Sets whether to include cashflows on the reference date.
    pub const fn set_include_today_cashflows(&mut self, include_today_cashflows: bool) {
        self.include_today_cashflows = include_today_cashflows;
    }
}

impl<T: HasCashflows> ConstVisit<T> for NPVByDateConstVisitor<'_> {
    type Output = Result<BTreeMap<Date, f64>>;
    fn visit(&self, visitable: &T) -> Self::Output {
        let mut npv_result = BTreeMap::new();
        npv_result.insert(self.reference_date, 0.0);
        visitable
            .cashflows()
            .iter()
            .try_for_each(|cf| -> Result<()> {
                let id = cf.id()?;
                let cf_market_data =
                    self.market_data
                        .get(id)
                        .ok_or(AtlasError::NotFoundErr(format!(
                            "Market data for cashflow with id {id}"
                        )))?;

                if cf_market_data.reference_date() == cf.payment_date()
                    && !self.include_today_cashflows
                    || cf.payment_date() < cf_market_data.reference_date()
                {
                    return Ok(());
                }

                let df = cf_market_data.df()?;
                let fx = cf_market_data.fx()?;
                let flag = match cf.side() {
                    Side::Pay => -1.0,
                    Side::Receive => 1.0,
                };
                let amount = cf.amount()?;
                let npv = amount * df * fx * flag;
                let acc = npv_result.entry(cf.payment_date()).or_insert(0.0);
                *acc += npv;
                Ok(())
            })?;
        Ok(npv_result)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    };

    use super::*;
    use crate::{
        core::marketstore::MarketStore,
        currencies::enums::Currency,
        instruments::makefixedrateinstrument::MakeFixedRateInstrument,
        models::{simplemodel::SimpleModel, traits::Model},
        rates::{
            enums::Compounding,
            interestrate::{InterestRate, RateDefinition},
            interestrateindex::{iborindex::IborIndex, overnightindex::OvernightIndex},
            traits::HasReferenceDate,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        visitors::{indexingvisitor::IndexingVisitor, traits::Visit},
    };

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
    fn test_npv_by_date_const_visitor_expired_instrument() -> Result<()> {
        let market_store = create_store()?;
        let indexer = IndexingVisitor::new();

        let start_date = Date::new(2010, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);

        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );

        let mut instrument_1 = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(100.0)
            .with_discount_curve_id(Some(0))
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        let _ = indexer.visit(&mut instrument_1);

        let mut instrument_2 = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(100.0)
            .with_discount_curve_id(Some(0))
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;
        let _ = indexer.visit(&mut instrument_2);

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let npv_visitor = NPVByDateConstVisitor::new(market_store.reference_date(), &data, false);
        let npv_result_inst_1 = npv_visitor.visit(&instrument_1)?;
        let npv_result_inst_2 = npv_visitor.visit(&instrument_2)?;

        assert_eq!(npv_result_inst_1.len(), 1);
        assert_eq!(npv_result_inst_2.len(), 1);

        Ok(())
    }

    #[test]
    fn test_npv_by_date_const_visitor() -> Result<()> {
        let market_store = create_store()?;
        let indexer = IndexingVisitor::new();

        let start_date = Date::new(2020, 1, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);

        let rate = InterestRate::new(
            0.05,
            Compounding::Compounded,
            Frequency::Annual,
            DayCounter::Actual360,
        );

        let mut instrument_1 = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Semiannual)
            .with_rate(rate)
            .with_notional(100.0)
            .with_discount_curve_id(Some(0))
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;

        let _ = indexer.visit(&mut instrument_1);

        let mut instrument_2 = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_payment_frequency(Frequency::Monthly)
            .with_rate(rate)
            .with_notional(100.0)
            .with_discount_curve_id(Some(0))
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .bullet()
            .build()?;
        let _ = indexer.visit(&mut instrument_2);

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let npv_visitor = NPVByDateConstVisitor::new(market_store.reference_date(), &data, false);
        let npv_result_inst_1 = npv_visitor.visit(&instrument_1)?;
        let npv_result_inst_2 = npv_visitor.visit(&instrument_2)?;

        assert_eq!(npv_result_inst_1.len(), 8);
        assert_eq!(npv_result_inst_2.len(), 41);

        Ok(())
    }
}
