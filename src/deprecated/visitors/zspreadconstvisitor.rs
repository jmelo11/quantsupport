use argmin::{
    core::{CostFunction, Error, Executor, State},
    solver::brent::BrentOpt,
};

use crate::{
    cashflows::{cashflow::Cashflow, traits::Payable},
    core::{meta::MarketData, traits::Registrable},
    rates::interestrate::{InterestRate, RateDefinition},
    utils::errors::{AtlasError, Result},
};

use super::traits::{ConstVisit, HasCashflows};

/// # `ZSpreadConstVisitor`
/// `ZSpreadConstVisitor` is a visitor that calculates the `ZSpread` of a generic instrument.
///
/// ## Parameters
/// * `market_data` - The market data to use for evaluation
/// * `rate_definition` - The rate definition to use for the given spread
/// * `target` - The target npv to match the spread calculation
pub struct ZSpreadConstVisitor<'a> {
    market_data: &'a [MarketData],
    rate_definition: RateDefinition,
    target: f64,
}

impl<'a> ZSpreadConstVisitor<'a> {
    /// Creates a new `ZSpreadConstVisitor` with the given market data, rate definition, and target NPV.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(
        market_data: &'a [MarketData],
        rate_definition: RateDefinition,
        target: f64,
    ) -> Self {
        ZSpreadConstVisitor {
            market_data,
            rate_definition,
            target,
        }
    }
}

struct SpreadedNPV<'a, T> {
    eval: &'a T,
    market_data: &'a [MarketData],
    rate_definition: RateDefinition,
    target: f64,
}

impl<T> SpreadedNPV<'_, T>
where
    T: HasCashflows,
{
    fn cashflow_npv(&self, cf: &Cashflow, spread: f64) -> Result<f64> {
        let id = cf.id()?;
        let data = self
            .market_data
            .get(id)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Market data for cashflow with id {id}"
            )))?;

        let t = self
            .rate_definition
            .day_counter()
            .year_fraction(data.reference_date(), cf.payment_date());

        if t < 0.0 {
            return Ok(0.0);
        }

        let df = data.df()?;
        let implied_df_rate = InterestRate::implied_rate(
            1.0 / df,
            self.rate_definition.day_counter(),
            self.rate_definition.compounding(),
            self.rate_definition.frequency(),
            t,
        )?;

        let composite_rate = InterestRate::new(
            implied_df_rate.rate() + spread,
            self.rate_definition.compounding(),
            self.rate_definition.frequency(),
            self.rate_definition.day_counter(),
        );
        let final_df = 1.0 / composite_rate.compound_factor_from_yf(t);
        let flag = cf.side().sign();
        let cf_npv = cf.amount()? * final_df * flag;
        Ok(cf_npv)
    }
}

impl<T> CostFunction for SpreadedNPV<'_, T>
where
    T: HasCashflows,
{
    type Param = f64;
    type Output = f64;

    fn cost(&self, param: &Self::Param) -> std::result::Result<Self::Output, Error> {
        let npv = self
            .eval
            .cashflows()
            .iter()
            .try_fold(0.0, |acc, cf| -> Result<f64> {
                if let Cashflow::Disbursement(_) = cf {
                    Ok(acc)
                } else {
                    let cf_npv = self.cashflow_npv(cf, *param)?;
                    Ok(acc + cf_npv)
                }
            })?;
        Ok((npv - self.target).abs())
    }
}

impl<T> ConstVisit<T> for ZSpreadConstVisitor<'_>
where
    T: HasCashflows,
{
    type Output = Result<f64>;

    fn visit(&self, visitable: &T) -> Self::Output {
        let npv = SpreadedNPV {
            eval: visitable,
            market_data: self.market_data,
            rate_definition: self.rate_definition,
            target: self.target,
        };
        let solver = BrentOpt::new(-1.0, 1.0).set_tolerance(1e-6, 1e-6);
        let res = Executor::new(npv, solver)
            .configure(|state| state.max_iters(100).target_cost(0.0))
            .run()?;

        let best_param = res.state().get_best_param().ok_or_else(|| {
            AtlasError::EvaluationErr("ZSpread solver did not return best parameter".to_string())
        })?;
        Ok(*best_param)
    }
}

#[cfg(test)]
mod tests {

    use std::sync::{Arc, RwLock};

    use crate::{
        cashflows::cashflow::Side,
        core::marketstore::MarketStore,
        currencies::enums::Currency,
        instruments::makefixedrateinstrument::MakeFixedRateInstrument,
        models::{simplemodel::SimpleModel, traits::Model},
        rates::{
            enums::Compounding,
            interestrate::{InterestRate, RateDefinition},
            interestrateindex::iborindex::IborIndex,
            traits::HasReferenceDate,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{
            date::Date,
            daycounter::DayCounter,
            enums::{Frequency, TimeUnit},
            period::Period,
        },
        utils::errors::Result,
        visitors::{
            indexingvisitor::IndexingVisitor,
            traits::{ConstVisit, Visit},
        },
    };

    use super::ZSpreadConstVisitor;

    #[allow(dead_code)]
    pub fn create_store() -> Result<MarketStore> {
        let ref_date = Date::new(2021, 9, 1);
        let local_currency = Currency::USD;
        let mut market_store = MarketStore::new(ref_date, local_currency);

        let discount_curve = Arc::new(FlatForwardTermStructure::new(
            ref_date,
            0.05,
            RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Continuous,
                Frequency::Semiannual,
            ),
        ));

        let ibor_index = IborIndex::new(discount_curve.reference_date())
            .with_term_structure(discount_curve)
            .with_frequency(Frequency::Semiannual);

        market_store
            .mut_index_store()
            .add_index(0, Arc::new(RwLock::new(ibor_index)))?;
        Ok(market_store)
    }

    #[test]
    fn test() -> Result<()> {
        let start_date = Date::new(2021, 9, 1);
        let end_date = start_date + Period::new(5, TimeUnit::Years);

        let rate = InterestRate::from_rate_definition(
            0.06,
            RateDefinition::new(
                DayCounter::Actual360,
                Compounding::Continuous,
                Frequency::Semiannual,
            ),
        );

        let mut instrument = MakeFixedRateInstrument::new()
            .with_start_date(start_date)
            .with_end_date(end_date)
            .with_rate(rate)
            .with_notional(100.0)
            .with_side(Side::Receive)
            .with_currency(Currency::USD)
            .with_discount_curve_id(Some(0))
            .with_payment_frequency(Frequency::Semiannual)
            .bullet()
            .build()?;

        let market_store = create_store()?;
        let indexer = IndexingVisitor::new();
        let _ = indexer.visit(&mut instrument);

        let model = SimpleModel::new(&market_store);
        let data = model.gen_market_data(&indexer.request())?;
        let zspread_rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Continuous,
            Frequency::Semiannual,
        );
        let zspread_visitor = ZSpreadConstVisitor::new(&data, zspread_rate_definition, 100.0);

        let zspread = zspread_visitor.visit(&instrument)? * 100.0;
        println!("ZSpread: {zspread}");
        Ok(())
    }
}
