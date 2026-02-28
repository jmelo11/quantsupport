use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    instruments::{
        cashflows::{cashflow::Cashflow, cashflowtype::CashflowType},
        fixedincome::fixedratedeposit::FixedRateDepositTrade,
    },
    utils::errors::{AtlasError, Result},
};

/// Pricer for deposits that uses discounted cash flow methodology. It calculates the
/// present value of the deposit's final payment by discounting it using the appropriate
/// discount factor from the relevant discount curve. The pricer also computes
/// sensitivities to the discount curve pillars, which can be used for risk
/// management and hedging purposes.
pub struct FixedRateDepositDiscountingPricer;

/// Holds state information for deposit price evaluation.
#[derive(Default)]
struct DepositPricerState {
    /// Price placeholder for perfomance reasons.
    pub value: Option<ADReal>,
    /// Market data response placeholder to avoid multiple calls to the market data provider.
    pub md_response: Option<MarketData>,
}

impl PricerState for DepositPricerState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.md_response.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.md_response.as_mut()
    }
}

impl HandleValue<FixedRateDepositTrade, DepositPricerState> for FixedRateDepositDiscountingPricer {
    fn handle_value(
        &self,
        trade: &FixedRateDepositTrade,
        state: &mut DepositPricerState,
    ) -> Result<f64> {
        Tape::start_recording();
        Tape::set_mark();

        let index = trade.instrument().market_index();
        let leg = trade.instrument().leg();

        // Extract cashflows from the leg
        let mut coupon_amount = 0.0;
        let mut coupon_date = None;
        let mut redemption_amount = 0.0;
        let mut redemption_date = None;

        for cashflow in leg.cashflows() {
            match cashflow {
                CashflowType::FixedRateCoupon(cf) => {
                    coupon_amount = cf.amount()?.value();
                    coupon_date = Some(cf.payment_date());
                }
                CashflowType::Redemption(cf) => {
                    redemption_amount = cf.amount()?;
                    redemption_date = Some(cf.payment_date());
                }
                _ => {}
            }
        }

        let coupon_date = coupon_date.ok_or_else(|| {
            AtlasError::NotFoundErr("Coupon date not found in leg cashflows".into())
        })?;
        let redemption_date = redemption_date.ok_or_else(|| {
            AtlasError::NotFoundErr("Redemption date not found in leg cashflows".into())
        })?;

        // get the element and put the pillars on tape for sensitivity calculation
        state.put_pillars_on_tape()?;

        // actually computing the price
        let df1 = state
            .get_discount_curve_element(&index)?
            .curve()
            .discount_factor(coupon_date)?;

        let df2 = if coupon_date != redemption_date {
            state
                .get_discount_curve_element(&index)?
                .curve()
                .discount_factor(redemption_date)?
        } else {
            df1
        };

        let value = (df1 * coupon_amount + df2 * redemption_amount).into();
        state.value = Some(value);

        Tape::stop_recording();
        Ok(value.value())
    }
}

impl HandleSensitivities<FixedRateDepositTrade, DepositPricerState>
    for FixedRateDepositDiscountingPricer
{
    fn handle_sensitivities(
        &self,
        trade: &FixedRateDepositTrade,
        state: &mut DepositPricerState,
    ) -> Result<SensitivityMap> {
        let price = if let Some(p) = state.value {
            p
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| AtlasError::NotFoundErr("Missing state.".into()))?
        };

        let () = price.backward_to_mark()?;
        let index = trade.instrument().market_index();
        let element = state.get_discount_curve_element(&index)?;

        let (ids, exposures): (Vec<_>, Vec<_>) = element
            .curve()
            .pillars()
            .into_iter()
            .flat_map(std::iter::IntoIterator::into_iter)
            .map(|(label, value)| (label, value.adjoint().ok()))
            .unzip();

        let exposures: Vec<f64> = exposures.into_iter().flatten().collect();

        let sensitivities = SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures);
        Ok(sensitivities)
    }
}

impl Pricer for FixedRateDepositDiscountingPricer {
    type Item = FixedRateDepositTrade;

    fn evaluate(
        &self,
        trade: &FixedRateDepositTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let depo = trade.instrument();
        let identifier = depo.identifier();

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DepositPricerState::default();
        let md_request = self.market_data_request(trade).ok_or_else(|| {
            AtlasError::InvalidValueErr("A market data request should have been returned!".into())
        })?;

        state.md_response = Some(ctx.handle_request(&md_request)?);

        for request in requests {
            match request {
                Request::Value => {
                    let price = self.handle_value(trade, &mut state)?;
                    results = results.with_price(price);
                }

                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                _ => {}
            }
        }

        Ok(results)
    }

    fn market_data_request(&self, trade: &FixedRateDepositTrade) -> Option<MarketDataRequest> {
        let discount_curve = ConstructedElementRequest::DiscountCurve {
            market_index: trade.instrument().market_index(),
        };
        Some(MarketDataRequest::default().with_constructed_elements_request(vec![discount_curve]))
    }
}
