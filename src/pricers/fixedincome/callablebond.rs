//! Common model preparation for bond engines with issuer exercise.
use super::bondanalytics::{contractual_market_request, BondTerms, BondValuation};
use super::bondevents::{bond_event_grid, BondEventGrid};
use crate::{
    ad::{dual::DualFwd, scalar::Scalar},
    core::{
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricerconfiguration::{PricerConfiguration, TimeGridConfiguration},
    },
    indices::marketindex::MarketIndex,
    models::hullwhite::parameters::HullWhiteParameters,
    utils::errors::{QSError, Result},
    volatility::modelcalibration::CalibrationSource,
};
pub(super) struct CallableBondModel<T: Scalar> {
    pub(super) parameters: HullWhiteParameters<T>,
    time_grid: TimeGridConfiguration,
}
impl CallableBondModel<f64> {
    pub(super) fn ad_model(&self, market: &MarketData) -> Result<CallableBondModel<DualFwd>> {
        Ok(CallableBondModel {
            parameters: self.parameters.ad_parameters(market)?,
            time_grid: self.time_grid,
        })
    }
}
impl<T: Scalar> CallableBondModel<T> {
    pub(super) fn events(&self, valuation: &BondValuation<'_, T>) -> Result<BondEventGrid<T>> {
        bond_event_grid(
            valuation.leg,
            valuation.units,
            valuation.calls,
            valuation.cashflows,
            valuation.reference_date,
            self.time_grid,
        )
    }
}
pub(super) fn pricer_configuration(
    trade: &impl BondTerms,
    context: &impl MarketDataProvider,
) -> Result<PricerConfiguration> {
    let market = context.handle_request(
        &MarketDataRequest::default()
            .with_pricer_configuration_request(vec![trade.instrument_type()]),
    )?;
    let configuration = market
        .pricer_configurations()
        .get(&trade.instrument_type())
        .cloned()
        .ok_or_else(|| {
            QSError::NotFoundErr(format!(
                "Pricer configuration not found for {:?}",
                trade.instrument_type()
            ))
        })?;
    configuration.validate()?;
    Ok(configuration)
}
pub(super) fn prepare_callable_bond(
    trade: &impl BondTerms,
    index: &MarketIndex,
    context: &impl MarketDataProvider,
    configuration: &PricerConfiguration,
    mut time_grid: TimeGridConfiguration,
) -> Result<(MarketData, CallableBondModel<f64>)> {
    let model = configuration.model(index).cloned().ok_or_else(|| {
        QSError::NotFoundErr(format!("Bond pricing model not found for index {index}"))
    })?;
    let mut parameters = HullWhiteParameters::new(model)?;
    let mut request = contractual_market_request(trade, index, context.evaluation_date())
        .with_pricer_configuration_request(vec![trade.instrument_type()]);
    if let Some(calibration) = parameters.calibration_request() {
        let mut elements = request
            .constructed_elements_request()
            .cloned()
            .unwrap_or_default();
        let (index, volatility) = match calibration.source() {
            CalibrationSource::Surface { market_index } => (
                market_index,
                ConstructedElementRequest::VolatilitySurface {
                    market_index: market_index.clone(),
                },
            ),
            CalibrationSource::Cube { market_index } => (
                market_index,
                ConstructedElementRequest::VolatilityCube {
                    market_index: market_index.clone(),
                },
            ),
        };
        elements.push(volatility);
        if !elements.iter().any(|element| matches!(element, ConstructedElementRequest::DiscountCurve { market_index } if market_index == index)) {
            elements.push(ConstructedElementRequest::DiscountCurve { market_index: index.clone() });
        }
        request = request
            .with_constructed_elements_request(elements)
            .with_calibration_request(calibration);
    }
    let market = context.handle_request(&request)?;
    parameters.resolve_calibration(&market, index)?;
    if let Some(day_counter) = parameters.day_counter() {
        time_grid = TimeGridConfiguration::new(
            time_grid.steps_per_year(),
            time_grid.max_steps(),
            day_counter,
        );
    }
    Ok((
        market,
        CallableBondModel {
            parameters,
            time_grid,
        },
    ))
}
