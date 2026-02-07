use crate::{
    ad::tape::Tape,
    core::{
        evaluationresults::{EvaluationResults, SensitivityKey, SensitivityMap},
        instrument::Instrument,
        pricer::Pricer,
        pricingdata::PricingDataContext,
        pricingrequest::{HandlePrice, HandleSensitivities, PricingRequest},
        trade::Trade,
    },
    instruments::fixedincome::deposit::DepositTrade,
    rates::yieldtermstructure::ratestermstructure::RatesTermStructure,
    utils::errors::{AtlasError, Result},
};

/// # `DiscountDepositPricer`
///
/// Implementation of pricer for deposit instruments.
pub struct DiscountDepositPricer;

/// # `DepositPriceEvaluationState`
///
/// Holds state information for deposit price evaluation.
pub struct DepositPriceEvaluationState {}

impl HandlePrice<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_price(
        &self,
        trade: &DepositTrade,
        ctx: &PricingDataContext,
        _state: &mut DepositPriceEvaluationState,
    ) -> Result<f64> {
        match trade.instrument().is_resolved() {
            false => Err(AtlasError::InstrumentResolutionErr(
                "Deposit instrument is not resolved".into(),
            )),
            true => {
                let deposit = trade.instrument();
                let start_date = deposit.start_date().unwrap_or(trade.trade_date());
                let maturity_date = deposit.maturity_date();
                let rate = deposit.rate();
                let compound_factor = rate.compound_factor(start_date, maturity_date);
                let discount_curve = ctx.discount_curve(trade.market_index())?;
                let discount = discount_curve.discount_factor(maturity_date)?;
                let cashflow = trade.notional() * compound_factor / deposit.units();
                Ok(cashflow * discount)
            }
        }
    }
}

impl HandleSensitivities<DepositTrade, DepositPriceEvaluationState> for DiscountDepositPricer {
    fn handle_sensitivities(
        &self,
        trade: &DepositTrade,
        ctx: &PricingDataContext,
        _state: &mut DepositPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        if !trade.instrument().is_resolved() {
            return Err(AtlasError::InstrumentResolutionErr(
                "Deposit instrument is not resolved".into(),
            ));
        }

        Tape::start_recording();

        let deposit = trade.instrument();
        let start_date = deposit.start_date().unwrap_or(trade.trade_date());
        let maturity_date = deposit.maturity_date();
        let rate = deposit.rate();
        let compound_factor = rate.compound_factor(start_date, maturity_date);
        let curve_inputs = ctx.discount_curve_inputs(trade.market_index())?;
        let discount = curve_inputs
            .curve()
            .discount_factor(maturity_date)?;
        let cashflow = trade.notional() * compound_factor / deposit.units();
        let price: crate::ad::adreal::ADReal = (discount * cashflow).into();

        price.backward()?;

        let mut sensitivities = SensitivityMap::new();
        for pillar in curve_inputs.pillars() {
            sensitivities.insert(
                SensitivityKey::new(
                    curve_inputs.market_index().clone(),
                    pillar.date(),
                ),
                pillar.value().adjoint()?,
            );
        }

        Tape::stop_recording();

        Ok(sensitivities)
    }
}

impl Pricer for DiscountDepositPricer {
    type Item = DepositTrade;

    fn evaluate(
        &self,
        trade: &DepositTrade,
        requests: &[PricingRequest],
        ctx: &PricingDataContext,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let depo = trade.instrument();
        let identifier = depo.identifier();
        
        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DepositPriceEvaluationState {};

        for request in requests {
            match request {
                PricingRequest::Price => {
                    let price = self.handle_price(trade, ctx, &mut state)?;
                    results = results.with_price(price);
                }

                PricingRequest::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, ctx, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::pricingdata::PricingDataContext,
        marketdata::{
            fixingprovider::FixingProvider,
            marketdataprovider::MarketDataProvider,
        },
        math::interpolation::interpolator::Interpolator,
        rates::compounding::Compounding,
        rates::interestrate::InterestRate,
        rates::yieldtermstructure::discounttermstructure::DiscountTermStructure,
        time::{date::Date, daycounter::DayCounter, enums::Frequency},
    };

    fn build_context(
        reference_date: Date,
        market_index: &crate::indices::marketindex::MarketIndex,
        discount_curve: DiscountTermStructure<f64>,
    ) -> PricingDataContext {
        let market_data = MarketDataProvider::new(reference_date);
        let mut context = PricingDataContext::new(market_data, FixingProvider::new(), 0);
        context.add_discount_curve(market_index.clone(), discount_curve);
        context
    }

    #[test]
    fn test_deposit_price_and_sensitivity() -> Result<()> {
        let start_date = Date::new(2024, 1, 1);
        let maturity_date = Date::new(2025, 1, 1);
        let trade_date = Date::new(2024, 1, 1);
        let rate_definition = crate::rates::interestrate::RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        );
        let deposit_rate = InterestRate::from_rate_definition(0.05, rate_definition);
        let deposit = crate::instruments::fixedincome::deposit::Deposit::new(
            "USD-DEPO".to_string(),
            1.0,
            deposit_rate,
            start_date,
            maturity_date,
        );
        let market_index = crate::indices::marketindex::MarketIndex::SOFR;
        let trade = DepositTrade::new(deposit, market_index.clone(), trade_date, 1_000_000.0);

        let curve_dates = vec![trade_date, maturity_date];
        let curve_dfs = vec![1.0, 0.97];
        let discount_curve = DiscountTermStructure::<f64>::new(
            curve_dates,
            curve_dfs,
            DayCounter::Actual360,
            Interpolator::Linear,
            true,
        )?;
        let ctx = build_context(trade_date, &market_index, discount_curve);
        let pricer = DiscountDepositPricer;
        let mut state = DepositPriceEvaluationState {};

        let price = pricer.handle_price(&trade, &ctx, &mut state)?;
        let expected_compound = InterestRate::from_rate_definition(0.05, rate_definition)
            .compound_factor(start_date, maturity_date);
        let expected_price = trade.notional() * expected_compound / trade.instrument().units();
        let expected_discount = 0.97;
        assert!((price - expected_price * expected_discount).abs() < 1e-10);

        let sensitivities = pricer.handle_sensitivities(&trade, &ctx, &mut state)?;
        let sensitivity_key = SensitivityKey::new(market_index.clone(), maturity_date);
        let sensitivity = sensitivities
            .get(&sensitivity_key)
            .copied()
            .expect("Missing pillar sensitivity");
        assert!((sensitivity - expected_price).abs() < 1e-10);

        Ok(())
    }
}
