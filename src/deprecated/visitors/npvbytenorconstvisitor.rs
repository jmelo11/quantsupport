use crate::{
    cashflows::traits::Payable,
    core::{meta::MarketData, traits::Registrable},
    time::period::Period,
    utils::errors::{AtlasError, Result},
};

use super::traits::{ConstVisit, HasCashflows};
use std::collections::BTreeMap;

/// # `NPVByTenorConstVisitor`
/// `NPVByTenorConstVisitor` is a visitor that calculates the NPV of an instrument by tenor.
/// Tenor is defined as a tuple of two periods: (start, end).
/// It assumes that the cashflows of the instrument have already been indexed and fixed.
pub struct NPVByTenorConstVisitor<'a> {
    market_data: &'a [MarketData],
    tenors: Vec<(Period, Period)>,
    include_today_cashflows: bool,
}

impl<'a> NPVByTenorConstVisitor<'a> {
    /// Creates a new `NPVByTenorConstVisitor` with the specified market data, tenors, and cashflow inclusion setting.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(
        market_data: &'a [MarketData],
        tenors: Vec<(Period, Period)>,
        include_today_cashflows: bool,
    ) -> Self {
        Self {
            market_data,
            tenors,
            include_today_cashflows,
        }
    }
    /// Sets whether cashflows on the reference date should be included in the NPV calculation.
    pub const fn set_include_today_cashflows(&mut self, include_today_cashflows: bool) {
        self.include_today_cashflows = include_today_cashflows;
    }

    /// Sets the tenors to be used for NPV calculation.
    pub fn set_tenors(&mut self, tenors: Vec<(Period, Period)>) {
        self.tenors = tenors;
    }

    /// Returns a copy of the current tenors.
    #[must_use]
    pub fn tenors(&self) -> Vec<(Period, Period)> {
        self.tenors.clone()
    }
}

impl<T: HasCashflows> ConstVisit<T> for NPVByTenorConstVisitor<'_> {
    type Output = Result<BTreeMap<(Period, Period), f64>>;
    fn visit(&self, visitable: &T) -> Self::Output {
        let reference_date = self.market_data[0].reference_date();

        let mut npv_result: BTreeMap<(Period, Period), f64> = BTreeMap::new();
        let tenors = self.tenors();

        for tenor in &tenors {
            npv_result.insert(*tenor, 0.0);
        }

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
                let flag = cf.side().sign();
                let amount = cf.amount()?;
                let npv = amount * df * fx * flag;

                for (key, value) in &mut npv_result {
                    if cf.payment_date() >= reference_date + key.0
                        && cf.payment_date() < reference_date + key.1
                    {
                        *value += npv;
                    }
                }

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

    use crate::{
        core::marketstore::MarketStore,
        currencies::enums::Currency,
        instruments::makefixedrateinstrument::MakeFixedRateInstrument,
        models::{simplemodel::SimpleModel, traits::Model},
        prelude::Side,
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
    fn test_npv_by_date_const_visitor() -> Result<()> {
        let market_store =
            create_store().unwrap_or_else(|e| panic!("market store creation should succeed: {e}"));
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

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;

        let tenors = vec![
            (
                Period::new(0, TimeUnit::Days),
                Period::new(1, TimeUnit::Months),
            ),
            (
                Period::new(1, TimeUnit::Months),
                Period::new(2, TimeUnit::Months),
            ),
            (
                Period::new(2, TimeUnit::Months),
                Period::new(3, TimeUnit::Months),
            ),
            (
                Period::new(6, TimeUnit::Months),
                Period::new(12, TimeUnit::Months),
            ),
            (
                Period::new(1, TimeUnit::Years),
                Period::new(2, TimeUnit::Years),
            ),
            (
                Period::new(2, TimeUnit::Years),
                Period::new(3, TimeUnit::Years),
            ),
            (
                Period::new(3, TimeUnit::Years),
                Period::new(5, TimeUnit::Years),
            ),
        ];

        let npv_visitor = NPVByTenorConstVisitor::new(&data, tenors, false);
        let npv_result_inst_1 = npv_visitor.visit(&instrument_1)?;

        println!("NPV by tenor: {npv_result_inst_1:?}");

        Ok(())
    }
}
