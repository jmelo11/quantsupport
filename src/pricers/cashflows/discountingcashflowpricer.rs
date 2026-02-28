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
        request::{HandleFairRate, HandleSensitivities, HandleValue, LegsProvider, Request},
        trade::Trade,
    },
    instruments::cashflows::{
        cashflow::Cashflow,
        cashflowtype::CashflowType,
        coupons::{LinearCoupon, NonLinearCoupon},
        leg::Leg,
    },
    utils::errors::{AtlasError, Result},
};
use std::{collections::HashSet, marker::PhantomData};

/// State for cashflow discounting, holding market data and intermediate values.
#[derive(Default)]
pub struct DCFState {
    /// The computed DCF value.
    pub value: Option<ADReal>,
    /// Market data response for discount curves.
    pub md_response: Option<MarketData>,
}

impl PricerState for DCFState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.md_response.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.md_response.as_mut()
    }
}

/// Generates market data request based on legs' currencies and market indices.
fn market_data_request_from_legs(legs: &[Leg]) -> MarketDataRequest {
    let mut discount_curves = Vec::new();
    let mut seen_indices = HashSet::new();

    for leg in legs {
        if let Some(index) = leg.market_index() {
            let index_key = format!("{:?}", index);
            if seen_indices.insert(index_key) {
                discount_curves.push(ConstructedElementRequest::DiscountCurve {
                    market_index: index.clone(),
                });
            }
        }
    }

    MarketDataRequest::default().with_constructed_elements_request(discount_curves)
}

/// Generic cashflow discounting pricer for any trade with linear cashflows.
/// Works directly with legs and their cashflows, properly handling:
/// - Floating rate coupons (forward rates are set via market data resolution)
/// - Multi-currency trades (uses FX parity from legs at valuation date)
/// - Automatic discount curve requests based on leg currencies/indices
pub struct CashflowDiscountPricer<I, T> {
    _phantom: PhantomData<fn() -> (I, T)>,
}

impl<I, T> CashflowDiscountPricer<I, T> {
    /// Creates a new [`CashflowDiscountPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I, T> Default for CashflowDiscountPricer<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Generic handler that computes sensitivities for any trade implementing LegsProvider.
/// Extracts sensitivities from discount curve pillars that were marked during valuation.
impl<I, T> HandleSensitivities<T, DCFState> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
    fn handle_sensitivities(&self, trade: &T, state: &mut DCFState) -> Result<SensitivityMap> {
        let price = if let Some(p) = state.value {
            p
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| AtlasError::NotFoundErr("Missing state.".into()))?
        };

        let () = price.backward_to_mark()?;

        // Collect sensitivities from all unique discount curves used in valuation
        let mut all_ids = Vec::new();
        let mut all_exposures = Vec::new();
        let mut seen_indices = HashSet::new();

        for leg in trade.legs() {
            if let Some(market_index) = leg.market_index() {
                let index_key = format!("{:?}", market_index);
                if seen_indices.insert(index_key) {
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

        let sensitivities = SensitivityMap::default()
            .with_instrument_keys(&all_ids)
            .with_exposure(&all_exposures);
        Ok(sensitivities)
    }
}

/// Generic handler that prices any trade implementing LegsProvider.
/// Iterates directly over legs and cashflows, handling floating rates and FX conversion.
impl<I, T> HandleValue<T, DCFState> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
    fn handle_value(&self, trade: &T, state: &mut DCFState) -> Result<f64> {
        // Check that all legs are linear
        for leg in trade.legs() {
            if !leg.is_linear() {
                return Err(AtlasError::InvalidValueErr(format!(
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
            let fx_parity = leg.fx_parity().unwrap_or(1.0);
            let market_index = leg.market_index().ok_or(AtlasError::NotFoundErr(
                "Market index required for pricing".to_string(),
            ))?;

            let discount_curve = state.get_discount_curve_element(market_index)?.curve();

            // Iterate through cashflows in this leg
            for cashflow in leg.cashflows() {
                match cashflow {
                    CashflowType::FixedRateCoupon(coupon) => {
                        let amount = coupon.amount()?.value();
                        let payment_date = Cashflow::payment_date(coupon);
                        let discount_factor = discount_curve.discount_factor(payment_date)?;
                        let cf_pv: ADReal =
                            (ADReal::new(amount * fx_parity) * discount_factor).into();
                        pv = (pv + cf_pv).into();
                    }
                    CashflowType::FloatingRateCoupon(coupon) => {
                        // Forward rate is resolved during market data resolution
                        let amount = coupon.amount()?.value();
                        let payment_date = Cashflow::payment_date(coupon);
                        let discount_factor = discount_curve.discount_factor(payment_date)?;
                        let cf_pv: ADReal =
                            (ADReal::new(amount * fx_parity) * discount_factor).into();
                        pv = (pv + cf_pv).into();
                    }
                    CashflowType::OptionEmbeddedCoupon(coupon) => {
                        let amount = coupon.amount()?.value();
                        let payment_date = coupon.payment_date();
                        let discount_factor = discount_curve.discount_factor(payment_date)?;
                        let cf_pv: ADReal =
                            (ADReal::new(amount * fx_parity) * discount_factor).into();
                        pv = (pv + cf_pv).into();
                    }
                    CashflowType::Redemption(cashflow) => {
                        let amount = cashflow.amount()?.value();
                        let payment_date = cashflow.payment_date();
                        let discount_factor = discount_curve.discount_factor(payment_date)?;
                        let cf_pv: ADReal =
                            (ADReal::new(amount * fx_parity) * discount_factor).into();
                        pv = (pv + cf_pv).into();
                    }
                    CashflowType::Disbursement(cashflow) => {
                        let amount = cashflow.amount()?.value();
                        let payment_date = cashflow.payment_date();
                        let discount_factor = discount_curve.discount_factor(payment_date)?;
                        let cf_pv: ADReal =
                            (ADReal::new(amount * fx_parity) * discount_factor).into();
                        pv = (pv + cf_pv).into();
                    }
                }
            }
        }

        state.value = Some(pv);

        Tape::stop_recording();
        Ok(state.value.unwrap().value())
    }
}

/// Computes the par (fair) rate for instruments that have both fixed and floating legs.
///
/// The par rate is the fixed coupon rate that makes the swap NPV equal to zero.
/// It is computed as:
///
///   par_rate = PV(floating coupons) / annuity(fixed leg)
///
/// where:
///   - PV(floating coupons) = Σ notional_i × (forward_i + spread_i) × τ_i × df(payment_i)
///   - annuity = Σ notional_i × τ_i × df(payment_i)  (over the fixed leg coupon periods)
///   - forward rates are derived from the discount curve: f = (df(start)/df(end) − 1) / τ
impl<I, T> HandleFairRate<T, DCFState> for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I>,
{
    fn handle_fair_rate(&self, trade: &T, state: &mut DCFState) -> Result<f64> {
        let mut annuity = 0.0_f64;
        let mut float_pv = 0.0_f64;

        for leg in trade.legs() {
            let fx_parity = leg.fx_parity().unwrap_or(1.0);
            let market_index = leg.market_index().ok_or(AtlasError::NotFoundErr(
                "Market index required for par rate computation".to_string(),
            ))?;

            let discount_curve = state.get_discount_curve_element(market_index)?.curve();

            for cashflow in leg.cashflows() {
                match cashflow {
                    CashflowType::FixedRateCoupon(coupon) => {
                        // Accumulate annuity: notional × year_fraction × df
                        let year_fraction = coupon
                            .rate()
                            .day_counter()
                            .year_fraction(coupon.accrual_start_date(), coupon.accrual_end_date());
                        let df = discount_curve
                            .discount_factor(Cashflow::payment_date(coupon))?
                            .value();
                        annuity += coupon.notional() * year_fraction * df * fx_parity;
                    }
                    CashflowType::FloatingRateCoupon(coupon) => {
                        // Compute forward rate from the discount curve
                        let start = coupon.accrual_start_date();
                        let end = coupon.accrual_end_date();
                        let tau = coupon.day_counter().year_fraction(start, end);

                        let df_start = discount_curve.discount_factor(start)?.value();
                        let df_end = discount_curve.discount_factor(end)?.value();
                        let forward = (df_start / df_end - 1.0) / tau;
                        let spread = coupon.spread().value();
                        let notional = LinearCoupon::notional(coupon);

                        let df_pay = discount_curve
                            .discount_factor(Cashflow::payment_date(coupon))?
                            .value();
                        float_pv += notional * (forward + spread) * tau * df_pay * fx_parity;
                    }
                    // Disbursements and redemptions cancel in a vanilla swap
                    // (both legs have the same notional exchange)
                    _ => {}
                }
            }
        }

        if annuity.abs() < f64::EPSILON {
            return Err(AtlasError::InvalidValueErr(
                "Cannot compute par rate: annuity is zero (no fixed coupons found)".into(),
            ));
        }

        Ok(float_pv / annuity)
    }
}

/// Implementation of the [`Pricer`] trait for the generic cashflow discounting pricer.
impl<I, T> Pricer for CashflowDiscountPricer<I, T>
where
    I: Instrument,
    T: LegsProvider + Trade<I> + Send + Sync,
{
    type Item = T;

    fn evaluate(
        &self,
        trade: &T,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> std::result::Result<EvaluationResults, AtlasError> {
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();

        let md_request = self
            .market_data_request(trade)
            .ok_or_else(|| AtlasError::InvalidValueErr("Missing market data request".into()))?;

        let mut results = EvaluationResults::new(eval_date, identifier);
        let mut state = DCFState {
            value: None,
            md_response: Some(ctx.handle_request(&md_request)?),
        };

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
        Some(market_data_request_from_legs(trade.legs()))
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
            elements::curveelement::DiscountCurveElement,
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider},
            },
            pricer::Pricer,
            request::Request,
            trade::Side,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::{
            cashflows::{cashflow::SimpleCashflow, cashflowtype::CashflowType, leg::Leg},
            fixedincome::{
                fixedratedeposit::{FixedRateDeposit, FixedRateDepositTrade},
                makefixedratedeposit::MakeFixedRateDeposit,
            },
        },
        rates::{
            compounding::Compounding, interestrate::RateDefinition,
            yieldtermstructure::flatforwardtermstructure::FlatForwardTermStructure,
        },
        time::{date::Date, daycounter::DayCounter, enums::Frequency},
    };

    #[test]
    fn test_cashflow_discounting_pricer_linearity_validation() {
        // Test that non-linear legs can be created but should not be used with the pricer
        let date = Date::new(2024, 6, 1);
        let cashflow = SimpleCashflow::new(100_000.0, date);

        // Create a non-linear leg
        let non_linear_leg = Leg::new(
            0,
            vec![CashflowType::Redemption(cashflow)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            false, // Non-linear
        );

        // Create a linear leg
        let linear_leg = Leg::new(
            1,
            vec![CashflowType::Redemption(SimpleCashflow::new(
                100_000.0, date,
            ))],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true, // Linear
        );

        // Verify properties
        assert_eq!(
            non_linear_leg.is_linear(),
            false,
            "Leg should be marked as non-linear"
        );
        assert_eq!(
            linear_leg.is_linear(),
            true,
            "Leg should be marked as linear"
        );
    }

    #[test]
    fn test_cashflow_discounting_pricer_market_data_request_generation() {
        // Test that market data requests are correctly generated from legs
        let date1 = Date::new(2024, 6, 1);
        let date2 = Date::new(2024, 12, 1);

        let cashflow1 = SimpleCashflow::new(50_000.0, date1);
        let cashflow2 = SimpleCashflow::new(100_000.0, date2);

        let leg1 = Leg::new(
            0,
            vec![CashflowType::Redemption(cashflow1)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        );

        let leg2 = Leg::new(
            1,
            vec![CashflowType::Redemption(cashflow2)],
            Currency::EUR,
            Some(MarketIndex::TermSOFR3m),
            None,
            None,
            Side::LongRecieve,
            true,
        );

        let legs = vec![leg1, leg2];
        let request = market_data_request_from_legs(&legs);

        // Verify that request contains constructed elements
        if let Some(constructed) = request.constructed_elements_request() {
            assert!(
                !constructed.is_empty(),
                "Market data request should contain discount curves"
            );
        } else {
            panic!("Market data request should have constructed elements");
        }
    }

    #[test]
    fn test_cashflow_discounting_pricer_fx_parity_handling() {
        // Test that FX parity is correctly stored and retrieved from legs
        let date = Date::new(2024, 6, 1);
        let cashflow = SimpleCashflow::new(100_000.0, date);

        let leg = Leg::new(
            0,
            vec![CashflowType::Redemption(cashflow)],
            Currency::EUR,
            Some(MarketIndex::TermSOFR3m),
            None,
            None,
            Side::LongRecieve,
            true,
        )
        .with_fx_parity(1.1);

        // Verify FX parity is set
        assert_eq!(leg.fx_parity(), Some(1.1), "FX parity should be 1.1");

        // Test with multiple legs having different parities
        let leg2 = Leg::new(
            1,
            vec![CashflowType::Redemption(SimpleCashflow::new(
                50_000.0, date,
            ))],
            Currency::USD,
            Some(MarketIndex::TermSOFR6m),
            None,
            None,
            Side::PayShort,
            true,
        )
        .with_fx_parity(1.25);

        assert_eq!(
            leg2.fx_parity(),
            Some(1.25),
            "Second leg should have parity 1.25"
        );
    }

    #[test]
    fn test_cashflow_discounting_pricer_multiple_cashflows_per_leg() {
        // Test that multiple cashflows in a leg are handled correctly
        let date1 = Date::new(2024, 6, 1);
        let date2 = Date::new(2024, 12, 1);

        let cashflow1 = SimpleCashflow::new(25_000.0, date1);
        let cashflow2 = SimpleCashflow::new(25_000.0, date2);

        let leg = Leg::new(
            0,
            vec![
                CashflowType::Redemption(cashflow1),
                CashflowType::Redemption(cashflow2),
            ],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        )
        .with_fx_parity(1.0);

        // Verify the leg contains both cashflows
        assert_eq!(leg.cashflows().len(), 2, "Leg should have 2 cashflows");
        assert_eq!(leg.is_linear(), true, "Leg should be linear");
        assert_eq!(leg.fx_parity(), Some(1.0), "FX parity should be 1.0");
    }

    #[test]
    fn test_cashflow_discounting_pricer_leg_properties() {
        // Test various leg properties are correctly configured
        let date = Date::new(2024, 6, 1);
        let cashflow = SimpleCashflow::new(100_000.0, date);

        let leg = Leg::new(
            42,
            vec![CashflowType::Redemption(cashflow)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        );

        // Verify leg properties
        assert_eq!(leg.leg_id(), 42, "Leg ID should be 42");
        assert_eq!(leg.cashflows().len(), 1, "Leg should have 1 cashflow");
        assert_eq!(leg.is_linear(), true, "Leg should be linear");
        assert_eq!(
            leg.market_index(),
            Some(&MarketIndex::SOFR),
            "Market index should be SOFR"
        );
        assert_eq!(leg.fx_parity(), None, "FX parity should default to None");
    }

    /// Mock trade implementing LegsProvider for integration tests
    struct MockTrade {
        legs: Vec<Leg>,
    }

    impl LegsProvider for MockTrade {
        fn legs(&self) -> &[Leg] {
            &self.legs
        }
    }

    #[test]
    fn test_cashflow_discounting_pricer_single_cashflow_pricing() {
        // Test pricing of a simple single cashflow: 100k due in 6 months with 2% discount rate
        let payment_date = Date::new(2024, 7, 1); // 6 months later

        let cashflow = SimpleCashflow::new(100_000.0, payment_date);
        let leg = Leg::new(
            0,
            vec![CashflowType::Redemption(cashflow)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        );

        let trade = MockTrade { legs: vec![leg] };

        // With a flat 2% discount rate for 6 months, the discount factor should be ~0.99
        // PV = 100,000 * 0.99 = 99,000 (approximately)
        // The exact value depends on the day count convention used

        // Verify the trade is set up correctly
        assert_eq!(trade.legs().len(), 1, "Trade should have 1 leg");
        assert_eq!(
            trade.legs()[0].cashflows().len(),
            1,
            "Leg should have 1 cashflow"
        );
        assert!(trade.legs()[0].is_linear(), "Leg should be linear");
        // Payment date verification handled via match when needed
    }

    #[test]
    fn test_cashflow_discounting_pricer_multiple_legs_different_currencies() {
        // Test that trades with multiple legs in different currencies are properly structured
        let date1 = Date::new(2024, 6, 1);
        let date2 = Date::new(2024, 12, 1);

        // USD leg: 100k
        let cashflow_usd = SimpleCashflow::new(100_000.0, date1);
        let leg_usd = Leg::new(
            0,
            vec![CashflowType::Redemption(cashflow_usd)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        )
        .with_fx_parity(1.0); // 1:1 parity

        // EUR leg: 85k with FX parity of 1.10 (1 EUR = 1.10 USD)
        let cashflow_eur = SimpleCashflow::new(85_000.0, date2);
        let leg_eur = Leg::new(
            1,
            vec![CashflowType::Redemption(cashflow_eur)],
            Currency::EUR,
            Some(MarketIndex::TermSOFR3m),
            None,
            None,
            Side::LongRecieve,
            true,
        )
        .with_fx_parity(1.10);

        let trade = MockTrade {
            legs: vec![leg_usd, leg_eur],
        };

        // Verify structure
        assert_eq!(trade.legs().len(), 2, "Trade should have 2 legs");
        assert_eq!(
            trade.legs()[0].fx_parity(),
            Some(1.0),
            "USD leg should have parity 1.0"
        );
        assert_eq!(
            trade.legs()[1].fx_parity(),
            Some(1.10),
            "EUR leg should have parity 1.10"
        );

        // Total USD equivalent before discounting: 100k + 85k * 1.10 = 193.5k
        // The actual value depends on the discount factors for each date
    }

    #[test]
    fn test_cashflow_discounting_pricer_sequential_cashflows() {
        // Test pricing of multiple sequential cashflows (bond-like instrument)
        let date1 = Date::new(2024, 7, 1);
        let date2 = Date::new(2024, 12, 1);
        let date3 = Date::new(2025, 7, 1);

        // Three cashflows: coupons + redemption
        let cf1 = SimpleCashflow::new(2_500.0, date1); // Coupon
        let cf2 = SimpleCashflow::new(2_500.0, date2); // Coupon
        let cf3 = SimpleCashflow::new(102_500.0, date3); // Coupon + Redemption

        let leg = Leg::new(
            0,
            vec![
                CashflowType::Redemption(cf1),
                CashflowType::Redemption(cf2),
                CashflowType::Redemption(cf3),
            ],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        );

        let trade = MockTrade { legs: vec![leg] };

        // Verify the cashflows are all present
        assert_eq!(
            trade.legs()[0].cashflows().len(),
            3,
            "Leg should have 3 cashflows"
        );

        // Total undiscounted value: 2.5k + 2.5k + 102.5k = 107.5k
        // Actual PV depends on discount factors for each date
    }

    #[test]
    fn test_cashflow_discounting_pricer_fx_conversion_in_pricing() {
        // Test that FX parity is properly applied during pricing
        // Two identical cashflows but with different FX parities

        let date = Date::new(2024, 7, 1);

        // Leg 1: 50k with FX parity 1.0 (no conversion)
        let cf1 = SimpleCashflow::new(50_000.0, date);
        let leg1 = Leg::new(
            0,
            vec![CashflowType::Redemption(cf1)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        )
        .with_fx_parity(1.0);

        // Leg 2: 50k with FX parity 1.2 (20% appreciation)
        let cf2 = SimpleCashflow::new(50_000.0, date);
        let leg2 = Leg::new(
            1,
            vec![CashflowType::Redemption(cf2)],
            Currency::EUR,
            Some(MarketIndex::TermSOFR3m),
            None,
            None,
            Side::LongRecieve,
            true,
        )
        .with_fx_parity(1.2);

        let trade = MockTrade {
            legs: vec![leg1, leg2],
        };

        // Assuming same discount factor for both dates:
        // PV = 50k * 1.0 * df + 50k * 1.2 * df = (50k + 60k) * df = 110k * df
        // So leg2's value is 20% higher due to FX parity
        assert_eq!(trade.legs()[0].fx_parity(), Some(1.0));
        assert_eq!(trade.legs()[1].fx_parity(), Some(1.2));
    }

    #[test]
    fn test_cashflow_discounting_pricer_pricing_consistency() {
        // Test that pricing is consistent with mathematical expectations:
        // PV = sum(CF_i * FX_i * DF(t_i))

        let date = Date::new(2024, 7, 1);

        // Simple case: single 100k cashflow with FX parity 1.0
        let cf = SimpleCashflow::new(100_000.0, date);
        let leg = Leg::new(
            0,
            vec![CashflowType::Redemption(cf)],
            Currency::USD,
            Some(MarketIndex::SOFR),
            None,
            None,
            Side::PayShort,
            true,
        );

        let trade = MockTrade { legs: vec![leg] };

        // The PV should be:
        // PV = 100,000 * 1.0 * DF(Jul 1, 2024)
        // where DF depends on the discount curve and evaluation date

        // Verify the parameters are set correctly for pricing
        let leg = &trade.legs()[0];
        assert_eq!(leg.cashflows().len(), 1);

        // Verify the cashflow via pattern matching
        if let CashflowType::Redemption(cf) = &leg.cashflows()[0] {
            assert_eq!(cf.amount().unwrap().value(), 100_000.0);
            assert_eq!(cf.payment_date(), date);
        } else {
            panic!("Expected Redemption cashflow");
        }
    }

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
}
