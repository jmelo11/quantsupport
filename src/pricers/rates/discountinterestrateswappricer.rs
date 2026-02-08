use crate::{
    ad::adreal::{ADReal, IsReal},
    core::{
        assets::AssetType,
        contextmanager::ContextManager,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        pricer::Pricer,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::rates::swap::{InterestRateSwapTrade, SwapDirection},
    indices::rateindices::rate_index_details,
    rates::interest_rate_curve::InterestRateCurveAsset,
    rates::yieldtermstructure::interestratestermstructure::InterestRatesTermStructure,
    utils::errors::{AtlasError, Result},
};

/// # `DiscountInterestRateSwapPricer`
///
/// Discounting pricer for interest rate swaps.
pub struct DiscountInterestRateSwapPricer;

/// Swap pricing state.
#[derive(Default)]
struct SwapPriceEvaluationState {
    price: Option<ADReal>,
}

impl HandleValue<InterestRateSwapTrade, SwapPriceEvaluationState> for DiscountInterestRateSwapPricer {
    fn handle_value(
        &self,
        trade: &InterestRateSwapTrade,
        ctx: &ContextManager,
        state: &mut SwapPriceEvaluationState,
    ) -> Result<f64> {
        let swap = trade.instrument();
        let notional = ADReal::new(trade.notional());
        let discount_index = swap.discount_curve_index();
        let forecast_index = swap.market_index();
        let discount_asset = ctx
            .assets()
            .get(&discount_index)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Curve for {} not found.",
                discount_index
            )))?;
        let AssetType::InterestRateCurve(discount_asset) = discount_asset else {
            return Err(AtlasError::InvalidValueErr(
                "Expected interest rate curve asset.".into(),
            ));
        };
        let discount_curve = discount_asset
            .as_any()
            .downcast_ref::<InterestRateCurveAsset>()
            .ok_or(AtlasError::InvalidValueErr(
                "Invalid curve asset type.".into(),
            ))?
            .curve();
        let forecast_asset = ctx
            .assets()
            .get(&forecast_index)
            .ok_or(AtlasError::NotFoundErr(format!(
                "Curve for {} not found.",
                forecast_index
            )))?;
        let AssetType::InterestRateCurve(forecast_asset) = forecast_asset else {
            return Err(AtlasError::InvalidValueErr(
                "Expected interest rate curve asset.".into(),
            ));
        };
        let forecast_curve = forecast_asset
            .as_any()
            .downcast_ref::<InterestRateCurveAsset>()
            .ok_or(AtlasError::InvalidValueErr(
                "Invalid curve asset type.".into(),
            ))?
            .curve();

        let schedule = swap.fixed_schedule()?;
        let dates = schedule.dates();
        if dates.len() < 2 {
            return Err(AtlasError::InvalidValueErr(
                "Swap schedule must contain at least two dates.".into(),
            ));
        }

        let mut fixed_leg_pv = ADReal::zero();
        for window in dates.windows(2) {
            let start = window[0];
            let end = window[1];
            let accrual = swap.day_counter().year_fraction(start, end);
            let df = discount_curve.discount_factor(end)?;
            fixed_leg_pv = (fixed_leg_pv + df * ADReal::new(accrual)).into();
        }
        fixed_leg_pv =
            (fixed_leg_pv * ADReal::new(swap.fixed_rate()) * notional).into();

        let float_schedule = swap.float_schedule()?;
        let float_dates = float_schedule.dates();
        if float_dates.len() < 2 {
            return Err(AtlasError::InvalidValueErr(
                "Swap float schedule must contain at least two dates.".into(),
            ));
        }

        let index_details = rate_index_details(&forecast_index).ok_or(
            AtlasError::NotFoundErr("Rate index details not found.".into()),
        )?;
        let rate_definition = index_details.rate_definition().ok_or(
            AtlasError::NotFoundErr("Rate definition not found.".into()),
        )?;

        let mut float_leg_pv = ADReal::zero();
        for window in float_dates.windows(2) {
            let start = window[0];
            let end = window[1];
            let accrual = rate_definition.day_counter().year_fraction(start, end);
            let forward = forecast_curve.forward_rate(
                start,
                end,
                rate_definition.compounding(),
                rate_definition.frequency(),
            )?;
            let df = discount_curve.discount_factor(end)?;
            float_leg_pv = (float_leg_pv
                + df * forward * ADReal::new(accrual) * notional)
                .into();
        }

        let price: ADReal = match swap.direction() {
            SwapDirection::PayFixed => (float_leg_pv - fixed_leg_pv).into(),
            SwapDirection::ReceiveFixed => (fixed_leg_pv - float_leg_pv).into(),
        };
        state.price = Some(price);
        Ok(price.value())
    }
}

impl HandleSensitivities<InterestRateSwapTrade, SwapPriceEvaluationState>
    for DiscountInterestRateSwapPricer
{
    fn handle_sensitivities(
        &self,
        _trade: &InterestRateSwapTrade,
        _ctx: &ContextManager,
        state: &mut SwapPriceEvaluationState,
    ) -> Result<SensitivityMap> {
        match state.price {
            Some(price) => {
                let _ = price.backward()?;
                Ok(SensitivityMap::new())
            }
            None => Err(AtlasError::ValueNotSetErr("Pricing not requested".into())),
        }
    }
}

impl Pricer for DiscountInterestRateSwapPricer {
    type Item = InterestRateSwapTrade;

    fn evaluate(
        &self,
        trade: &InterestRateSwapTrade,
        requests: &[Request],
        ctx: &ContextManager,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let swap = trade.instrument();
        let identifier = swap.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = SwapPriceEvaluationState::default();

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, ctx, &mut state)?;
                    results = results.with_price(price);
                }
                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, ctx, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }
}
