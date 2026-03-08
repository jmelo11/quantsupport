use crate::{
    ad::{
        adreal::{ADReal, IsReal},
        tape::Tape,
    },
    core::{
        collateral::DiscountPolicy,
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::Instrument,
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricer::Pricer,
        pricerstate::PricerState,
        request::{
            HandleCashflows, HandleFairRate, HandleSensitivities, HandleValue, LegsProvider,
            Request,
        },
        trade::Trade,
    },
    instruments::cashflows::{cashflow::Cashflow, cashflowtype::CashflowType, leg::Leg},
    rates::compounding::Compounding,
    time::enums::Frequency,
    utils::errors::{QSError, Result},
};
use std::{collections::HashSet, marker::PhantomData};

/// State for cashflow discounting, holding market data and intermediate values.
struct DCFState<'a> {
    /// The computed DCF value.
    pub value: Option<ADReal>,
    /// Market data response for discount curves.
    pub md_response: Option<MarketData>,
    /// Resolved legs used by default cashflows handling.
    pub legs: &'a [Leg],
}

impl PricerState for DCFState<'_> {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.md_response.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.md_response.as_mut()
    }
}

/// Generic cashflow discounting pricer for any trade with linear cashflows.
/// Works directly with legs and their cashflows, properly handling:
/// - Floating rate coupons (forward rates are set via market data resolution)
/// - Multi-currency trades (uses FX parity from legs at valuation date)
/// - Automatic discount curve requests based on leg currencies/indices
pub struct CashflowDiscountPricer<I, T> {
    _phantom: PhantomData<fn() -> (I, T)>,
    discount_policy: Option<Box<dyn DiscountPolicy<I>>>,
}

impl<I, T> CashflowDiscountPricer<I, T> {
    /// Creates a new [`CashflowDiscountPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
            discount_policy: None,
        }
    }
}

impl LegsProvider for DCFState<'_> {
    fn legs(&self) -> &[Leg] {
        self.legs
    }
}

impl<I, T> HandleCashflows<T, DCFState<'_>> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
}

impl<I, T> Default for CashflowDiscountPricer<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, T> HandleSensitivities<T, DCFState<'_>> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
    fn handle_sensitivities(&self, trade: &T, state: &mut DCFState<'_>) -> Result<SensitivityMap> {
        let price = if let Some(p) = state.value {
            p
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| QSError::NotFoundErr("Missing state.".into()))?
        };

        let () = price.backward_to_mark()?;

        // Collect sensitivities from all unique discount curves used in valuation
        let mut all_ids = Vec::new();
        let mut all_exposures = Vec::new();
        let mut seen_indices = HashSet::new();

        // Forward / leg curves
        for leg in trade.legs() {
            if let Some(market_index) = leg.market_index() {
                if seen_indices.insert(market_index.clone()) {
                    let element = state.get_discount_curve_element(market_index)?;

                    let (ids, exposures): (Vec<_>, Vec<_>) = element
                        .curve()
                        .pillars()
                        .into_iter()
                        .flat_map(std::iter::IntoIterator::into_iter)
                        .map(|(label, value)| (label, value.adjoint().ok()))
                        .unzip();

                    all_ids.extend(ids);
                    let exposures: Vec<f64> = exposures.into_iter().flatten().collect();
                    all_exposures.extend(exposures);
                }
            }
        }

        // CSA collateral OIS curve
        if let Some(policy) = &self.discount_policy {
            let csa_index = policy.accept(trade.instrument())?;
            if seen_indices.insert(csa_index.clone()) {
                let element = state.get_discount_curve_element(&csa_index)?;
                let (ids, exposures): (Vec<_>, Vec<_>) = element
                    .curve()
                    .pillars()
                    .into_iter()
                    .flat_map(std::iter::IntoIterator::into_iter)
                    .map(|(label, value)| (label, value.adjoint().ok()))
                    .unzip();

                all_ids.extend(ids);
                let exposures: Vec<f64> = exposures.into_iter().flatten().collect();
                all_exposures.extend(exposures);
            }
        }

        // FX sensitivities from the exchange-rate store
        if let Some(fx_store) = state.get_exchange_rate_store() {
            for (label, value) in fx_store
                .pillars()
                .ok_or_else(|| QSError::ValueNotSetErr("Pillars".into()))?
            {
                all_ids.push(label);
                all_exposures.push(value.adjoint().unwrap_or(0.0));
            }
        }

        let sensitivities = SensitivityMap::default()
            .with_instrument_keys(&all_ids)
            .with_exposure(&all_exposures);
        Ok(sensitivities)
    }
}

impl<I, T> HandleValue<T, DCFState<'_>> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
    fn handle_value(&self, trade: &T, state: &mut DCFState<'_>) -> Result<f64> {
        // Check that all legs are linear
        for leg in trade.legs() {
            if !leg.is_linear() {
                return Err(QSError::InvalidValueErr(format!(
                    "Leg {} is not linear. CashflowDiscountPricer only supports linear payoffs",
                    leg.leg_id()
                )));
            }
        }

        Tape::start_recording();
        Tape::set_mark();

        let mut pv = ADReal::new(0.0);

        // Put pillars on tape for sensitivity calculation
        state.put_pillars_on_tape()?;

        // Iterate through all legs
        for leg in trade.legs() {
            let leg_currency = leg.currency();
            let leg_has_floating = leg
                .cashflows()
                .iter()
                .any(|cf| matches!(cf, CashflowType::FloatingRateCoupon(_)));

            let leg_discount_index = if let Some(policy) = &self.discount_policy {
                policy.accept(trade.instrument())?
            } else if leg_has_floating {
                let mut matching = state
                    .get_market_data_reponse()
                    .ok_or_else(|| {
                        QSError::NotFoundErr("MarketDataResponse not available.".into())
                    })?
                    .constructed_elements()
                    .discount_curves()
                    .iter()
                    .filter(|(_, element)| element.currency() == leg_currency)
                    .map(|(index, _)| index.clone());

                let first = matching.next().ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "No discount curve found for currency {leg_currency}. For derivative legs without CSA this curve is required as risk-free discounting curve."
                    ))
                })?;

                if matching.next().is_some() {
                    return Err(QSError::InvalidValueErr(format!(
                        "Multiple discount curves found for currency {leg_currency}. Set a leg market index or a discount policy to disambiguate discounting."
                    )));
                }

                first
            } else if let Some(index) = leg.market_index() {
                index.clone()
            } else {
                let mut matching = state
                    .get_market_data_reponse()
                    .ok_or_else(|| {
                        QSError::NotFoundErr("MarketDataResponse not available.".into())
                    })?
                    .constructed_elements()
                    .discount_curves()
                    .iter()
                    .filter(|(_, element)| element.currency() == leg_currency)
                    .map(|(index, _)| index.clone());

                let first = matching.next().ok_or_else(|| {
                    QSError::NotFoundErr(format!(
                        "No discount curve found for currency {leg_currency}. Provide a leg market index or load a risk-free curve for this currency."
                    ))
                })?;

                if matching.next().is_some() {
                    return Err(QSError::InvalidValueErr(format!(
                        "Multiple discount curves found for currency {leg_currency}. Set leg market index to select discount curve explicitly."
                    )));
                }

                first
            };

            let side = leg.side().sign();

            // Iterate through cashflows in this leg
            for cashflow in leg.cashflows() {
                // 1. Extract amount and payment date
                let (amount, payment_date) = match cashflow {
                    CashflowType::FixedRateCoupon(coupon) => {
                        (coupon.amount()?, coupon.payment_date())
                    }
                    CashflowType::FloatingRateCoupon(coupon) => {
                        // Forward / projection curve always comes from the leg's own market index
                        let forward_index = leg.market_index().ok_or(QSError::NotFoundErr(
                            "A market index must be set to price floating rate coupons."
                                .to_string(),
                        ))?;
                        let fwd_curve = state.get_discount_curve_element(forward_index)?.curve();
                        let fwd = fwd_curve.forward_rate(
                            coupon.accrual_start_date(),
                            coupon.accrual_end_date(),
                            Compounding::Simple,
                            Frequency::Annual,
                        )?;
                        coupon.set_fixing(fwd);
                        (coupon.amount()?, coupon.payment_date())
                    }
                    CashflowType::OptionEmbeddedCoupon(_) => {
                        return Err(QSError::InvalidValueErr(format!(
                            "Option-embedded coupon found in leg {}. CashflowDiscountPricer does not support non-linear payoffs",
                            leg.leg_id()
                        )));
                    }
                    CashflowType::Redemption(cf) => (ADReal::from(cf.amount()?), cf.payment_date()),
                    CashflowType::Disbursement(cf) => {
                        (ADReal::from(cf.amount()?), cf.payment_date())
                    }
                };

                let cf_pv: ADReal = if self.discount_policy.is_some() {
                    let df_leg = state
                        .get_discount_curve_element(&leg_discount_index)?
                        .curve()
                        .discount_factor(payment_date)?;

                    let discount_currency = state
                        .get_discount_curve_element(&leg_discount_index)?
                        .currency();
                    if leg_currency == discount_currency {
                        (amount * df_leg).into()
                    } else {
                        let leg_curve_index = leg.market_index().ok_or_else(|| {
                            QSError::NotFoundErr(format!(
                                "Leg {} requires market index to compute FX parity against discount curve currency {}",
                                leg.leg_id(),
                                discount_currency
                            ))
                        })?;
                        let df_leg_ccy = state
                            .get_discount_curve_element(leg_curve_index)?
                            .curve()
                            .discount_factor(payment_date)?;
                        let fx_spot = state.get_exchange_rate(leg_currency, discount_currency)?;
                        // FX_fwd(t) × DF_coll(t) = FX_spot × DF_leg(t)
                        let fx_fwd: ADReal = (fx_spot * df_leg_ccy / df_leg).into();
                        (amount * fx_fwd * df_leg).into()
                    }
                } else {
                    let df = state
                        .get_discount_curve_element(&leg_discount_index)?
                        .curve()
                        .discount_factor(payment_date)?;
                    (amount * df).into()
                };

                pv = (pv + cf_pv * side).into();
            }
        }

        state.value = Some(pv);

        Tape::stop_recording();
        Ok(state.value.unwrap().value())
    }
}

impl<I, T> HandleFairRate<T, DCFState<'_>> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
    fn handle_fair_rate(&self, trade: &T, state: &mut DCFState<'_>) -> Result<f64> {
        let mut annuity = 0.0_f64;
        let mut float_pv = 0.0_f64;

        for leg in trade.legs() {
            // Forward / projection curve always comes from the leg's own market index
            let forward_index = leg.market_index().ok_or(QSError::NotFoundErr(
                "Market index required for par rate computation".to_string(),
            ))?;
            let forward_curve = state.get_discount_curve_element(forward_index)?.curve();

            for cashflow in leg.cashflows() {
                match cashflow {
                    CashflowType::FixedRateCoupon(coupon) => {
                        let year_fraction = coupon
                            .rate()
                            .day_counter()
                            .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date());
                        let payment_date = Cashflow::payment_date(coupon);

                        let df_fx = if let Some(policy) = &self.discount_policy {
                            let collateral_index = policy.accept(trade.instrument())?;
                            let coll_curve =
                                state.get_discount_curve_element(&collateral_index)?.curve();
                            let df_coll = coll_curve.discount_factor(payment_date)?.value();
                            let discount_currency = state
                                .get_discount_curve_element(&collateral_index)?
                                .currency();
                            if leg.currency() == discount_currency {
                                df_coll
                            } else {
                                let fx_spot = state
                                    .get_exchange_rate(leg.currency(), discount_currency)?
                                    .value();
                                let df_leg = forward_curve.discount_factor(payment_date)?.value();
                                // FX_fwd(t) × DF_coll(t) = FX_spot × DF_leg(t)
                                fx_spot * df_leg
                            }
                        } else {
                            
                            forward_curve.discount_factor(payment_date)?.value()
                        };
                        annuity += coupon.notional() * year_fraction * df_fx;
                    }
                    CashflowType::FloatingRateCoupon(coupon) => {
                        let forward = forward_curve
                            .forward_rate(
                                coupon.accrual_start_date(),
                                coupon.accrual_end_date(),
                                Compounding::Simple,
                                Frequency::Annual,
                            )?
                            .value();
                        coupon.set_fixing(forward.into());
                        let amount = coupon.amount()?.value();
                        let payment_date = Cashflow::payment_date(coupon);

                        let df_fx = if let Some(policy) = &self.discount_policy {
                            let collateral_index = policy.accept(trade.instrument())?;
                            let coll_curve =
                                state.get_discount_curve_element(&collateral_index)?.curve();
                            let df_coll = coll_curve.discount_factor(payment_date)?.value();
                            let discount_currency = state
                                .get_discount_curve_element(&collateral_index)?
                                .currency();
                            if leg.currency() == discount_currency {
                                df_coll
                            } else {
                                let fx_spot = state
                                    .get_exchange_rate(leg.currency(), discount_currency)?
                                    .value();
                                let df_leg = forward_curve.discount_factor(payment_date)?.value();
                                fx_spot * df_leg
                            }
                        } else {
                            
                            forward_curve.discount_factor(payment_date)?.value()
                        };
                        float_pv += amount * df_fx;
                    }
                    // Disbursements and redemptions cancel in a vanilla swap
                    // (both legs have the same notional exchange)
                    // this is an incorrect assumption
                    _ => {}
                }
            }
        }

        if annuity.abs() < f64::EPSILON {
            return Err(QSError::InvalidValueErr(
                "Cannot compute par rate: annuity is zero (no fixed coupons found)".into(),
            ));
        }

        Ok(float_pv / annuity)
    }
}

impl<I, T> Pricer for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I> + Send + Sync,
{
    type Item = T;
    type Policy = dyn DiscountPolicy<I>;

    fn evaluate(
        &self,
        trade: &T,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| QSError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DCFState {
            value: None,
            md_response: Some(ctx.handle_request(&md_request)?),
            legs: trade.legs(),
        };

        // Pre-compute value if any request needs it (Value, Cashflows, Sensitivities all need the
        // tape to be set up exactly once).
        let needs_value = requests.iter().any(|r| {
            matches!(
                r,
                Request::Value | Request::Cashflows | Request::Sensitivities
            )
        });
        let wants_price = requests.iter().any(|r| matches!(r, Request::Value));

        if needs_value {
            let price = self.handle_value(trade, &mut state)?;
            if wants_price {
                results = results.with_price(price);
            }
        }

        for request in requests {
            match request {
                Request::Value => {
                    // Already handled above
                }
                Request::Sensitivities => {
                    let sensitivities = self.handle_sensitivities(trade, &mut state)?;
                    results = results.with_sensitivities(sensitivities);
                }
                Request::Cashflows => {
                    let cashflows = <Self as HandleCashflows<T, DCFState<'_>>>::handle_cashflows(
                        self, trade, &mut state,
                    )?;
                    results = results.with_cashflows(cashflows);
                }
                Request::FairRate => {
                    let fair_rate = self.handle_fair_rate(trade, &mut state)?;
                    results = results.with_fair_rate(fair_rate);
                }
                _ => {}
            }
        }

        Ok(results)
    }

    fn market_data_request(&self, trade: &T) -> Option<MarketDataRequest> {
        let legs = trade.legs();
        let mut constructed_elements = Vec::new();
        let mut seen_indices = HashSet::new();
        let mut requires_fx = false;

        // Always request forward / discount curves for each leg
        for leg in legs {
            if let Some(index) = leg.market_index() {
                if seen_indices.insert(index.clone()) {
                    constructed_elements.push(ConstructedElementRequest::DiscountCurve {
                        market_index: index.clone(),
                    });
                }
            }
        }

        // When a CSA discount policy is set, also request:
        //  1. The collateral currency's OIS discount curve
        //  2. Exchange rates (via ExchangeRateStore) for forward FX computation
        if let Some(policy) = &self.discount_policy {
            let csa_index = policy.accept(trade.instrument()).ok()?;
            if seen_indices.insert(csa_index.clone()) {
                constructed_elements.push(ConstructedElementRequest::DiscountCurve {
                    market_index: csa_index,
                });
            }
            requires_fx = true;
        }

        let mut request =
            MarketDataRequest::default().with_constructed_elements_request(constructed_elements);
        if requires_fx {
            request = request.with_exchange_rates();
        }
        Some(request)
    }

    fn set_discount_policy(&mut self, policy: Box<Self::Policy>) {
        self.discount_policy = Some(policy);
    }

    fn discount_policy(&self) -> Option<&Self::Policy> {
        self.discount_policy.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use crate::{
        core::{
            collateral::CSADiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
            },
            pricer::Pricer,
            request::Request,
            trade::Side,
        },
        currencies::{currency::Currency, exchangeratestore::ExchangeRateStore},
        indices::marketindex::MarketIndex,
        instruments::{
            fixedincome::{
                fixedratedeposit::{FixedRateDeposit, FixedRateDepositTrade},
                makefixedratedeposit::MakeFixedRateDeposit,
            },
            rates::crosscurrencyswap::{CrossCurrencySwap, CrossCurrencySwapTrade},
            rates::makecrosscurrencyswap::MakeCrossCurrencySwap,
            rates::makeswap::MakeSwap,
            rates::swap::{Swap, SwapTrade},
        },
        rates::{
            compounding::Compounding, interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{date::Date, daycounter::DayCounter, enums::Frequency},
    };

    #[test]
    fn test_fixed_rate_deposit_pricing_with_discounting_pricer() {
        struct TestMarketDataProvider {
            evaluation_date: Date,
            market_data: MarketData,
        }

        impl MarketDataProvider for TestMarketDataProvider {
            fn handle_request(
                &self,
                _request: &MarketDataRequest,
            ) -> crate::utils::errors::Result<MarketData> {
                Ok(MarketData::new(
                    self.market_data.fixings().clone(),
                    self.market_data.constructed_elements().clone(),
                    &[],
                ))
            }

            fn evaluation_date(&self) -> Date {
                self.evaluation_date
            }
        }

        // --- Parameters ---
        let trade_date = Date::new(2024, 1, 1);
        let maturity_date = Date::new(2024, 7, 1);
        let notional = 100_000.0;
        let deposit_rate = 0.05;
        let discount_rate = 0.03;

        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        );

        // --- 1. Build the deposit trade ---
        let deposit = MakeFixedRateDeposit::default()
            .with_identifier("TEST_DEPOSIT".to_string())
            .with_start_date(trade_date)
            .with_maturity_date(maturity_date)
            .with_notional(notional)
            .with_rate(deposit_rate)
            .with_rate_definition(rate_definition)
            .with_currency(Currency::USD)
            .with_side(Side::PayShort)
            .with_market_index(MarketIndex::SOFR)
            .build()
            .expect("Failed to build deposit");
        let trade = FixedRateDepositTrade::new(deposit, trade_date, notional, Side::PayShort);

        // --- 2. Set up market data: flat 3% discount curve ---
        let discount_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(discount_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("SOFR_flat".to_string());

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(
                MarketIndex::SOFR,
                Currency::USD,
                Rc::new(RefCell::new(discount_curve)),
            ),
        );
        let market_data = MarketData::new(HashMap::new(), constructed_elements, &[]);

        let provider = TestMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        // --- 3. Price using the Pricer trait ---
        let pricer = CashflowDiscountPricer::<FixedRateDeposit, FixedRateDepositTrade>::new();
        let results = pricer
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)
            .expect("Pricing failed");

        // --- 4. Verify PV ---
        // The deposit leg has:
        //   - Disbursement(100k) at start_date (initial funding)
        //   - FixedRateCoupon at maturity (interest accrued)
        //   - Redemption(100k) at maturity (principal repayment)
        // All cashflows are discounted and summed.
        let pv = results.price().expect("Missing price in results");
        assert!(pv > 0.0, "PV should be positive, got {pv}");
        println!("Deposit PV = {pv:.4}");

        // --- 5. Verify sensitivities ---
        let sensitivities = results
            .sensitivities()
            .expect("Missing sensitivities in results");

        let keys = sensitivities.instrument_keys();
        let exposures = sensitivities.exposure();

        // With a flat curve, there should be exactly one pillar
        assert!(!keys.is_empty(), "Sensitivities should have pillar keys");
        assert_eq!(keys.len(), 1, "Expected 1 pillar, got {}", keys.len());
        assert_eq!(keys[0], "SOFR_flat", "Pillar label should be SOFR_flat");

        // Exposure should be negative (higher rate -> lower PV)
        assert!(
            exposures[0] < 0.0,
            "dPV/dr should be negative, got {}",
            exposures[0]
        );
        println!("Sensitivity to {}: {:.4}", keys[0], exposures[0]);
    }

    #[test]
    fn test_vanilla_swap_cashflows() {
        let start_date = Date::new(2024, 1, 1);
        let maturity_date = Date::new(2025, 1, 1);
        let notional = 1_000_000.0;

        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        );

        let swap = MakeSwap::default()
            .with_identifier("VANILLA_SWAP_CF_TEST".to_string())
            .with_start_date(start_date)
            .with_maturity_date(maturity_date)
            .with_fixed_rate(0.03)
            .with_notional(notional)
            .with_rate_definition(rate_definition)
            .with_currency(Currency::USD)
            .with_market_index(MarketIndex::SOFR)
            .with_side(Side::LongRecieve)
            .with_fixed_leg_frequency(Frequency::Semiannual)
            .with_floating_leg_frequency(Frequency::Quarterly)
            .build()
            .expect("Failed to build vanilla swap");

        let trade = SwapTrade::new(swap, start_date, notional, Side::LongRecieve);

        struct VanillaSwapMarketDataProvider {
            evaluation_date: Date,
            market_data: MarketData,
        }

        impl MarketDataProvider for VanillaSwapMarketDataProvider {
            fn handle_request(&self, _request: &MarketDataRequest) -> Result<MarketData> {
                Ok(MarketData::new(
                    self.market_data.fixings().clone(),
                    self.market_data.constructed_elements().clone(),
                    &[],
                ))
            }

            fn evaluation_date(&self) -> Date {
                self.evaluation_date
            }
        }

        let discount_curve = FlatForwardTermStructure::new(
            start_date,
            ADReal::from(0.031),
            RateDefinition::default(),
        )
        .with_pillar_label("SOFR_flat".to_string());

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(
                MarketIndex::SOFR,
                Currency::USD,
                Rc::new(RefCell::new(discount_curve)),
            ),
        );
        let market_data = MarketData::new(HashMap::new(), constructed_elements, &[]);
        let provider = VanillaSwapMarketDataProvider {
            evaluation_date: start_date,
            market_data,
        };

        let pricer = CashflowDiscountPricer::<Swap, SwapTrade>::new();
        let results = pricer
            .evaluate(&trade, &[Request::Cashflows], &provider)
            .expect("Vanilla swap cashflows evaluation failed");

        let cashflows = results
            .cashflows()
            .expect("Missing cashflows in evaluation results");

        let payment_dates = cashflows.payment_dates();
        let cashflow_types = cashflows.cashflow_types();
        let amounts = cashflows.amounts();
        let fixings = cashflows.fixing();
        let accrual_periods = cashflows.accrual_periods();
        let currencies = cashflows.currencies();
        let floorlet_strikes = cashflows.floorlet_strikes();
        let caplet_strikes = cashflows.caplet_strikes();

        println!("Vanilla swap cashflows table (from EvaluationResults.cashflows):");
        println!(
            "| {:<4} | {:<20} | {:<10} | {:<14} | {:<12} | {:<12} | {:<10} | {:<14} |",
            "row", "type", "payment", "amount", "fixing", "accrual", "currency", "floor/cap"
        );
        println!("{}", "-".repeat(118));

        let mut fixed_coupon_count = 0usize;
        let mut floating_coupon_count = 0usize;
        let mut option_embedded_count = 0usize;
        let mut missing_floating_fixing_count = 0usize;

        for i in 0..payment_dates.len() {
            let ctype = &cashflow_types[i];
            if ctype == "FixedRateCoupon" {
                fixed_coupon_count += 1;
            } else if ctype == "FloatingRateCoupon" {
                floating_coupon_count += 1;
                if fixings[i].is_none() {
                    missing_floating_fixing_count += 1;
                }
            } else if ctype == "OptionEmbeddedCoupon" {
                option_embedded_count += 1;
            }

            let fixing_str = fixings[i]
                .map(|v| format!("{v:.8}"))
                .unwrap_or_else(|| "-".to_string());
            let floor_cap = format!(
                "{}/{}",
                floorlet_strikes[i]
                    .map(|v| format!("{v:.6}"))
                    .unwrap_or_else(|| "-".to_string()),
                caplet_strikes[i]
                    .map(|v| format!("{v:.6}"))
                    .unwrap_or_else(|| "-".to_string())
            );

            println!(
                "| {:<4} | {:<20} | {:<10} | {:<14.8} | {:<12} | {:<12.8} | {:<10} | {:<14} |",
                i,
                ctype,
                payment_dates[i],
                amounts[i],
                fixing_str,
                accrual_periods[i],
                currencies[i],
                floor_cap
            );
        }

        assert!(
            fixed_coupon_count > 0,
            "Expected fixed coupons in cashflows table"
        );
        assert!(
            floating_coupon_count > 0,
            "Expected floating coupons in cashflows table"
        );
        assert_eq!(
            missing_floating_fixing_count, 0,
            "Floating coupon fixings should be resolved by pricer before building cashflows table"
        );
        assert_eq!(
            option_embedded_count, 0,
            "Vanilla swap should not contain option-embedded coupons"
        );
    }

    #[test]
    fn test_csa_discounting_with_fx_conversion() {
        // Cross-currency swap (EUR fixed vs USD floating) with EUR CSA collateral.
        //
        // Under CSA:
        //   - EUR leg (collateral currency): discounted with DF_ESTR directly
        //   - USD leg (foreign currency):
        //       FX_fwd(t) = FX_spot × DF_SOFR(t) / DF_ESTR(t)
        //       PV_EUR    = CF_USD(t) × FX_fwd(t) × DF_ESTR(t)
        //                 = CF_USD(t) × FX_spot × DF_SOFR(t)
        //
        // Sensitivities: ESTR (from EUR leg), SOFR (from USD leg), FX spot (from USD leg).

        struct CsaMarketDataProvider {
            evaluation_date: Date,
            market_data: MarketData,
        }

        impl MarketDataProvider for CsaMarketDataProvider {
            fn handle_request(&self, _request: &MarketDataRequest) -> Result<MarketData> {
                let mut md = MarketData::new(
                    self.market_data.fixings().clone(),
                    self.market_data.constructed_elements().clone(),
                    &[],
                );
                if let Some(store) = self.market_data.exchange_rate_store() {
                    md = md.with_exchange_rate_store(store.clone());
                }
                Ok(md)
            }

            fn evaluation_date(&self) -> Date {
                self.evaluation_date
            }
        }

        // --- Parameters ---
        let trade_date = Date::new(2024, 1, 1);
        let maturity_date = Date::new(2025, 1, 1); // 1Y swap
        let eur_notional = 100_000.0;
        let usd_notional = 108_700.0; // ~1/0.92
        let fixed_rate = 0.025; // 2.5% EUR fixed
        let sofr_rate = 0.03; // SOFR (USD leg curve)
        let estr_rate = 0.02; // ESTR (CSA collateral / EUR OIS)
        let fx_usd_eur = 0.92; // 1 USD = 0.92 EUR

        let estr_index = MarketIndex::Other("ESTR".to_string());

        let rate_definition = RateDefinition::new(
            DayCounter::Actual360,
            Compounding::Simple,
            Frequency::Annual,
        );

        // --- 1. Build the cross-currency swap ---
        // Domestic = EUR (fixed, receive), Foreign = USD (floating SOFR, pay)
        let xccy_swap = MakeCrossCurrencySwap::default()
            .with_identifier("XCCY_EUR_USD".to_string())
            .with_start_date(trade_date)
            .with_maturity_date(maturity_date)
            .with_domestic_notional(eur_notional)
            .with_foreign_notional(usd_notional)
            .with_fixed_rate(fixed_rate)
            .with_spread(0.01) // 100 bps spread breaks par on USD floating leg
            .with_rate_definition(rate_definition)
            .with_domestic_currency(Currency::EUR)
            .with_foreign_currency(Currency::USD)
            .with_domestic_market_index(estr_index.clone())
            .with_foreign_market_index(MarketIndex::SOFR)
            .with_side(Side::LongRecieve) // receive EUR fixed, pay USD floating
            .with_domestic_leg_frequency(Frequency::Semiannual)
            .with_foreign_leg_frequency(Frequency::Quarterly)
            .build()
            .expect("Failed to build cross-currency swap");

        let trade = CrossCurrencySwapTrade::new(
            xccy_swap,
            trade_date,
            eur_notional,
            usd_notional,
            Side::LongRecieve,
        );

        // --- 2. Set up market data ---
        // SOFR curve (USD floating leg projection + forward FX)
        let sofr_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(sofr_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("SOFR_flat".to_string());

        // ESTR curve (EUR leg discount + CSA collateral OIS)
        let estr_curve = FlatForwardTermStructure::new(
            trade_date,
            ADReal::from(estr_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("ESTR_flat".to_string());

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            MarketIndex::SOFR,
            DiscountCurveElement::new(
                MarketIndex::SOFR,
                Currency::USD,
                Rc::new(RefCell::new(sofr_curve)),
            ),
        );
        constructed_elements.discount_curves_mut().insert(
            estr_index.clone(),
            DiscountCurveElement::new(
                estr_index.clone(),
                Currency::EUR,
                Rc::new(RefCell::new(estr_curve)),
            ),
        );

        // FX spot rate
        let mut fx_store = ExchangeRateStore::new();
        fx_store.add_exchange_rate(Currency::USD, Currency::EUR, ADReal::from(fx_usd_eur));

        let market_data = MarketData::new(HashMap::new(), constructed_elements, &[])
            .with_exchange_rate_store(fx_store);

        let provider = CsaMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        // --- 3. Price with CSA ---
        let mut pricer = CashflowDiscountPricer::<CrossCurrencySwap, CrossCurrencySwapTrade>::new();
        pricer.set_discount_policy(Box::new(CSADiscountPolicy::new(estr_index.clone())));
        let results = pricer
            .evaluate(&trade, &[Request::Value, Request::Sensitivities], &provider)
            .expect("Pricing with CSA failed");
        let pv = results.price().expect("Missing CSA price");
        println!("PV with CSA = {pv:.4}");

        // --- 4. Verify sensitivities ---
        let sensitivities = results
            .sensitivities()
            .expect("Missing sensitivities in CSA results");
        let keys = sensitivities.instrument_keys();
        let exposures = sensitivities.exposure();
        println!("Sensitivities: {:?}", sensitivities);

        // ESTR: non-zero (EUR leg is discounted directly with ESTR)
        assert!(
            keys.iter().any(|k| k == "ESTR_flat"),
            "Should have ESTR sensitivity, got {keys:?}"
        );
        let estr_idx = keys.iter().position(|k| k == "ESTR_flat").unwrap();
        assert!(
            exposures[estr_idx].abs() > 1e-6,
            "ESTR sensitivity must be non-zero (EUR leg), got {}",
            exposures[estr_idx]
        );

        // SOFR: non-zero (USD leg forward projection + forward FX)
        assert!(
            keys.iter().any(|k| k == "SOFR_flat"),
            "Should have SOFR sensitivity, got {keys:?}"
        );
        let sofr_idx = keys.iter().position(|k| k == "SOFR_flat").unwrap();
        assert!(
            exposures[sofr_idx].abs() > 1e-6,
            "SOFR sensitivity must be non-zero (USD leg), got {}",
            exposures[sofr_idx]
        );

        // FX spot: non-zero (USD leg FX conversion)
        assert!(
            keys.iter().any(|k| k == "USD/EUR"),
            "Should have FX USD/EUR sensitivity, got {keys:?}"
        );
        let fx_idx = keys.iter().position(|k| k == "USD/EUR").unwrap();
        assert!(
            exposures[fx_idx].abs() > 1e-6,
            "FX sensitivity must be non-zero (USD leg), got {}",
            exposures[fx_idx]
        );
    }
}
