use std::collections::HashSet;

use crate::{
    ad::{dual::DualFwd, tape::Tape},
    core::{
        collateral::{DiscountPolicy, Discountable},
        evaluationresults::{EvaluationResults, SensitivityMap},
        instrument::{AssetClass, Instrument},
        marketdatahandling::{
            constructedelementrequest::ConstructedElementRequest,
            fxrequest::FxRequest,
            marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
        },
        pillars::Pillars,
        pricer::Pricer,
        pricerstate::PricerState,
        request::{HandleFairRate, HandleSensitivities, HandleValue, Request},
        trade::Trade,
    },
    currencies::currency::Currency,
    instruments::fx::fxforward::FxForwardTrade,
    utils::errors::{QSError, Result},
};

/// Pricer for FX forward trades.
///
/// The model forward is computed as
/// `F = S * DF_quote(T) / DF_base(T)`,
/// and the NPV of the trade (from the buyer's side) is
/// `NPV = N * (F - K) * DF_quote(T)`,
/// where `K` is the agreed forward price and the trade side signs the result.
/// This is the implemented DF ratio; the FX option pricer uses the reciprocal
/// ratio. A strike-free calibration instrument returns `N * F * DF_quote`
/// without a side multiplier.
///
/// A [`DiscountPolicy`] is required and resolves curves for both currencies.
/// Spot and strike are quote-currency units per base-currency unit.
/// The formulas above give quote-currency PV. The policy can select a reporting
/// currency for value and sensitivities; the fair strike retains the pair units.
///
/// ## Example
/// ```rust
/// use quantsupport::prelude::*;
///
/// let pricer = FxForwardPricer::new();
///
/// // Build the instrument:
/// let fx_fwd = MakeFxForward::default()
///     .with_identifier("EURUSD-1M".to_string())
///     .with_delivery_date(Date::new(2024, 7, 1))
///     .with_forward_price(1.1025)
///     .with_base_currency(Currency::EUR)
///     .with_quote_currency(Currency::USD)
///     .as_deliverable()
///     .build()
///     .expect("failed to build fx forward");
///
/// let trade = FxForwardTrade::new(
///     fx_fwd, Date::new(2024, 6, 1), 1_000_000.0, Side::LongReceive,
/// );
/// assert_eq!(trade.notional(), 1_000_000.0);
/// ```
pub struct FxForwardPricer {
    discount_policy: Option<Box<dyn DiscountPolicy>>,
}

impl FxForwardPricer {
    /// Creates a new [`FxForwardPricer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            discount_policy: None,
        }
    }
}

impl Default for FxForwardPricer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct FxForwardState {
    value: Option<DualFwd>,
    market_data: Option<MarketData>,
}

impl PricerState for FxForwardState {
    fn get_market_data_reponse(&self) -> Option<&MarketData> {
        self.market_data.as_ref()
    }

    fn get_market_data_reponse_mut(&mut self) -> Option<&mut MarketData> {
        self.market_data.as_mut()
    }
}

/// Lightweight [`Discountable`] wrapper used to resolve a discount curve
/// for a specific currency through the discount policy.
struct CurrencyDiscountable {
    currency: Currency,
}

impl Discountable for CurrencyDiscountable {
    fn asset_class(&self) -> AssetClass {
        AssetClass::Fx
    }

    fn currency(&self) -> Currency {
        self.currency
    }
}

impl HandleValue<FxForwardTrade, FxForwardState> for FxForwardPricer {
    fn handle_value(&self, trade: &FxForwardTrade, state: &mut FxForwardState) -> Result<f64> {
        Tape::start_recording_fwd();
        Tape::set_mark_fwd();
        state.put_pillars_on_tape()?;

        let inst = trade.instrument();
        let base = inst.base_currency();
        let quote = inst.quote_currency();

        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX forward pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable { currency: base })?;
        let quote_idx = policy.accept(&CurrencyDiscountable { currency: quote })?;

        let df_base = state
            .get_discount_curve_element(&base_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;
        let df_quote = state
            .get_discount_curve_element(&quote_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;

        let spot = state.get_exchange_rate(base, quote)?;
        let forward: DualFwd = (spot * df_quote / df_base).into();

        // NPV = notional * (F_model - K) * DF_quote * side
        let notional = DualFwd::new(trade.notional());
        let npv: DualFwd = inst.forward_price().map_or_else(
            || (notional * forward * df_quote).into(),
            |k| {
                let side = DualFwd::new(trade.side().sign());
                (notional * (forward - k) * df_quote * side).into()
            },
        );
        let source = inst.quote_currency();
        let npv = if let Some(target) = state
            .get_market_data_reponse()
            .and_then(MarketData::base_currency)
            .filter(|target| *target != source)
        {
            (npv * state.get_exchange_rate(source, target)?).into()
        } else {
            npv
        };
        state.value = Some(npv);

        Tape::stop_recording_fwd();
        Ok(npv.value())
    }
}

impl HandleFairRate<FxForwardTrade, FxForwardState> for FxForwardPricer {
    /// Computes the fair (par) forward rate, defined as `F = S * DF_quote / DF_base`.
    /// This is the strike at which the trade has zero NPV.
    fn handle_fair_rate(
        &self,
        trade: &FxForwardTrade,
        state: &mut FxForwardState,
        target: usize,
    ) -> Result<f64> {
        if target != 0 {
            return Err(QSError::InvalidValueErr(
                "Fair-rate target must be zero for this product".into(),
            ));
        }
        let inst = trade.instrument();
        let base = inst.base_currency();
        let quote = inst.quote_currency();

        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX forward pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable { currency: base })?;
        let quote_idx = policy.accept(&CurrencyDiscountable { currency: quote })?;

        let df_base = state
            .get_discount_curve_element(&base_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;
        let df_quote = state
            .get_discount_curve_element(&quote_idx)?
            .curve()
            .discount_factor(inst.delivery_date())?;

        let spot = state.get_exchange_rate(base, quote)?;
        let forward: DualFwd = (spot * df_quote / df_base).into();
        Ok(forward.value())
    }
}

impl HandleSensitivities<FxForwardTrade, FxForwardState> for FxForwardPricer {
    fn handle_sensitivities(
        &self,
        trade: &FxForwardTrade,
        state: &mut FxForwardState,
    ) -> Result<SensitivityMap> {
        let value = if let Some(v) = state.value {
            v
        } else {
            let _ = self.handle_value(trade, state)?;
            state
                .value
                .ok_or_else(|| QSError::UnexpectedErr("Missing value in FX forward state".into()))?
        };

        value.backward_to_mark()?;

        let inst = trade.instrument();
        let policy = self.discount_policy.as_ref().ok_or_else(|| {
            QSError::InvalidValueErr("Discount policy required for FX forward pricing".into())
        })?;
        let base_idx = policy.accept(&CurrencyDiscountable {
            currency: inst.base_currency(),
        })?;
        let quote_idx = policy.accept(&CurrencyDiscountable {
            currency: inst.quote_currency(),
        })?;

        let mut ids = Vec::new();
        let mut exposures = Vec::new();

        for idx in [base_idx, quote_idx] {
            let element = state.get_discount_curve_element(&idx)?;
            for (label, value) in element.curve().pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().map_or(0.0, |a| a.value()));
            }
        }

        if let Some(store) = state.get_fx_store() {
            for (label, value) in store.pillars().into_iter().flatten() {
                ids.push(label);
                exposures.push(value.adjoint().map_or(0.0, |a| a.value()));
            }
        }

        Ok(SensitivityMap::default()
            .with_instrument_keys(&ids)
            .with_exposure(&exposures)
            .aggregate())
    }
}

impl Pricer for FxForwardPricer {
    type Item = FxForwardTrade;
    type Policy = dyn DiscountPolicy;

    fn evaluate(
        &self,
        trade: &FxForwardTrade,
        requests: &[Request],
        ctx: &impl MarketDataProvider,
    ) -> Result<EvaluationResults> {
        Request::validate(requests)?;
        let eval_date = ctx.evaluation_date();
        let identifier = trade.instrument().identifier();
        let md_request = self.market_data_request(trade).ok_or_else(|| {
            QSError::InvalidValueErr("Missing market-data request for FX forward".into())
        })?;

        let mut state = FxForwardState {
            value: None,
            market_data: Some(ctx.handle_request(&md_request)?),
        };

        let mut out = EvaluationResults::new(eval_date, identifier);
        for req in requests {
            match req {
                Request::Value => out = out.with_price(self.handle_value(trade, &mut state)?),
                Request::Sensitivities => {
                    out = out.with_sensitivities(self.handle_sensitivities(trade, &mut state)?);
                }
                Request::FairRate(target) => {
                    out = out.with_fair_rate(self.handle_fair_rate(trade, &mut state, *target)?);
                }
                _ => {}
            }
        }

        Ok(out)
    }

    fn market_data_request(&self, trade: &FxForwardTrade) -> Option<MarketDataRequest> {
        let policy = self.discount_policy.as_ref()?;
        let inst = trade.instrument();
        let mut elements = Vec::new();
        let mut seen_indices = HashSet::new();

        for ccy in [inst.base_currency(), inst.quote_currency()] {
            if let Ok(idx) = policy.accept(&CurrencyDiscountable { currency: ccy }) {
                if seen_indices.insert(idx.clone()) {
                    elements.push(ConstructedElementRequest::DiscountCurve { market_index: idx });
                }
            }
        }

        let mut request = MarketDataRequest::default().with_fx_request(vec![FxRequest::pair(
            inst.base_currency(),
            inst.quote_currency(),
        )]);

        if !elements.is_empty() {
            request = request.with_constructed_elements_request(elements);
        }

        let mut fx = request.fx_request().cloned().unwrap_or_default();
        fx.push(FxRequest::single(inst.quote_currency()));
        request = request.with_fx_request(fx);
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
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use super::FxForwardPricer;
    use crate::{
        ad::dual::DualFwd,
        core::{
            collateral::DiscountPolicy,
            elements::curveelement::DiscountCurveElement,
            evaluationresults::EvaluationResults,
            marketdatahandling::{
                constructedelementstore::ConstructedElementStore,
                marketdata::{MarketData, MarketDataProvider, MarketDataRequest},
            },
            pricer::Pricer,
            request::Request,
            trade::Side,
        },
        currencies::currency::Currency,
        indices::marketindex::MarketIndex,
        instruments::fx::{fxforward::FxForwardTrade, makefxforward::MakeFxForward},
        quotes::fxstore::FxStore,
        rates::{
            interestrate::RateDefinition,
            yieldtermstructure::{
                flatforwardtermstructure::FlatForwardTermStructure,
                interestratestermstructure::InterestRatesTermStructure,
            },
        },
        time::{date::Date, enums::TimeUnit, period::Period},
        utils::errors::{QSError, Result},
    };

    const BASE_CCY: Currency = Currency::EUR;
    const QUOTE_CCY: Currency = Currency::USD;

    /// Maps each currency of the pair to its discount index.
    struct FxDiscountPolicy {
        base_index: MarketIndex,
        quote_index: MarketIndex,
    }

    impl DiscountPolicy for FxDiscountPolicy {
        fn accept(
            &self,
            target: &dyn crate::core::collateral::Discountable,
        ) -> Result<MarketIndex> {
            if target.currency() == BASE_CCY {
                Ok(self.base_index.clone())
            } else if target.currency() == QUOTE_CCY {
                Ok(self.quote_index.clone())
            } else {
                Err(QSError::InvalidValueErr(format!(
                    "Unsupported currency: {}",
                    target.currency()
                )))
            }
        }

        fn discount_indices(&self) -> Vec<MarketIndex> {
            vec![self.base_index.clone(), self.quote_index.clone()]
        }
    }

    struct SimpleMarketDataProvider {
        evaluation_date: Date,
        market_data: MarketData,
    }

    impl MarketDataProvider for SimpleMarketDataProvider {
        fn handle_request(&self, _: &MarketDataRequest) -> Result<MarketData> {
            Ok(MarketData::new(
                self.market_data.fixings().clone(),
                self.market_data.constructed_elements().clone(),
            )
            .with_fx_store(self.market_data.fx_store().cloned().unwrap_or_default()))
        }

        fn evaluation_date(&self) -> Date {
            self.evaluation_date
        }
    }

    /// Returns the synthetic base-currency discount index used by FX-forward tests.
    fn base_index() -> MarketIndex {
        MarketIndex::Other("EUR_DISC".to_string())
    }

    /// Returns the synthetic quote-currency discount index used by FX-forward tests.
    fn quote_index() -> MarketIndex {
        MarketIndex::Other("USD_DISC".to_string())
    }

    /// Builds flat discount curves for both currencies plus an FX spot store.
    fn setup_fx_forward_market_data(
        trade_date: Date,
        spot: f64,
        base_rate: f64,
        quote_rate: f64,
    ) -> Result<MarketData> {
        let base_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(base_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("base_rate".to_string());
        let quote_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(quote_rate),
            RateDefinition::default(),
        )
        .with_pillar_label("quote_rate".to_string());

        let mut constructed_elements = ConstructedElementStore::default();
        constructed_elements.discount_curves_mut().insert(
            base_index(),
            DiscountCurveElement::new(base_index(), Rc::new(RefCell::new(base_curve))),
        );
        constructed_elements.discount_curves_mut().insert(
            quote_index(),
            DiscountCurveElement::new(quote_index(), Rc::new(RefCell::new(quote_curve))),
        );

        let mut fx_store = FxStore::new();
        fx_store.add_fx_rate(BASE_CCY, QUOTE_CCY, DualFwd::new(spot));

        Ok(MarketData::new(HashMap::new(), constructed_elements).with_fx_store(fx_store))
    }

    /// Evaluates an FX forward with the given strike/spot, returning results.
    fn evaluate_fx_forward(
        trade_date: Date,
        delivery_date: Date,
        spot: f64,
        strike: f64,
        notional: f64,
        base_rate: f64,
        quote_rate: f64,
        requests: &[Request],
    ) -> Result<EvaluationResults> {
        let market_data = setup_fx_forward_market_data(trade_date, spot, base_rate, quote_rate)?;
        let fx_fwd = MakeFxForward::default()
            .with_identifier("EURUSD-FWD".to_string())
            .with_delivery_date(delivery_date)
            .with_forward_price(strike)
            .with_base_currency(BASE_CCY)
            .with_quote_currency(QUOTE_CCY)
            .as_deliverable()
            .build()?;
        let trade = FxForwardTrade::new(fx_fwd, trade_date, notional, Side::LongReceive);

        let provider = SimpleMarketDataProvider {
            evaluation_date: trade_date,
            market_data,
        };

        let mut pricer = FxForwardPricer::new();
        pricer.set_discount_policy(Box::new(FxDiscountPolicy {
            base_index: base_index(),
            quote_index: quote_index(),
        }));
        pricer.evaluate(&trade, requests, &provider)
    }

    /// Computes the model discount factors used for closed-form comparisons.
    fn discount_factors(
        trade_date: Date,
        delivery_date: Date,
        base_rate: f64,
        quote_rate: f64,
    ) -> Result<(f64, f64)> {
        let base_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(base_rate),
            RateDefinition::default(),
        );
        let quote_curve = FlatForwardTermStructure::new(
            trade_date,
            DualFwd::from(quote_rate),
            RateDefinition::default(),
        );
        Ok((
            base_curve.discount_factor(delivery_date)?.value(),
            quote_curve.discount_factor(delivery_date)?.value(),
        ))
    }

    #[test]
    fn fair_forward_matches_covered_interest_parity() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let delivery_date = trade_date + Period::new(6, TimeUnit::Months);
        let (spot, base_rate, quote_rate) = (1.10, 0.03, 0.05);

        let results = evaluate_fx_forward(
            trade_date,
            delivery_date,
            spot,
            1.10,
            1_000_000.0,
            base_rate,
            quote_rate,
            &[Request::FairRate(0)],
        )?;
        let fair = results
            .fair_rate()
            .ok_or_else(|| QSError::UnexpectedErr("Missing fair rate".into()))?;

        let (df_base, df_quote) =
            discount_factors(trade_date, delivery_date, base_rate, quote_rate)?;
        let expected = spot * df_quote / df_base;
        assert!(
            (fair - expected).abs() < 1e-12,
            "Fair forward {fair} should match parity {expected}"
        );
        Ok(())
    }

    /// Boundary test: the NPV at the fair forward strike must be zero.
    #[test]
    fn npv_is_zero_at_fair_forward_strike() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let delivery_date = trade_date + Period::new(6, TimeUnit::Months);
        let (spot, base_rate, quote_rate) = (1.10, 0.03, 0.05);
        let notional = 1_000_000.0;

        let (df_base, df_quote) =
            discount_factors(trade_date, delivery_date, base_rate, quote_rate)?;
        let fair = spot * df_quote / df_base;

        let results = evaluate_fx_forward(
            trade_date,
            delivery_date,
            spot,
            fair,
            notional,
            base_rate,
            quote_rate,
            &[Request::Value],
        )?;
        let npv = results
            .price()
            .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;
        assert!(
            npv.abs() < 1e-6,
            "NPV at fair strike should be zero, got {npv}"
        );
        Ok(())
    }

    /// Ladder test: NPV must be linear in strike with slope `-N * df_quote`.
    #[test]
    fn npv_ladder_is_linear_in_strike() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let delivery_date = trade_date + Period::new(6, TimeUnit::Months);
        let (spot, base_rate, quote_rate) = (1.10, 0.03, 0.05);
        let notional = 1_000_000.0;

        let (df_base, df_quote) =
            discount_factors(trade_date, delivery_date, base_rate, quote_rate)?;
        let fair = spot * df_quote / df_base;

        let strikes = [1.00, 1.05, 1.10, 1.15, 1.20];
        let mut npvs = Vec::new();
        for strike in strikes {
            let results = evaluate_fx_forward(
                trade_date,
                delivery_date,
                spot,
                strike,
                notional,
                base_rate,
                quote_rate,
                &[Request::Value],
            )?;
            let npv = results
                .price()
                .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))?;

            let expected = notional * (fair - strike) * df_quote;
            assert!(
                (npv - expected).abs() < 1e-4,
                "NPV {npv} should match closed form {expected} at strike {strike}"
            );
            npvs.push(npv);
        }

        // Consecutive ladder points must have constant slope -N * df_quote.
        let expected_slope = -notional * df_quote;
        for pair in npvs.windows(2) {
            let slope = (pair[1] - pair[0]) / 0.05;
            assert!(
                (slope - expected_slope).abs() < 1e-2,
                "Ladder slope {slope} should equal {expected_slope}"
            );
        }
        Ok(())
    }

    /// Ladder test: the AD spot sensitivity must match a central
    /// finite-difference bump-and-reprice of the FX spot across strikes.
    #[test]
    fn spot_sensitivity_matches_finite_difference_across_strikes() -> Result<()> {
        let trade_date = Date::new(2025, 1, 2);
        let delivery_date = trade_date + Period::new(6, TimeUnit::Months);
        let (spot, base_rate, quote_rate) = (1.10, 0.03, 0.05);
        let notional = 1_000_000.0;
        let bump = 1e-6;

        for strike in [1.00, 1.10, 1.20] {
            let results = evaluate_fx_forward(
                trade_date,
                delivery_date,
                spot,
                strike,
                notional,
                base_rate,
                quote_rate,
                &[Request::Value, Request::Sensitivities],
            )?;
            let sensitivities = results
                .sensitivities()
                .ok_or_else(|| QSError::UnexpectedErr("Missing sensitivities".into()))?;
            let ad_spot_sens = sensitivities
                .instrument_keys()
                .iter()
                .zip(sensitivities.exposure().iter().copied())
                .find(|(key, _)| key.contains('/'))
                .map(|(_, exposure)| exposure)
                .ok_or_else(|| QSError::NotFoundErr("FX spot sensitivity not found".into()))?;

            let npv_at = |s: f64| -> Result<f64> {
                evaluate_fx_forward(
                    trade_date,
                    delivery_date,
                    s,
                    strike,
                    notional,
                    base_rate,
                    quote_rate,
                    &[Request::Value],
                )?
                .price()
                .ok_or_else(|| QSError::UnexpectedErr("Missing price".into()))
            };
            let fd = (npv_at(spot + bump)? - npv_at(spot - bump)?) / (2.0 * bump);

            assert!(
                (ad_spot_sens - fd).abs() < 1e-2,
                "AD spot sensitivity {ad_spot_sens} vs FD {fd} mismatch at strike {strike}"
            );
        }
        Ok(())
    }
}
